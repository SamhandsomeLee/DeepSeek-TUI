//! #4605 Phase 3: request/result dispatch coordinator.
//!
//! Moves Enter preparation (hooks, @file expand, auto-route, preflight,
//! system prompt) and Engine mailbox acceptance off the UI event-loop task.
//! The UI keeps ownership of [`App`] and only commits formal history after
//! [`DispatchEvent::Accepted`].
//!
//! No `Arc<Mutex<App>>`. Late results are matched by `dispatch_id` +
//! `generation`.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::compaction::CompactionConfig;
use crate::config::{ApiProvider, Config, ProviderIdentity};
use crate::core::engine::EngineHandle;
use crate::core::ops::Op;
use crate::hooks::{HookEvent, HookExecutor, MessageSubmitOutcome};
use crate::localization::Locale;
use crate::model_routing::AutoRouteSelection;
use crate::models::{ContentBlock, Message, SystemPrompt};
use crate::route_runtime::{resolve_runtime_route, resolve_runtime_route_for_identity};
use crate::tui::app::{AppMode, QueuedMessage, ReasoningEffort};
use crate::tui::approval::ApprovalMode;
use crate::tui::file_mention::ContextReference;
use crate::utils::spawn_supervised;
use codewhale_config::route::RouteLimits;

pub struct DispatchRequest {
    pub dispatch_id: String,
    pub generation: u64,
    pub message: QueuedMessage,
    pub context: DispatchContextSnapshot,
    pub engine: EngineHandle,
}

/// Immutable prep inputs captured on the UI thread at enqueue time.
#[derive(Debug, Clone)]
pub struct DispatchContextSnapshot {
    pub workspace: PathBuf,
    pub cwd: Option<PathBuf>,
    pub config: Config,
    pub route_identity: ProviderIdentity,
    pub route_config: Config,
    pub api_provider: ApiProvider,
    pub model: String,
    pub auto_model: bool,
    pub mode: AppMode,
    pub reasoning_effort: ReasoningEffort,
    pub api_messages: Vec<Message>,
    pub hooks: HookExecutor,
    pub hook_mode_label: String,
    pub hook_session_id: String,
    pub hook_tokens: u32,
    pub skills_dir: PathBuf,
    pub skills_scan_codewhale_only: bool,
    pub plugin_registry: Arc<crate::plugins::PluginRegistry>,
    pub ui_locale: Locale,
    pub translation_enabled: bool,
    pub show_thinking: bool,
    pub verbosity: Option<String>,
    pub active_route_limits: Option<RouteLimits>,
    pub allow_shell: bool,
    pub trust_mode: bool,
    pub auto_approve: bool,
    pub approval_mode: ApprovalMode,
    pub active_allowed_tools: Option<Vec<String>>,
    pub runtime_hook_executor: Option<Arc<HookExecutor>>,
    pub hunt_token_budget: Option<u32>,
    pub hunt_goal_status: crate::tools::goal::GoalStatus,
    pub hunt_quarry: Option<String>,
    pub paused_plan: PausedCommandPlan,
    pub client_preflight_required: bool,
    pub auto_compact: bool,
    pub auto_compact_user_configured: bool,
    pub auto_compact_threshold_percent: f64,
}

/// Paused-command plan computed on the UI thread (cheap) and applied only
/// after Engine acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PausedCommandPlan {
    None,
    ClearWithoutQuarry,
    Resume { quarry: String, note: String },
    Detach { note: String },
}

impl PausedCommandPlan {
    pub fn note(&self) -> Option<&str> {
        match self {
            Self::Resume { note, .. } | Self::Detach { note } => Some(note),
            Self::None | Self::ClearWithoutQuarry => None,
        }
    }

    pub fn goal_objective(&self) -> Option<String> {
        match self {
            Self::Resume { quarry, .. } => Some(quarry.clone()),
            Self::Detach { .. } | Self::ClearWithoutQuarry => None,
            Self::None => None,
        }
    }

