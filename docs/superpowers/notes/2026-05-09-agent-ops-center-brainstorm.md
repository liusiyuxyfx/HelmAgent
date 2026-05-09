# Agent Ops Center Brainstorm Notes

Date: 2026-05-09
Status: discussion notes, not an approved spec

## Problem

AI agents can complete work faster than the human review loop can absorb. The user needs a coordinator that reduces task triage and context switching first, then helps with review, without turning every small item into a manual project management burden.

Priority order:

1. Task routing
2. Context recovery
3. Review support
4. Full workflow automation

## Desired Shape

The primary interaction should be a main agent running in Claude Code or Codex. The user talks to this main agent directly. The main agent creates tasks, assigns work, tracks progress, collects results, and tells the user when human review is needed.

A board is useful, but it is a companion surface for status and review rather than the main interaction model.

## Agent Runtime Preferences

OpenCode and Claude Code are preferred for routine work because they are effectively free for the user. Codex is paid and should be used selectively for tasks that justify the cost.

The system should support Claude Code, Codex, and OpenCode. It should be able to route tasks to different agents based on cost, risk, capability, and workflow fit.

## Permission Model

V1 should be semi-automatic:

- Low-risk small tasks can be started automatically, preferably on free agents.
- High-risk work, cross-project changes, paid Codex usage, and final review/merge should require user confirmation.
- The permission model should be policy-driven so it can later become advisory, semi-automatic, or more automatic without redesigning the system.

## Storage Model

Use a centralized workspace rather than per-project storage as the source of truth for tasks, sessions, progress, logs, and review items.

## Task Entry

V1 task entry is mainly through conversation with the main agent. The user tells the main agent what needs doing. The system should keep strong progress records so the user can come back after context switching.

## Session Model

Use tmux as the default execution surface for child agents. Each dispatched task should get a resumable tmux session. The user should be able to attach to a session directly to inspect or continue the work.

ACP should be used where practical for agent communication. If ACP is not stable or available for a runtime, the system can initially fall back to CLI prompts or tmux input.

## Workflow Isolation

The user is concerned that this tool could interfere with existing workflows such as Claude Code workflows, Codex Superpowers, hooks, skills, agents, and other installed systems.

V1 should keep isolation lightweight:

- Do not modify `~/.claude`, `~/.codex`, or project `.claude` / `.codex` configuration by default.
- Do not try to make project config override global workflow config as the primary isolation strategy.
- Start child agents through explicit wrappers and record which runtime/workflow/session was used.
- Treat stronger isolation as an upgrade path, not a V1 requirement.

Possible later isolation levels:

- Dedicated `CLAUDE_CONFIG_DIR` or `CODEX_HOME` per workflow.
- Git worktree per write task.
- Sandbox or container for untrusted/high-risk workflows.

## Recommended Architecture Direction

Design as a local Agent Ops Center, but build V1 at the size of a tmux-based dispatcher.

Core components:

- Main agent command/plugin layer
- Central task store
- Policy engine
- Agent launcher
- Workflow adapter
- Review/status board
- Lightweight workflow isolation rules

Main boundary:

The main agent routes, records, resumes, and asks for review. Child agents execute concrete work. The user remains the final authority for high-risk decisions.
