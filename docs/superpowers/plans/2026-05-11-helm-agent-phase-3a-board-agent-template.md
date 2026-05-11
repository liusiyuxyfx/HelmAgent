# HelmAgent Phase 3A Board And Agent Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a human-readable board and a copyable main-agent operating template so HelmAgent is usable for daily task coordination.

**Architecture:** Keep persistence unchanged. Add a `task board` CLI path that reuses `TaskStore::list_tasks`, then format grouped lanes in `output.rs`. Add docs-only template files without introducing runtime plugin installation.

**Tech Stack:** Rust CLI with Clap, existing YAML task records, integration tests via `assert_cmd`.

---

## Files

- Modify `src/cli.rs`: add `Board(BoardArgs)` subcommand and handler.
- Modify `src/output.rs`: add board lane selection and board rendering.
- Modify `tests/cli_task_flow.rs`: add board behavior integration tests.
- Create `docs/agent-integrations/main-agent-template.md`: copyable coordinator instructions.
- Modify `README.md`: mention `task board` and the template.
- Modify `docs/agent-integrations/main-agent.md`: link board usage and template.

## Task 1: Board Output

- [ ] **Step 1: Write failing board tests**

Add tests to `tests/cli_task_flow.rs`:

```rust
#[test]
fn board_groups_tasks_for_human_review() {
    // create tasks in inbox, running, blocked, required-review, done
    // run `task board`
    // assert headings: Inbox, Running, Blocked, Review, Done
    // assert archived task is not shown
    // assert triaged review-required task appears under Review
}

#[test]
fn board_includes_recovery_context_after_dispatch_preview() {
    // create task, dry-run dispatch to claude
    // run `task board`
    // assert attach and resume commands are shown
}
```

- [ ] **Step 2: Verify board tests fail**

Run:

```bash
rtk cargo test --test cli_task_flow board_
```

Expected: fail because `task board` does not exist.

- [ ] **Step 3: Implement board rendering**

Add:

```rust
pub fn task_board(tasks: &[TaskRecord]) -> String
```

Rules:

- Exclude archived in CLI before output.
- Sort by `updated_at` descending before rendering.
- Group with lane priority: Blocked, Review, exact status lane.
- Print only non-empty lanes.
- Print `No active tasks\n` when empty.

- [ ] **Step 4: Verify board tests pass**

Run:

```bash
rtk cargo test --test cli_task_flow board_
```

Expected: board tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src/cli.rs src/output.rs tests/cli_task_flow.rs
rtk git commit -m "feat: add task board"
```

## Task 2: Main-Agent Template

- [ ] **Step 1: Write failing docs test**

Add a test to `tests/cli_task_flow.rs` or a focused docs test:

```rust
#[test]
fn main_agent_template_contains_required_operating_commands() {
    let template = std::fs::read_to_string("docs/agent-integrations/main-agent-template.md").unwrap();
    assert!(template.contains("helm-agent task board"));
    assert!(template.contains("helm-agent task triage"));
    assert!(template.contains("helm-agent task dispatch --dry-run"));
    assert!(template.contains("--confirm"));
    assert!(template.contains("task review --accept"));
}
```

- [ ] **Step 2: Verify docs test fails**

Run:

```bash
rtk cargo test --test cli_task_flow main_agent_template
```

Expected: fail because the template file does not exist.

- [ ] **Step 3: Create template and update docs**

Create `docs/agent-integrations/main-agent-template.md` with direct coordinator instructions. Update `README.md` and `docs/agent-integrations/main-agent.md` to mention `task board` and link the template.

- [ ] **Step 4: Verify docs test passes**

Run:

```bash
rtk cargo test --test cli_task_flow main_agent_template
```

Expected: test passes.

- [ ] **Step 5: Commit**

```bash
rtk git add README.md docs/agent-integrations/main-agent.md docs/agent-integrations/main-agent-template.md tests/cli_task_flow.rs
rtk git commit -m "docs: add main agent operating template"
```

## Task 3: Final Verification And Review

- [ ] **Step 1: Run full verification**

```bash
rtk cargo test
rtk cargo fmt --check
rtk git diff --check
```

- [ ] **Step 2: Run smoke flow**

```bash
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase3a-smoke cargo run --quiet --bin helm-agent -- task create --id PM-20260511-BOARD --title "Board smoke" --project .
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase3a-smoke cargo run --quiet --bin helm-agent -- task triage PM-20260511-BOARD --risk medium --runtime claude --review-reason "Smoke review"
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase3a-smoke cargo run --quiet --bin helm-agent -- task board
```

Expected: board output contains `Review`, `PM-20260511-BOARD`, and `Smoke review`.

- [ ] **Step 3: Run multi-agent review**

Launch reviewers for:

- CLI behavior and state semantics.
- Output usability.
- Docs safety and overclaiming.

- [ ] **Step 4: Fix findings and re-verify**

Use TDD for behavior findings. Re-run full verification after fixes.