    pub fn apply(self, app: &mut crate::tui::app::App, engine_handle: &EngineHandle) {
        engine_handle.set_paused(false);
        match self {
            Self::None => {}
            Self::ClearWithoutQuarry => {
                app.paused = false;
                app.pausable = false;
            }
            Self::Resume { quarry, .. } => {
                app.paused = false;
                app.paused_quarry = None;
                app.hunt.quarry = Some(quarry);
                app.pausable = true;
            }
            Self::Detach { .. } => {
                app.paused = false;
                app.hunt.quarry = None;
                app.hunt.tokens_used = 0;
                app.hunt.time_used_seconds = 0;
                app.hunt.continuation_count = 0;
            }
        }
    }
}

/// Everything the UI needs to commit a successful send.
#[derive(Debug)]
pub struct PreparedDispatch {
    pub dispatch_id: String,
    pub generation: u64,
    pub message: QueuedMessage,
    pub references: Vec<ContextReference>,
    pub next_system_prompt: SystemPrompt,
    pub next_api_message: Message,
    pub turn_compaction: CompactionConfig,
    pub paused_plan: PausedCommandPlan,
    pub auto_selection: Option<AutoRouteSelection>,
    pub effective_provider: ApiProvider,
    pub effective_model: String,
    pub effective_provider_identity: String,
    pub effective_provider_label: String,
    pub selected_reasoning_effort: Option<ReasoningEffort>,
    pub auto_model: bool,
}

#[derive(Debug)]
pub enum DispatchEvent {
    Accepted(PreparedDispatch),
    Rejected {
        dispatch_id: String,
        generation: u64,
        original: QueuedMessage,
        error: String,
        /// When false, show status but do not treat as hard restore failure
        /// (e.g. hook block already has a user-facing reason).
        restore_composer: bool,
    },
}

#[derive(Clone)]
pub struct DispatchCoordinatorHandle {
    tx: mpsc::Sender<DispatchRequest>,
}

impl DispatchCoordinatorHandle {
    /// Non-blocking enqueue. On full/closed channel returns the request so
    /// the UI can restore the user message (never silent-drop).
    pub fn try_enqueue(&self, request: DispatchRequest) -> Result<(), DispatchRequest> {
        self.tx.try_send(request).map_err(|err| match err {
            mpsc::error::TrySendError::Full(req) | mpsc::error::TrySendError::Closed(req) => req,
        })
    }
}

/// Spawn the coordinator once and return the UI-facing handle + event drain.
pub fn spawn_dispatch_coordinator() -> (
    DispatchCoordinatorHandle,
    mpsc::UnboundedReceiver<DispatchEvent>,
) {
    let (req_tx, mut req_rx) = mpsc::channel::<DispatchRequest>(1);
    let (event_tx, event_rx) = mpsc::unbounded_channel::<DispatchEvent>();
    spawn_supervised(
        "dispatch-coordinator",
        std::panic::Location::caller(),
        async move {
            while let Some(request) = req_rx.recv().await {
                let event = prepare_and_accept(request).await;
                if event_tx.send(event).is_err() {
                    break;
                }
            }
        },
    );
    (
        DispatchCoordinatorHandle { tx: req_tx },
        event_rx,
    )
}

