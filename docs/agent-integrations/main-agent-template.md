# HelmAgent Main-Agent Reminder

Use this project with HelmAgent when coordinating delegated AI coding work.

Prefer the `helm-agent-coordinator` skill. If the runtime does not expose skills, read the installed skill source at `$HELM_AGENT_HOME/skills/helm-agent-coordinator/SKILL.md`.

Before reporting status, run `helm-agent task board` or `helm-agent task status <id>`. Before delegating work, create and triage a HelmAgent task. After delegation, use `helm-agent task resume <id>` and `helm-agent task brief <id>` for human review or recovery.

Do not claim code-changing delegated work is complete until HelmAgent review is accepted.
