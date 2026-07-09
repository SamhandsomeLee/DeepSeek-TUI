# Dogfood: Automatic Workflow scenarios (#4131)

Reproducible checks for the soft-auto Workflow product path. Another engineer
should be able to rerun every scenario from this doc alone.

Related:

- [Automatic Workflows](AUTOMATIC_WORKFLOWS.md) — product behavior
- [Workflow Authoring](WORKFLOW_AUTHORING.md) — checked-in scripts / IR
- [Fleet + Workflow Tutorial](FLEET_WORKFLOW_TUTORIAL.md) — manual Fleet path
- Example scripts: [`docs/examples/dogfood-automatic/`](examples/dogfood-automatic/)
- Panel unit coverage: `crates/tui/src/tui/widgets/workflow_panel.rs` (`dogfood_*` tests)
- Runtime resilience: `crates/workflow-js/tests/vm_tests.rs` (`parallel_*`, cancel drop)

## Preconditions

```bash
# From a clean worktree on origin/main (or the PR under test)
cargo build -p codewhale-tui --locked
# Optional headless/runtime checks used by the scenarios below
cargo test -p codewhale-tui --locked dogfood_ -- --nocapture
cargo test -p codewhale-workflow-js --locked parallel_ fan_out cancel -- --nocapture
```

Isolate config so dogfood does not touch your real home:

```bash
export DOGFOOD_ROOT="$(mktemp -d)"
export CODEWHALE_HOME="$DOGFOOD_ROOT/codewhale-home"
export HOME="$DOGFOOD_ROOT/home"
mkdir -p "$HOME" "$CODEWHALE_HOME" "$DOGFOOD_ROOT/workspace"
cd "$DOGFOOD_ROOT/workspace"
# Point at the CodeWhale checkout under test for read/audit work
export CODEWHALE_SRC=/path/to/CodeWhale
```

Safety:

1. Do not `git push` during dogfood.
2. Prefer read-only prompts first; approve write/worktree runs deliberately.
3. Tear down with `rm -rf "$DOGFOOD_ROOT"` when finished.

Primary interactive surface:

```bash
codewhale-tui   # or: cargo run -p codewhale-tui --locked
```

Confirm soft-auto is on (`[workflow] automatic = true` is the default).

---

## Scenario matrix

| ID | Scenario | Primary prompt / command | Expected UI | Automated check |
|----|----------|--------------------------|-------------|-----------------|
| WF-A1 | Read-only repo audit | natural-language audit prompt | soft-auto or `/workflow`; panel phases; no write-approval for pure read plan | `dogfood_read_only_repo_audit_panel` |
| WF-A2 | Staged bug fix (worktree implementer + verifier) | staged implement+verify prompt | implementer row `wt`, verifier on main or second phase; write approval if elevated | `dogfood_staged_worktree_implementer_verifier` |
| WF-A3 | Partial failure + synthesis | parallel partial-fail script / prompt | failed slots null/`fail` count; synthesis still produces operator summary | `dogfood_partial_failure_and_synthesis` + workflow-js `parallel_fan_out_*` |
| WF-A4 | Cancellation mid-run | start long run → panel `[c]` or `/workflow cancel` | lifecycle `cancelled`; running children cancelled; cancel_all invoked | `dogfood_cancellation_mid_run` + workflow-js drop/cancel tests |

Fill the pass/fail table at the bottom after each interactive pass.

---

## WF-A1 — Read-only repo audit

### Reproducible prompt

In `codewhale-tui` with workspace = CodeWhale checkout (or a copy):

```text
Audit this repository for security and reliability risks. Stay read-only:
list crates, scan for unsafe blocks and unwrap in hot paths, and summarize
findings by severity. Do not edit files or run destructive commands.
```

Force orchestration if soft-auto does not trigger:

```text
/workflow
```

Then restate the same audit goal, or run the checked-in example:

```text
/workflow run docs/examples/dogfood-automatic/wf_a1_read_only_audit.workflow.js
```

### Expected UI behavior

- Soft-auto may announce shape (“scout crates then synthesize”) before launch.
- Read-only plan may start without a write-approval card when
  `auto_start_read_only = true`.
- Workflow panel shows ≥1 phase and child rows with roles/labels (not
  “unknown child”).
- Compact history card remains calm; expand for phase/child detail.
- No worktree-required rows for pure read scouts (workspace = main).

### Pass / fail notes

| Check | Pass? | Notes |
|-------|-------|-------|
| Orchestration started (soft-auto or `/workflow`) | | |
| Panel shows phases + labeled children | | |
| No write approval for pure read plan | | |
| No file edits / no push | | |
| Synthesis summary operator-readable | | |

Automated:

```bash
cargo test -p codewhale-tui --locked dogfood_read_only_repo_audit_panel
```

---