async fn prepare_and_accept(request: DispatchRequest) -> DispatchEvent {
    let DispatchRequest {
        dispatch_id,
        generation,
        mut message,
        context,
        engine,
    } = request;

    // --- hooks (Phase 1: dominant when configured) ---
    if context.hooks.has_hooks_for_event(HookEvent::MessageSubmit) {
        let hook_context = crate::hooks::HookContext::new()
            .with_mode(&context.hook_mode_label)
            .with_workspace(context.workspace.clone())
            .with_model(&context.model)
            .with_session_id(&context.hook_session_id)
            .with_tokens(context.hook_tokens)
            .with_message(&message.display);
        let outcome = context
            .hooks
            .execute_message_submit_transform(&hook_context, &message.display);
        match outcome {
            MessageSubmitOutcome::Unchanged { .. } => {}
            MessageSubmitOutcome::Replaced { text, .. } => {
                message.display = text;
            }
            MessageSubmitOutcome::Blocked { reason } => {
                return DispatchEvent::Rejected {
                    dispatch_id,
                    generation,
                    original: message,
                    error: reason,
                    restore_composer: true,
                };
            }
        }
    }

    let references = crate::tui::file_mention::context_references_from_input(
        &message.display,
        &context.workspace,
        context.cwd.clone(),
    );

    let mut content = match queued_message_content(&context, &message) {
        Ok(content) => content,
        Err(err) => {
            return DispatchEvent::Rejected {
                dispatch_id,
                generation,
                original: message,
                error: err.to_string(),
                restore_composer: true,
            };
        }
    };
    if let Some(note) = context.paused_plan.note() {
        content.push_str(note);
    }

    let auto_selection = if context.auto_model {
        match crate::model_routing::resolve_auto_route_with_inventory_for_session(
            &context.route_config,
            if content.trim().is_empty() {
                message.display.as_str()
            } else {
                content.as_str()
            },
            &crate::tui::auto_router::recent_auto_router_context(&context.api_messages),
            context.mode.as_setting(),
            "auto",
            context
                .reasoning_effort
                .as_setting_for_provider(context.api_provider),
        )
        .await
        {
            Ok(selection) => Some(selection),
            Err(err) => {
                return DispatchEvent::Rejected {
                    dispatch_id,
                    generation,
                    original: message,
                    error: format!("Auto model route unavailable: {err}"),
                    restore_composer: true,
                };
            }
        }
    } else {
        None
    };

    let effective_provider = auto_selection
        .as_ref()
        .map(|selection| selection.provider)
        .unwrap_or(context.api_provider);
    let effective_model = if context.auto_model {
        auto_selection
            .as_ref()
            .map(|selection| selection.model.clone())
            .unwrap_or_else(|| {
                crate::model_routing::auto_model_heuristic(&message.display, &context.model)
            })
    } else {
        context.model.clone()
    };

    let turn_route = if effective_provider == context.route_identity.provider {
        resolve_runtime_route_for_identity(
            &context.route_config,
            &context.route_identity,
            Some(&effective_model),
        )
    } else {
        resolve_runtime_route(
            &context.route_config,
            effective_provider,
            Some(&effective_model),
        )
    };
    let turn_route = match turn_route {
        Ok(route) => route,
        Err(err) => {
            return DispatchEvent::Rejected {
                dispatch_id,
                generation,
                original: message,
                error: err.to_string(),
                restore_composer: true,
            };
        }
    };
    let turn_route = if context.client_preflight_required {
        match turn_route.preflight() {
            Ok(route) => route,
            Err(err) => {
                return DispatchEvent::Rejected {
                    dispatch_id,
                    generation,
                    original: message,
                    error: err.to_string(),
                    restore_composer: true,
                };
            }
        }
    } else {
        turn_route
    };

    let turn_route_limits = crate::route_budget::known_route_limits(turn_route.candidate.limits());
    let effective_provider_identity = turn_route.identity.key.clone();
    let effective_provider_label = if effective_provider == ApiProvider::Custom {
        effective_provider_identity.clone()
    } else {
        effective_provider.display_name().to_string()
    };
    let turn_compaction = compaction_for_snapshot(&context, &turn_route.model, turn_route_limits);
    let goal_objective = context
        .paused_plan
        .goal_objective()
        .or_else(|| context.hunt_quarry.clone());
    let next_system_prompt = build_system_prompt(&context, goal_objective.as_deref());
    let next_api_message = Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: content.clone(),
            cache_control: None,
        }],
    };
    let auto_controls_reasoning =
        context.auto_model || context.reasoning_effort == ReasoningEffort::Auto;
    let selected_reasoning_effort = if auto_controls_reasoning {
        let effort = auto_selection
            .as_ref()
            .and_then(|selection| selection.reasoning_effort)
            .unwrap_or_else(|| crate::auto_reasoning::select(false, &message.display));
        Some(effort)
    } else {
        None
    };
    let effective_reasoning_effort = if let Some(effort) = selected_reasoning_effort {
        effort
            .api_value_for_route(
                effective_provider,
                &turn_route.candidate.endpoint().base_url,
                &turn_route.model,
            )
            .map(str::to_string)
    } else {
        context
            .reasoning_effort
            .api_value_for_route(
                effective_provider,
                &turn_route.candidate.endpoint().base_url,
                &turn_route.model,
            )
            .map(str::to_string)
    };

    if let Err(err) = engine
        .send(Op::SendMessage {
            content,
            mode: context.mode,
            route: Box::new(turn_route),
            compaction: Box::new(turn_compaction.clone()),
            goal_objective,
            goal_token_budget: context.hunt_token_budget,
            goal_status: context.hunt_goal_status,
            reasoning_effort: effective_reasoning_effort,
            reasoning_effort_auto: auto_controls_reasoning,
            auto_model: context.auto_model,
            allow_shell: context.allow_shell,
            trust_mode: context.trust_mode,
            auto_approve: context.auto_approve,
            approval_mode: context.approval_mode,
            translation_enabled: context.translation_enabled,
            show_thinking: context.show_thinking,
            allowed_tools: context.active_allowed_tools.clone(),
            dynamic_tools: Vec::new(),
            hook_executor: context.runtime_hook_executor.clone(),
            verbosity: context.verbosity.clone(),
            provenance: crate::core::ops::UserInputProvenance::ExternalUser,
        })
        .await
    {
        return DispatchEvent::Rejected {
            dispatch_id,
            generation,
            original: message,
            error: err.to_string(),
            restore_composer: true,
        };
    }

    DispatchEvent::Accepted(PreparedDispatch {
        dispatch_id,
        generation,
        message,
        references,
        next_system_prompt,
        next_api_message,
        turn_compaction,
        paused_plan: context.paused_plan,
        auto_selection,
        effective_provider,
        effective_model,
        effective_provider_identity,
        effective_provider_label,
        selected_reasoning_effort,
        auto_model: context.auto_model,
    })
}

