# HelmAgent

[English](README.md) | [简体中文](README.zh-CN.md)

HelmAgent 是一个面向多编码 Agent 协作的本地协调层。

它为主 Agent 提供持久化任务看板、子 Agent 交接 brief、分发记录、review 检查点和恢复命令，让高速 AI 工作仍然可以被人类跟踪、审查和接管，而不是散落在多个终端会话和聊天上下文里。

HelmAgent 设计为本地运行。它把任务状态存储在 `HELM_AGENT_HOME` 下，可以通过 `tmux` 启动子 Agent，也可以通过 stdio 把任务交给兼容 ACP 的 Agent。

## 功能

- 持久化本地任务记录，覆盖 inbox、triage、queued、running、blocked、review、done 等状态。
- 为 Codex、Claude Code、OpenCode 或全部运行时生成主 Agent 工作指引。
- 通过项目级 `AGENTS.md` 和 `CLAUDE.md` include 接入，不修改全局 Agent 配置。
- 生成子 Agent 任务 brief，包含范围、恢复命令、最近事件和 review 指令。
- 支持 Claude、Codex、OpenCode 的 `tmux` 分发预览和真实子 Agent 会话。
- 支持 ACP Agent 注册，以及兼容 stdio Agent 的一次性 ACP brief 交接。
- 内置人工 review 流程，支持 ready-for-review、changes-requested、accepted 状态。
- 提供本地 Web 看板，用浏览器查看任务并记录进展。
- 提供 install、update、repair、doctor、uninstall 等完整 CLI 生命周期命令。

## 当前状态

HelmAgent 仍处于早期阶段，但已经可以作为本地 CLI 使用。当前重点是稳定的本地协调、明确的人工 review gate，以及在委派工作需要人工介入时提供可靠恢复路径。

## 依赖

- macOS 或其他类 Unix shell 环境。
- Rust 工具链，包括 `cargo`、`rustc` 和 `git`。
- `$HOME/.cargo/bin` 已加入 `PATH`。
- 使用 tmux 子 Agent 分发时需要安装 `tmux`。
- 只有使用 `--runtime acp` 时才需要 ACP 兼容 Agent 可执行文件。

## 安装

从 GitHub 安装：

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" install
```

从本地 checkout 安装：

```bash
git clone https://github.com/liusiyuxyfx/HelmAgent.git
cd HelmAgent
make install
```

安装脚本会通过 `cargo install` 安装二进制文件，并默认把本地支持文件写入 `$HOME/.helm-agent`。

## 更新、修复和卸载

从 GitHub 执行：

```bash
INSTALLER=/tmp/helm-agent-install.sh

curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" update
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" repair
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" doctor
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" uninstall
```

从本地 checkout 执行：

```bash
make update
make repair
make doctor
make uninstall
```

普通卸载会保留 `$HOME/.helm-agent`，避免误删任务记录。只有在你明确希望删除 HelmAgent 数据时，才使用 `make uninstall-purge` 或 `sh ./install.sh uninstall --purge`。

更多 dry-run、purge 安全检查、旧版 `init-project` 和环境变量说明见 [docs/install.md](docs/install.md)。

## 快速开始

先初始化一个项目，让主 Agent 能发现 HelmAgent 指令：

```bash
helm-agent project init --path /path/to/project --agent all
```

这会在项目的 `AGENTS.md` 和 `CLAUDE.md` 中加入 include，指向已安装的 `$HOME/.helm-agent/main-agent-template.md`。

为主 Agent 打印启动/操作提示词：

```bash
helm-agent agent prompt --runtime codex
helm-agent agent prompt --runtime claude
helm-agent agent prompt --runtime opencode
```

创建并分诊任务：

```bash
helm-agent task create --id PM-20260511-001 --title "Add retry tests" --project .
helm-agent task triage PM-20260511-001 --risk medium --priority high --runtime claude --review-reason "Touches retry policy"
```

打开任务看板：

```bash
helm-agent task board
helm-agent board serve --host 127.0.0.1 --port 8765
```

准备或启动子 Agent 交接：

```bash
helm-agent task dispatch PM-20260511-001 --runtime claude --dry-run
helm-agent task dispatch PM-20260511-001 --runtime claude --send-brief
```

把任务交给人工 review：

```bash
helm-agent task mark PM-20260511-001 --ready-for-review --message "Implementation and tests are ready"
helm-agent task review PM-20260511-001 --request-changes "Add a regression test before merging"
helm-agent task review PM-20260511-001 --accept
```

## ACP Agent

HelmAgent 可以注册 ACP 兼容 Agent，并通过 stdio 把生成的任务 brief 作为一次性 prompt 发送给它。

```bash
helm-agent acp agent add local-acp --command /path/to/acp-agent --arg=--stdio
helm-agent acp agent list
helm-agent task dispatch PM-20260511-001 --runtime acp --agent local-acp --dry-run
helm-agent task dispatch PM-20260511-001 --runtime acp --agent local-acp --confirm
```

ACP 分发会记录 ACP session id，并在交接完成后把任务移动到 `ready_for_review`。失败或超时的 ACP 分发会把任务移动到 `needs_changes`，方便修复 Agent 配置后重试。

## 常用命令

列出任务：

```bash
helm-agent task list
helm-agent task list --review
helm-agent task list --status blocked --status ready_for_review
```

查看或恢复单个任务：

```bash
helm-agent task status PM-20260511-001
helm-agent task resume PM-20260511-001
```

生成子 Agent brief：

```bash
helm-agent task brief PM-20260511-001
helm-agent task brief PM-20260511-001 --write
```

手动记录进展：

```bash
helm-agent task event PM-20260511-001 --type progress --message "Tests are running"
helm-agent task mark PM-20260511-001 --blocked --message "Waiting for API contract confirmation"
helm-agent task mark PM-20260511-001 --ready-for-review --message "Ready for review"
```

汇总 tmux 会话健康状态，再向人汇报委派会话健康状态（delegated session health）：

```bash
helm-agent task sync PM-20260511-001
helm-agent task sync --all
```

## 数据和隔离

默认情况下，HelmAgent 只写入：

```text
$HOME/.helm-agent/
```

以及你显式初始化的项目文件：

```text
AGENTS.md
CLAUDE.md
```

它不会安装全局 Claude Code hooks、Codex config、skills、agents 或 ACP servers。项目初始化使用 include 行接入，因此可以和已有工作流保持隔离。

常用环境变量：

```bash
HELM_AGENT_HOME=$HOME/.helm-agent
HELM_AGENT_TMUX_BIN=tmux
HELM_AGENT_ACP_TIMEOUT_MS=300000
```

## 开发

运行测试：

```bash
cargo test
```

从 checkout 中运行 CLI：

```bash
cargo run --bin helm-agent -- task create --id PM-20260512-DEV --title "Example task" --project .
cargo run --bin helm-agent -- task status PM-20260512-DEV
```

提交变更前检查：

```bash
cargo fmt -- --check
cargo test
git diff --check
```

## 文档

- [安装指南](docs/install.md)
- [主 Agent 集成](docs/agent-integrations/main-agent.md)

## 许可证

MIT。见 [LICENSE](LICENSE)。
