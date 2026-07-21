//! Enter→send segmented latency tracing for #4605.
//!
//! Gated behind `CODEWHALE_TRACE_SEND_LATENCY=1` (or `true`). When disabled,
//! helpers are no-ops so the UI critical path pays only a cheap env check at
//! Enter. Traces never record prompt text, file contents, API keys, or Hook
//! payloads — only stage names, durations, and non-sensitive dimensions.

use std::time::{Duration, Instant};

/// Environment variable that enables send-path segment timing.
pub const TRACE_SEND_LATENCY_ENV: &str = "CODEWHALE_TRACE_SEND_LATENCY";

/// Marker set at Enter so the first successful draw can measure UI
/// acknowledgement latency (Enter → cleared composer / Sending state visible).
#[derive(Debug, Clone)]
pub struct PendingSendUiAck {
    pub dispatch_id: String,
    pub enter_received_at: Instant,
}

/// In-memory segment timer for a single Enter→dispatch attempt.
#[derive(Debug)]
pub struct SendLatencyTrace {
    pub dispatch_id: String,
    started_at: Instant,
    last_at: Instant,
    segments: Vec<(&'static str, Duration)>,
    /// Non-sensitive dimensions for the summary line.
    pub input_chars: usize,
    pub history_messages: usize,
}

impl SendLatencyTrace {
    pub fn start(dispatch_id: String) -> Self {
        let now = Instant::now();
        Self {
            dispatch_id,
            started_at: now,
            last_at: now,
            segments: Vec::with_capacity(16),
            input_chars: 0,
            history_messages: 0,
        }
    }

    pub fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.segments
            .push((name, now.saturating_duration_since(self.last_at)));
        self.last_at = now;
    }

    pub fn total(&self) -> Duration {
        self.last_at.saturating_duration_since(self.started_at)
    }

    pub fn segments(&self) -> &[(&'static str, Duration)] {
        &self.segments
    }

    pub fn slowest(&self) -> Option<(&'static str, Duration)> {
        self.segments.iter().copied().max_by_key(|(_, d)| *d)
    }

    /// Emit one structured summary line and drop the trace.
    pub fn finish(self) {
        let total = self.total();
        let slowest = self.slowest();
        let segments = self
            .segments
            .iter()
            .map(|(name, d)| format!("{name}={}ms", d.as_millis()))
            .collect::<Vec<_>>()
            .join(" ");
        let (slowest_name, slowest_ms) = match slowest {
            Some((name, d)) => (name, d.as_millis()),
            None => ("(none)", 0),
        };
        tracing::info!(
            target: "send_latency",
            dispatch_id = %self.dispatch_id,
            total_ms = total.as_millis() as u64,
            slowest = slowest_name,
            slowest_ms = slowest_ms as u64,
            input_chars = self.input_chars,
            history_messages = self.history_messages,
            "send latency {segments}"
        );
    }
}

/// Whether send-latency tracing is enabled for this process.
///
/// Reads the env on each call so tests can toggle it without OnceLock
/// sticky state. Enter is not hot enough for this to matter.
pub fn send_latency_trace_enabled() -> bool {
    match std::env::var(TRACE_SEND_LATENCY_ENV) {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Allocate a stable per-send id used by both the segment trace and UI ack.
pub fn new_dispatch_id() -> String {
    format!("send-{}", &uuid::Uuid::new_v4().simple().to_string()[..12])
}

/// Record UI acknowledgement once the terminal has drawn a frame that already
/// reflects Enter being received (empty composer, Preparing, and/or Sending).
pub fn maybe_record_send_ui_ack(
    pending: &mut Option<PendingSendUiAck>,
    composer_empty: bool,
    is_loading: bool,
) {
    maybe_record_send_ui_ack_with_preparing(pending, composer_empty, is_loading, false);
}

/// Like [`maybe_record_send_ui_ack`], also treating an explicit Preparing
/// status as acknowledgement (Phase 2 deferred dispatch).
pub fn maybe_record_send_ui_ack_with_preparing(
    pending: &mut Option<PendingSendUiAck>,
    composer_empty: bool,
    is_loading: bool,
    is_preparing: bool,
) {
    if pending.is_none() {
        return;
    }
    if !(composer_empty || is_loading || is_preparing) {
        return;
    }
    let ack = pending.take().expect("pending checked above");
    let latency = ack.enter_received_at.elapsed();
    tracing::debug!(
        target: "send_latency",
        dispatch_id = %ack.dispatch_id,
        ui_ack_ms = latency.as_millis() as u64,
        "send ui acknowledged"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{EnvVarGuard, lock_test_env};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn trace_disabled_by_default_when_env_unset() {
        let _lock = lock_test_env();
        let _guard = EnvVarGuard::remove(TRACE_SEND_LATENCY_ENV);
        assert!(!send_latency_trace_enabled());
    }

    #[test]
    fn trace_enabled_for_one_true_yes() {
        let _lock = lock_test_env();
        for value in ["1", "true", "TRUE", "yes", "Yes"] {
            let _guard = EnvVarGuard::set(TRACE_SEND_LATENCY_ENV, value);
            assert!(
                send_latency_trace_enabled(),
                "expected enabled for {value:?}"
            );
        }
        let _off = EnvVarGuard::set(TRACE_SEND_LATENCY_ENV, "0");
        assert!(!send_latency_trace_enabled());
        let _no = EnvVarGuard::set(TRACE_SEND_LATENCY_ENV, "false");
        assert!(!send_latency_trace_enabled());
    }

    #[test]
    fn mark_records_segments_and_identifies_slowest() {
        let mut trace = SendLatencyTrace::start("send-test".to_string());
        trace.input_chars = 10;
        trace.history_messages = 3;
        trace.mark("enter_received");
        thread::sleep(Duration::from_millis(5));
        trace.mark("submit_input_done");
        thread::sleep(Duration::from_millis(20));
        trace.mark("system_prompt_done");
        let slowest = trace.slowest().expect("segments");
        assert_eq!(slowest.0, "system_prompt_done");
        assert!(slowest.1 >= Duration::from_millis(15));
        assert!(trace.total() >= Duration::from_millis(20));
        // finish emits tracing; should not panic
        trace.finish();
    }

    #[test]
    fn ui_ack_records_only_when_composer_cleared_or_loading() {
        let mut pending = Some(PendingSendUiAck {
            dispatch_id: "send-ack".to_string(),
            enter_received_at: Instant::now(),
        });
        maybe_record_send_ui_ack(&mut pending, false, false);
        assert!(pending.is_some(), "should wait until UI reflects send");

        maybe_record_send_ui_ack(&mut pending, true, false);
        assert!(pending.is_none(), "empty composer acknowledges");
    }

    #[test]
    fn dispatch_id_is_stable_prefix_and_unique() {
        let a = new_dispatch_id();
        let b = new_dispatch_id();
        assert!(a.starts_with("send-"));
        assert!(b.starts_with("send-"));
        assert_ne!(a, b);
    }
}
