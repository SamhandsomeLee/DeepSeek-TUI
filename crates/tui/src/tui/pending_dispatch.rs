//! #4605 Phase 2: immediate Enter acknowledgement via pending UI state.
//!
//! Enter arms a non-persistent [`PendingUserDispatch`], clears the composer,
//! and returns so the event loop can paint one Preparing/empty-composer frame
//! before the existing synchronous `dispatch_user_message` path runs.
//!
//! Formal `HistoryCell` / `api_messages` / checkpoint commits still happen only
//! after the Engine accepts `Op::SendMessage` (unchanged invariant).

use std::time::Instant;

use crate::tui::app::QueuedMessage;

/// Status text shown while a deferred send is waiting for its first redraw.
pub const PREPARING_MESSAGE_STATUS: &str = "Preparing message…";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDispatchState {
    /// Armed at Enter; waiting for the first successful draw.
    Preparing,
    /// First Preparing/empty-composer frame has been painted; flush may run.
    ReadyToDispatch,
    /// #4605 Phase 3: coordinator owns prep + Engine acceptance.
    AwaitingEngine,
}

/// UI-only pending send. Must not be persisted as a formal turn.
#[derive(Debug, Clone)]
pub struct PendingUserDispatch {
    /// Stable id for late-result matching (Phase 3 coordinator).
    pub id: String,
    pub message: QueuedMessage,
    #[allow(dead_code)]
    pub created_at: Instant,
    pub state: PendingDispatchState,
    /// Session generation at arm time; stale results are dropped.
    pub generation: u64,
    /// `/edit` already undid locally; flush still needs `Op::SyncSession`.
    pub needs_edit_sync: bool,
}

impl PendingUserDispatch {
    pub fn display(&self) -> &str {
        &self.message.display
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, PendingDispatchState::ReadyToDispatch)
    }

    pub fn is_awaiting_engine(&self) -> bool {
        matches!(self.state, PendingDispatchState::AwaitingEngine)
    }

    pub fn mark_redrawn(&mut self) {
        if matches!(self.state, PendingDispatchState::Preparing) {
            self.state = PendingDispatchState::ReadyToDispatch;
        }
    }

    pub fn mark_awaiting_engine(&mut self) {
        self.state = PendingDispatchState::AwaitingEngine;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparing_becomes_ready_only_after_redraw() {
        let mut pending = PendingUserDispatch {
            id: "send-test".to_string(),
            message: QueuedMessage::new("hi".to_string(), None),
            created_at: Instant::now(),
            state: PendingDispatchState::Preparing,
            generation: 1,
            needs_edit_sync: false,
        };
        assert!(!pending.is_ready());
        pending.mark_redrawn();
        assert!(pending.is_ready());
        assert_eq!(pending.state, PendingDispatchState::ReadyToDispatch);
    }
}
