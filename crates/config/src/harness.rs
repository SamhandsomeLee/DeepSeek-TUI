//! Harness posture + profile config types (#3311).
//!
//! A *harness posture* is the agent-shaping policy (sub-agent cap, tool
//! surface, compaction/cache strategy, safety stance); a *harness profile*
//! binds a posture to a provider route + model pattern. Extracted verbatim
//! from lib.rs to separate this agent-posture domain from the rest of the
//! config schema; re-exported at the crate root so existing paths are
//! unchanged. Behavior is identical.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::ProviderKind;

/// Kinds of built-in harness postures.
///
/// A posture names the runtime strategy CodeWhale should use for a
/// provider/model route: how much context to preload, how aggressively to lean
/// on sub-agents, and how to balance prompt-cache stability against quick
/// exploration. Runtime selection is wired in later v0.9 slices; this config
/// model intentionally keeps the policy data explicit first.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessPostureKind {
    /// Full-featured default: rich constitution, broad tool catalog, and normal
    /// sub-agent posture.
    #[default]
    Standard,
    /// Cache-heavy: deeper prompt layering and prefix-cache-oriented context.
    CacheHeavy,
    /// Lean: smaller starting context, faster compaction, and stronger
    /// exploration/delegation bias.
    Lean,
    /// User-defined posture assembled from explicit knobs below.
    Custom,
}

/// How this posture should approach compaction and prompt-cache stability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessCompactionStrategy {
    #[default]
    Default,
    PrefixCache,
    Aggressive,
}

/// Which tool catalog shape this posture prefers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessToolSurface {
    #[default]
    Full,
    ReadOnly,
    Auto,
}

/// Safety posture applied when the runtime consumes a harness profile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessSafetyPosture {
    #[default]
    Standard,
    Strict,
    Permissive,
}

/// A concrete harness posture with policy knobs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HarnessPosture {
    /// Named posture kind.
    #[serde(default)]
    pub kind: HarnessPostureKind,
    /// Maximum number of concurrent sub-agents (0 = runtime default).
    #[serde(default)]
    pub max_subagents: usize,
    /// Prefer search-based/on-demand context over always-on documentation.
    #[serde(default)]
    pub prefer_codebase_search: bool,
    /// Compaction and prompt-cache strategy.
    #[serde(default)]
    pub compaction_strategy: HarnessCompactionStrategy,
    /// Preferred tool catalog shape.
    #[serde(default)]
    pub tool_surface: HarnessToolSurface,
    /// Safety posture for runtime consumers.
    #[serde(default)]
    pub safety_posture: HarnessSafetyPosture,
}

impl Default for HarnessPosture {
    fn default() -> Self {
        Self {
            kind: HarnessPostureKind::Standard,
            max_subagents: 0,
            prefer_codebase_search: false,
            compaction_strategy: HarnessCompactionStrategy::default(),
            tool_surface: HarnessToolSurface::default(),
            safety_posture: HarnessSafetyPosture::default(),
        }
    }
}

impl HarnessPosture {
    /// A cache-heavy posture tuned for DeepSeek V4 / MiMo-style models.
    #[must_use]
    pub fn cache_heavy() -> Self {
        Self {
            kind: HarnessPostureKind::CacheHeavy,
            max_subagents: 10,
            prefer_codebase_search: false,
            compaction_strategy: HarnessCompactionStrategy::PrefixCache,
            tool_surface: HarnessToolSurface::Full,
            safety_posture: HarnessSafetyPosture::Standard,
        }
    }

    /// A lean posture for smaller-context or weaker tool-use models.
    #[must_use]
    pub fn lean() -> Self {
        Self {
            kind: HarnessPostureKind::Lean,
            max_subagents: 20,
            prefer_codebase_search: true,
            compaction_strategy: HarnessCompactionStrategy::Aggressive,
            tool_surface: HarnessToolSurface::Full,
            safety_posture: HarnessSafetyPosture::Standard,
        }
    }
}

