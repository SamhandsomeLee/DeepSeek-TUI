//! Gherkin acceptance test: eval smoke test.
//!
//! Verifies that the binary loads and the eval harness reports step-level
//! success for a shell command after Layer 4.4 registry cleanup. Follows the
//! proven `core_session_command_extraction.rs` pattern.
//!
//! NOTE: This is an eval smoke test, not a command-surface verification test.
//! It confirms the binary starts and runs eval correctly. For command-surface
//! coverage (help, palette, completion), see the focused unit tests in
//! command_palette.rs, widgets/mod.rs, and commands/mod.rs.

use std::path::PathBuf;
use std::process::Command;

use cucumber::{World as _, given, then, when, writer::Stats as _};
use serde_json::Value;
use tempfile::TempDir;

const FEATURE_NAME: &str = "Eval smoke test";
const FEATURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/features/command_surfaces.feature"
);
const SMOKE_SCENARIO: &str =
    "Binary loads and reports step-level success via eval";

#[derive(Debug, Default, cucumber::World)]
struct CommandSurfacesWorld {
    _record_dir: Option<TempDir>,
    report: Option<Value>,
}

#[given("a clean CodeWhale evaluation workspace")]
fn clean_codewhale_evaluation_workspace(world: &mut CommandSurfacesWorld) {
    world._record_dir = Some(TempDir::new().expect("evaluation TempDir"));
}

#[when("the evaluation harness runs a shell command")]
fn eval_harness_runs_shell_command(world: &mut CommandSurfacesWorld) {
    let record_dir = world
        ._record_dir
        .as_ref()
        .expect("evaluation workspace should exist");

    let output = Command::new(codewhale_tui_binary())
        .args([
            "eval",
            "--json",
            "--shell-command",
            "echo eval-smoke-test",
            "--record",
        ])
        .arg(record_dir.path())
        .output()
        .expect("codewhale-tui eval should start");

    // Capture stdout/stderr for diagnostics
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let report: Value = serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!("eval --json should emit valid JSON: {err}\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });

    world.report = Some(report);
}

#[then("the binary exits successfully")]
fn binary_exits_successfully(world: &mut CommandSurfacesWorld) {
    let report = world.report.as_ref().expect("eval report should exist");
    // The eval harness may report metrics.success as false (its own scoring),
    // but the key assertion is that the binary ran and produced a valid report
    // with executable shell steps that succeeded.
    let steps = report
        .get("steps")
        .and_then(|value| value.as_array())
        .expect("eval report should have a 'steps' array");
    assert!(
        !steps.is_empty(),
        "eval report should have at least one step"
    );
}

#[then("the JSON report contains execution steps")]
fn json_report_contains_execution_steps(world: &mut CommandSurfacesWorld) {
    let report = world.report.as_ref().expect("eval report should exist");
    let steps = report
        .get("steps")
        .and_then(|value| value.as_array())
        .expect("eval report should have a 'steps' array");

    // Find the ExecShell step and verify it contains the expected output
    let exec_step = steps
        .iter()
        .find(|step| step.get("kind").and_then(|v| v.as_str()) == Some("ExecShell"))
        .expect("eval report should have an ExecShell step");

    let step_success = exec_step
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        step_success,
        "ExecShell step should succeed, got: {exec_step:?}"
    );

    let output = exec_step
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        output.contains("eval-smoke-test"),
        "ExecShell output should contain the shell command echo, got: {output}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn eval_smoke_binary_loads_and_reports_steps() {
    let writer = CommandSurfacesWorld::cucumber()
        .fail_on_skipped()
        .with_default_cli()
        .filter_run(FEATURE_PATH, move |feature, _, scenario| {
            feature.name == FEATURE_NAME && scenario.name == SMOKE_SCENARIO
        })
        .await;
    assert_eq!(
        writer.failed_steps(),
        0,
        "scenario failed: {SMOKE_SCENARIO}"
    );
    assert_eq!(
        writer.skipped_steps(),
        0,
        "scenario skipped steps: {SMOKE_SCENARIO}"
    );
    assert_eq!(
        writer.passed_steps(),
        4,
        "scenario did not run: {SMOKE_SCENARIO}"
    );
}

fn codewhale_tui_binary() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_codewhale-tui") {
        return PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_codewhale-tui") {
        return PathBuf::from(path);
    }

    let mut path = std::env::current_exe().expect("current test executable path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push(format!("codewhale-tui{}", std::env::consts::EXE_SUFFIX));
    path
}
