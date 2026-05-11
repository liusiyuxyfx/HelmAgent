# HelmAgent Tmux Brief Injection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in tmux brief injection to real dispatch.

**Architecture:** Extend the existing `Launcher` as the tmux boundary by adding a `send_keys` helper. Keep task state changes in `src/cli.rs`, using the existing pre-launch persisted brief as the handoff source. Record `brief_sent` or warning events through the existing event log.

**Tech Stack:** Rust, clap, tmux CLI, existing HelmAgent YAML/JSONL store, assert_cmd integration tests.

---

### Task 1: Launcher Send Keys

**Files:**
- Modify: `src/launcher.rs`
- Test: `tests/launcher_tests.rs`

- [ ] **Step 1: Write failing tests**

Add tests that expect `Launcher::send_keys` to invoke:

```text
send-keys
-t
=helm-agent-PM-20260511-SEND-claude
hello child agent
Enter
```

and to return a useful error when tmux exits non-zero.

- [ ] **Step 2: Run red test**

Run:

```bash
rtk cargo test --test launcher_tests send_keys
```

Expected: compile failure because `send_keys` does not exist.

- [ ] **Step 3: Implement `Launcher::send_keys`**

Add a method that shells out to the configured tmux binary with `send-keys -t =<session> <message> Enter`, using the same output-context error style as `launch`.

- [ ] **Step 4: Run green test**

Run:

```bash
rtk cargo test --test launcher_tests send_keys
```

Expected: pass.

### Task 2: Dispatch `--send-brief`

**Files:**
- Modify: `src/cli.rs`
- Test: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write failing CLI tests**

Cover:

- `--send-brief --dry-run` fails early and does not call tmux.
- real dispatch with `--send-brief` calls fake tmux for both `new-session` and `send-keys`.
- success output includes `Brief sent: yes`.
- task events include `brief_sent`.
- send failure after launch exits success, prints recovery paths, writes warning event, and reports `Brief sent: no`.

- [ ] **Step 2: Run red tests**

Run:

```bash
rtk cargo test --test cli_task_flow send_brief
```

Expected: failure because the CLI flag and behavior do not exist.

- [ ] **Step 3: Implement CLI behavior**

Add `send_brief: bool` to `DispatchArgs`, reject it with `--dry-run`, build the handoff message from the persisted brief path, call `launcher.send_keys`, and record `brief_sent` or `brief_send_warning`.

- [ ] **Step 4: Run green tests**

Run:

```bash
rtk cargo test --test cli_task_flow send_brief
```

Expected: pass.

### Task 3: Docs And Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/agent-integrations/main-agent.md`
- Modify: `docs/agent-integrations/main-agent-template.md`
- Test: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write docs regression tests**

Assert docs include `--send-brief`, explain it is opt-in, and show it only on real dispatch examples.

- [ ] **Step 2: Update docs**

Document:

```bash
helm-agent task dispatch PM-20260509-101 --runtime claude --send-brief
```

and the recovery behavior when sending fails.

- [ ] **Step 3: Run full verification**

Run:

```bash
rtk cargo test
rtk cargo check
rtk cargo fmt -- --check
rtk git diff --check
```

Expected: all pass.