/// A harness profile binds a posture to a provider route and model pattern.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HarnessProfile {
    /// Provider route this profile applies to, e.g. "deepseek" or
    /// "xiaomi-mimo".
    pub provider_route: String,
    /// Regex or glob pattern for model names, e.g. "deepseek-v4.*".
    pub model_pattern: String,
    /// The posture to apply.
    #[serde(default)]
    pub posture: HarnessPosture,
}

impl HarnessProfile {
    /// Return true when this profile applies to the provider/model route.
    ///
    /// This is a pure config helper: matching a profile must not mutate runtime
    /// provider selection, prompts, auth, tools, context, or persisted config.
    #[must_use]
    pub fn matches_route(&self, provider_route: &str, model: &str) -> bool {
        provider_routes_equal(&self.provider_route, provider_route)
            && wildcard_pattern_matches(&self.model_pattern, model)
    }
}

/// Resolution source for harness profile selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HarnessSource {
    /// Matched a user-configured `[[harness_profiles]]` entry.
    UserProfile,
    /// Matched a built-in seed profile.
    BuiltInSeed,
    /// No match; fell back to the Standard default posture.
    #[default]
    Default,
}

/// Deterministic harness resolution for a provider/model route.
///
/// Pure data: constructing it must not mutate provider selection, prompts,
/// auth, tools, context, or persisted config. The [`Default`] value is a
/// no-match Standard resolution, suitable for an initial cached state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HarnessResolution {
    /// Effective posture (`HarnessPosture::default()` when nothing matched).
    pub posture: HarnessPosture,
    /// Where the posture came from.
    pub source: HarnessSource,
    /// Matched profile identity for display (`None` when `source` is `Default`).
    pub matched: Option<MatchedProfile>,
}

/// Display identity for a matched harness profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedProfile {
    pub provider_route: String,
    pub model_pattern: String,
}

struct ProfileCandidate<'a> {
    profile: &'a HarnessProfile,
    source: HarnessSource,
    declaration_index: usize,
}

/// Deterministic `model_pattern` specificity score.
///
/// Exact patterns (no `*`/`?`) beat wildcard patterns. Among wildcards, more
/// non-wildcard literal characters wins. This is a stable heuristic, not a
/// complete glob semantics engine.
#[must_use]
fn model_pattern_specificity_score(pattern: &str) -> (u8, usize) {
    let has_wildcard = pattern.contains('*') || pattern.contains('?');
    let literal_count = pattern
        .chars()
        .filter(|ch| *ch != '*' && *ch != '?')
        .count();
    if has_wildcard {
        (1, literal_count)
    } else {
        (0, literal_count)
    }
}

fn source_tier(source: HarnessSource) -> u8 {
    match source {
        HarnessSource::UserProfile => 0,
        HarnessSource::BuiltInSeed => 1,
        HarnessSource::Default => 2,
    }
}

fn compare_profile_candidates(
    left: &ProfileCandidate<'_>,
    right: &ProfileCandidate<'_>,
) -> std::cmp::Ordering {
    match source_tier(left.source).cmp(&source_tier(right.source)) {
        std::cmp::Ordering::Equal => {}
        ordering => return ordering,
    }

    let left_specificity = model_pattern_specificity_score(&left.profile.model_pattern);
    let right_specificity = model_pattern_specificity_score(&right.profile.model_pattern);
    match left_specificity
        .0
        .cmp(&right_specificity.0)
        .then_with(|| right_specificity.1.cmp(&left_specificity.1))
    {
        std::cmp::Ordering::Equal => {}
        ordering => return ordering,
    }

    left.declaration_index.cmp(&right.declaration_index)
}