fn queued_message_content(
    context: &DispatchContextSnapshot,
    message: &QueuedMessage,
) -> anyhow::Result<String> {
    if let Some(authority) = message.skill_provenance.as_ref() {
        if authority.workspace != context.workspace {
            anyhow::bail!("Queued plugin skill belongs to a different workspace and was denied");
        }
        crate::plugins::registry::verify_plugin_authority(authority).map_err(anyhow::Error::msg)?;
    }
    let user_request = crate::tui::file_mention::user_request_with_file_mentions(
        &message.display,
        &context.workspace,
        context.cwd.clone(),
    );
    if let Some(skill_instruction) = message.skill_instruction.as_ref() {
        Ok(format!(
            "{skill_instruction}\n\n---\n\nUser request: {user_request}"
        ))
    } else {
        Ok(user_request)
    }
}

fn compaction_for_snapshot(
    context: &DispatchContextSnapshot,
    model: &str,
    route_limits: Option<RouteLimits>,
) -> CompactionConfig {
    CompactionConfig {
        enabled: if context.auto_compact_user_configured {
            context.auto_compact
        } else {
            crate::route_budget::auto_compact_default_for_route(
                context.api_provider,
                model,
                route_limits,
            )
        },
        token_threshold: crate::route_budget::compaction_threshold_for_route_at_percent(
            context.api_provider,
            model,
            route_limits,
            context.auto_compact_threshold_percent,
        ),
        model: model.to_string(),
        effective_context_window: Some(crate::route_budget::route_context_window_tokens(
            context.api_provider,
            model,
            route_limits,
        )),
        ..Default::default()
    }
}

fn build_system_prompt(
    context: &DispatchContextSnapshot,
    goal_objective: Option<&str>,
) -> SystemPrompt {
    let instructions = crate::tui::ui::configured_instruction_sources(&context.config);
    let memory_path = context.config.memory_path();
    let user_memory_block = crate::memory::compose_block(
        context.config.memory_enabled() && !context.config.moraine_fallback(),
        &memory_path,
    );
    crate::prompts::system_prompt_for_mode_with_context_skills_and_session(
        &context.workspace,
        None,
        Some(&context.skills_dir),
        Some(&instructions),
        crate::prompts::PromptSessionContext {
            user_memory_block: user_memory_block.as_deref(),
            goal_objective,
            project_context_pack_enabled: context.config.project_context_pack_enabled(),
            locale_tag: context.ui_locale.tag(),
            translation_enabled: context.translation_enabled,
            model_id: &context.model,
            context_window_override: Some(crate::route_budget::route_context_window_tokens(
                context.api_provider,
                &context.model,
                context.active_route_limits,
            )),
            show_thinking: context.show_thinking,
            verbosity: context.verbosity.as_deref(),
            skills_scan_codewhale_only: context.skills_scan_codewhale_only,
            plugin_registry: Some(context.plugin_registry.as_ref()),
        },
    )
}
