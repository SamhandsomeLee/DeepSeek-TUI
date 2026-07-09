/**
 * #4131 WF-A4 — cancellation mid-run dogfood fixture.
 *
 * Starts multiple long-lived explore tasks so the operator has time to press
 * panel [c] / X or run `/workflow cancel <run_id>`.
 *
 * Run: /workflow run docs/examples/dogfood-automatic/wf_a4_cancel_mid_run.workflow.js
 * Then cancel while lifecycle is running.
 */
export default async function () {
  phase("Long work");
  // Fan-out several slow scouts; cancel should finalize outstanding children.
  const results = await parallel([
    () =>
      task({
        description: "Slow scout 1 — cancel target",
        label: "slow-1",
        type: "explore",
        prompt:
          "Thoroughly inventory crates/tui/src file names (read-only). Take your time; list directories depth-first. Return a count at the end.",
      }),
    () =>
      task({
        description: "Slow scout 2 — cancel target",
        label: "slow-2",
        type: "explore",
        prompt:
          "Thoroughly inventory crates/config/src file names (read-only). Take your time; list directories depth-first. Return a count at the end.",
      }),
    () =>
      task({
        description: "Slow scout 3 — cancel target",
        label: "slow-3",
        type: "explore",
        prompt:
          "Thoroughly inventory docs/ markdown titles (read-only). Take your time. Return a count at the end.",
      }),
  ]);

  phase("Unreachable if cancelled");
  return { scenario: "WF-A4", results };
}