/// Deterministic harness resolution for a provider/model route.
///
/// User profiles beat built-in seeds; narrower `model_pattern` beats broader;
/// declaration order is the stable tiebreak. Returns Standard posture with
/// [`HarnessSource::Default`] when nothing matches.
#[must_use]
pub fn resolve_harness_for_profiles(
    user_profiles: &[HarnessProfile],
    provider_route: &str,
    model: &str,
) -> HarnessResolution {
    let mut candidates = Vec::new();

    for (index, profile) in user_profiles.iter().enumerate() {
        if profile.matches_route(provider_route, model) {
            candidates.push(ProfileCandidate {
                profile,
                source: HarnessSource::UserProfile,
                declaration_index: index,
            });
        }
    }

    for (index, profile) in built_in_harness_profiles().iter().enumerate() {
        if profile.matches_route(provider_route, model) {
            candidates.push(ProfileCandidate {
                profile,
                source: HarnessSource::BuiltInSeed,
                declaration_index: index,
            });
        }
    }

    match candidates
        .iter()
        .min_by(|left, right| compare_profile_candidates(left, right))
    {
        Some(candidate) => HarnessResolution {
            posture: candidate.profile.posture.clone(),
            source: candidate.source,
            matched: Some(MatchedProfile {
                provider_route: candidate.profile.provider_route.clone(),
                model_pattern: candidate.profile.model_pattern.clone(),
            }),
        },
        None => HarnessResolution {
            posture: HarnessPosture::default(),
            source: HarnessSource::Default,
            matched: None,
        },
    }
}

/// Built-in profile seeds for common provider/model families.
///
/// User-configured profiles are always checked first; these seeds only provide
/// a stable resolver result when config has no narrower match.
#[must_use]
pub fn built_in_harness_profiles() -> &'static [HarnessProfile] {
    static PROFILES: OnceLock<Vec<HarnessProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        vec![
            HarnessProfile {
                provider_route: "deepseek".to_string(),
                model_pattern: "deepseek-v4*".to_string(),
                posture: HarnessPosture::cache_heavy(),
            },
            HarnessProfile {
                provider_route: "xiaomi-mimo".to_string(),
                model_pattern: "mimo-v2.5*".to_string(),
                posture: HarnessPosture::cache_heavy(),
            },
            HarnessProfile {
                provider_route: "arcee".to_string(),
                model_pattern: "trinity-large-thinking".to_string(),
                posture: HarnessPosture::cache_heavy(),
            },
            HarnessProfile {
                provider_route: "huggingface".to_string(),
                model_pattern: "*".to_string(),
                posture: HarnessPosture::lean(),
            },
            HarnessProfile {
                provider_route: "sglang".to_string(),
                model_pattern: "*".to_string(),
                posture: HarnessPosture::lean(),
            },
            HarnessProfile {
                provider_route: "vllm".to_string(),
                model_pattern: "*".to_string(),
                posture: HarnessPosture::lean(),
            },
            HarnessProfile {
                provider_route: "ollama".to_string(),
                model_pattern: "*".to_string(),
                posture: HarnessPosture::lean(),
            },
        ]
    })
}

fn provider_routes_equal(expected: &str, actual: &str) -> bool {
    match (ProviderKind::parse(expected), ProviderKind::parse(actual)) {
        (Some(expected), Some(actual)) => expected == actual,
        _ => expected.trim().eq_ignore_ascii_case(actual.trim()),
    }
}

fn wildcard_pattern_matches(pattern: &str, value: &str) -> bool {
    wildcard_chars_match(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
    )
}

fn wildcard_chars_match(pattern: &[char], value: &[char]) -> bool {
    let (mut pattern_idx, mut value_idx) = (0, 0);
    let mut star_idx: Option<usize> = None;
    let mut star_value_idx = 0;

    while value_idx < value.len() {
        if pattern_idx < pattern.len()
            && (pattern[pattern_idx] == '?' || pattern[pattern_idx] == value[value_idx])
        {
            pattern_idx += 1;
            value_idx += 1;
        } else if pattern_idx < pattern.len() && pattern[pattern_idx] == '*' {
            star_idx = Some(pattern_idx);
            pattern_idx += 1;
            star_value_idx = value_idx;
        } else if let Some(star) = star_idx {
            pattern_idx = star + 1;
            star_value_idx += 1;
            value_idx = star_value_idx;
        } else {
            return false;
        }
    }

    pattern[pattern_idx..].iter().all(|ch| *ch == '*')
}
