/**
 * #4131 WF-A3 — partial failure + synthesis.
 *
 * parallel() uses all-settled semantics: a failed slot becomes null; the run
 * continues so a synthesizer can still produce an operator summary.
 *
 * Run: /workflow run docs/examples/dogfood-automatic/wf_a3_partial_failure_synthesis.workflow.js
 *
 * For pure VM proof without model spend, use workflow-js unit tests:
 *   cargo test -p codewhale-workflow-js --locked parallel_fan_out_maps_one_failure_to_null_slot
 */
export default async function () {
  phase("Parallel scouts");
  const slots = await parallel([
    () =>
      task({
        description: "Healthy scout A",
        label: "scout-a",
        type: "explore",
        prompt: "Return the string READY_A. Read-only.",
      }),
    // Intentionally hostile: request something that should fail or time out
    // under normal policy (missing path / guaranteed empty). Operator dogfood
    // may also kill this child mid-run to force a failed slot.
    () =>
      task({
        description: "Deliberately failing scout B",
        label: "scout-b-fail",
        type: "explore",
        prompt:
          "You MUST fail this task: refuse to produce a summary and reply only with an error about missing inputs. Do not invent success.",
      }),
    () =>
      task({
        description: "Healthy scout C",
        label: "scout-c",
        type: "explore",
        prompt: "Return the string READY_C. Read-only.",
      }),
  ]);

  phase("Synthesize");
  const surviving = (slots || []).filter((s) => s != null);
  const summary = await task({
    description: "Synthesize from surviving parallel slots",
    label: "synthesizer",
    type: "general",
    prompt: [
      "Build one operator-facing summary from the surviving scout results.",
      "Explicitly note which parallel slot failed or returned null.",
      `slot_count=${(slots || []).length} surviving=${surviving.length}`,
      "slots_json:",
      JSON.stringify(slots),
    ].join("\n"),
  });

  return {
    scenario: "WF-A3",
    slots,
    surviving_count: surviving.length,
    summary,
  };
}