## WF-A2 — Staged bug fix (worktree implementer + verifier)

### Reproducible prompt

```text
Staged fix for a small bug in the docs only:
1) implementer: add a one-line clarification to docs/AUTOMATIC_WORKFLOWS.md
   in an isolated worktree (do not touch main workspace).
2) verifier: re-read the file and confirm the change is correct; do not
   implement further edits.
Keep the change minimal and reversible.
```

Or run:

```text
/workflow run docs/examples/dogfood-automatic/wf_a2_staged_bugfix.workflow.js
```

### Expected UI behavior

- Elevated plan (writes / worktree) surfaces an approval card when
  `require_approval_for_writes = true` (#4126).
- Panel phases resemble: Implement → Verify (or equivalent labels).
- Implementer child row shows `wt` (worktree) isolation.
- Verifier child completes with a short confirmation summary.
- One artifact / card per delegated unit (no duplicate delegate spam).

### Pass / fail notes

| Check | Pass? | Notes |
|-------|-------|-------|
| Approval card for write/worktree plan | | |
| Implementer row marks worktree | | |
| Verifier runs after implementer | | |
| Main workspace untouched until merge/apply | | |
| Compact history card summarizes phases | | |

Automated:

```bash
cargo test -p codewhale-tui --locked dogfood_staged_worktree_implementer_verifier
```

---

## WF-A3 — Partial failure and synthesis

### Reproducible command / script

Headless runtime (always runnable):

```bash
cargo test -p codewhale-workflow-js --locked \
  parallel_fan_out_maps_one_failure_to_null_slot \
  parallel_logs_a_breadcrumb_when_a_slot_is_dropped_to_null
```

Interactive / tool path:

```text
/workflow run docs/examples/dogfood-automatic/wf_a3_partial_failure_synthesis.workflow.js
```

Natural-language equivalent:

```text
Run three parallel scouts; deliberately allow one to fail. Synthesize a single
operator-facing summary from the successful slots and call out the failed branch.
```

### Expected UI behavior

- Parallel slots that fail appear as failed/cancelled rows or null slots with
  a log breadcrumb (not a silent drop).
- Panel header shows non-zero `fail` count when a child fails.
- Run can still complete with a synthesis summary from surviving slots
  (`parallel()` partial-success semantics).
- Expanded history card lists failed child + overall result summary.

### Pass / fail notes

| Check | Pass? | Notes |
|-------|-------|-------|
| Failed slot visible (row and/or breadcrumb) | | |
| Successful slots still contribute to summary | | |
| Header fail count ≥ 1 | | |
| No full-run panic on single child failure | | |

Automated:

```bash
cargo test -p codewhale-tui --locked dogfood_partial_failure_and_synthesis
cargo test -p codewhale-workflow-js --locked parallel_fan_out_maps_one_failure_to_null_slot
```

---

## WF-A4 — Cancellation mid-run

### Reproducible steps

1. Start a long-running multi-child workflow:

```text
/workflow run docs/examples/dogfood-automatic/wf_a4_cancel_mid_run.workflow.js
```

Or a natural long audit with several scouts.

2. While status is `running`, cancel via one of:

```text
# Panel focus + key
[c]   # or X — Workflow panel cancel

# Slash
/workflow status
/workflow cancel <run_id>
```

### Expected UI behavior

- Panel shows `cancelling…` then lifecycle `cancelled`.
- Still-running children finalize as cancelled; already-succeeded rows stay
  succeeded.
- Host cancel path is idempotent (second cancel is a no-op).
- Completed panel remains visible until the next run starts.

### Pass / fail notes

| Check | Pass? | Notes |
|-------|-------|-------|
| Cancel accepted while running | | |
| Lifecycle becomes cancelled | | |
| Running children cancelled; done children preserved | | |
| Second cancel no-op | | |
| No hung agents after cancel | | |

Automated:

```bash
cargo test -p codewhale-tui --locked dogfood_cancellation_mid_run
cargo test -p codewhale-workflow-js --locked dropping_the_run_future_cancels_outstanding_tasks
```

---

## Bugs discovered

File or link bugs found during dogfood here (do not silently absorb them):

| Date | Scenario | Symptom | Issue / PR |
|------|----------|---------|------------|
| | | | |

---

## Interactive results log (copy per pass)

```
Tree / binary: _______________
Operator: _______________
Date: _______________

WF-A1: PASS / FAIL — notes:
WF-A2: PASS / FAIL — notes:
WF-A3: PASS / FAIL — notes:
WF-A4: PASS / FAIL — notes:

New bugs filed:
Follow-ups:
```

### CI / PR gate (non-interactive)

```bash
cargo fmt --all -- --check
cargo test -p codewhale-tui --locked dogfood_
cargo test -p codewhale-workflow-js --locked parallel_ fan_out cancel drop
```
