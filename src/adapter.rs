use crate::domain::AgentRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAdapter {
    pub runtime: AgentRuntime,
    pub command: &'static str,
    pub native_resume_template: &'static str,
    pub native_resume_available: bool,
    pub acp_supported: bool,
}

impl RuntimeAdapter {
    pub fn for_runtime(runtime: AgentRuntime) -> Self {
        match runtime {
            AgentRuntime::Claude => Self {
                runtime,
                command: "claude",
                native_resume_template: "claude --resume <session-id>",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::Codex => Self {
                runtime,
                command: "codex",
                native_resume_template: "codex resume <session-id> --all",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::OpenCode => Self {
                runtime,
                command: "opencode",
                native_resume_template: "opencode resume <session-id>",
                native_resume_available: false,
                acp_supported: false,
            },
            AgentRuntime::Acp => Self {
                runtime,
                command: "acp",
                native_resume_template: "",
                native_resume_available: false,
                acp_supported: true,
            },
        }
    }
}
