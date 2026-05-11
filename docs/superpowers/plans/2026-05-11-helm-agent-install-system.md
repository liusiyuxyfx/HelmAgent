# HelmAgent Install System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an open-source style installer with install, update, repair, doctor, uninstall, purge, and project initialization commands.

**Architecture:** Keep runtime HelmAgent unchanged. Add a POSIX shell installer with safe dry-run behavior, a Makefile as local command sugar, install documentation, and integration tests that exercise installer dry-run output without mutating the host.

**Tech Stack:** POSIX `sh`, Rust integration tests, Cargo install, Makefile.

---

## Files

- Create `install.sh`: installer entrypoint.
- Create `Makefile`: local shortcuts for install/update/repair/doctor/uninstall.
- Create `docs/install.md`: installation guide.
- Create `tests/install_script_tests.rs`: dry-run behavior tests.
- Modify `README.md`: add installation section.

## Task 1: Installer Dry-Run Tests

- [ ] **Step 1: Write failing tests**

Create `tests/install_script_tests.rs` with tests that run `sh install.sh ... --dry-run` and assert expected output for:

- install
- update
- repair
- doctor
- uninstall
- uninstall `--purge`
- init-project
- unknown command

- [ ] **Step 2: Verify tests fail**

Run:

```bash
rtk cargo test --test install_script_tests
```

Expected: fail because `install.sh` does not exist.

- [ ] **Step 3: Commit tests after implementation, not before**

These tests remain uncommitted until Task 2 makes them pass.

## Task 2: `install.sh`

- [ ] **Step 1: Create POSIX shell script**

Implement:

```bash
./install.sh install [--dry-run]
./install.sh update [--dry-run]
./install.sh repair [--dry-run]
./install.sh doctor [--dry-run]
./install.sh uninstall [--purge] [--dry-run]
./install.sh init-project <path> [--dry-run]
```

- [ ] **Step 2: Add safety helpers**

Helpers:

- `log`
- `run`
- `have`
- `ensure_home`
- `write_env`
- `cargo_install`
- `doctor`
- `usage`

- [ ] **Step 3: Verify tests pass**

Run:

```bash
rtk cargo test --test install_script_tests
```

Expected: all install script tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add install.sh tests/install_script_tests.rs
rtk git commit -m "feat: add installer script"
```

## Task 3: Makefile And Docs

- [ ] **Step 1: Add Makefile**

Targets:

- `install`
- `update`
- `repair`
- `doctor`
- `uninstall`
- `uninstall-purge`
- `test`
- `fmt`

- [ ] **Step 2: Add docs**

Create `docs/install.md` and update `README.md`.

Docs must include:

- curl install
- local install
- update
- repair
- doctor
- uninstall
- purge
- init-project
- env variables
- safety notes

- [ ] **Step 3: Verify docs contain required commands**

Add or extend tests to check docs mention required commands if useful.

- [ ] **Step 4: Commit**

```bash
rtk git add Makefile README.md docs/install.md
rtk git commit -m "docs: add install workflow"
```

## Task 4: Verification And Review

- [ ] **Step 1: Run full verification**

```bash
rtk cargo test
rtk cargo fmt --check
rtk git diff --check
```

- [ ] **Step 2: Optional shell validation**

```bash
rtk shellcheck install.sh
```

If unavailable, record that in final status.

- [ ] **Step 3: Smoke dry-run commands**

```bash
rtk sh install.sh install --dry-run
rtk sh install.sh update --dry-run
rtk sh install.sh repair --dry-run
rtk sh install.sh doctor --dry-run
rtk sh install.sh uninstall --dry-run
rtk sh install.sh uninstall --purge --dry-run
```

- [ ] **Step 4: Multi-agent review**

Review angles:

- Shell safety and host mutation risk.
- Open-source install UX.
- Docs correctness and no global workflow pollution.

- [ ] **Step 5: Fix findings and re-verify**

Use tests first for behavior changes.
