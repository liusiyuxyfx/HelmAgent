use crate::domain::AgentRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAdapter {
    pub runtime: AgentRuntime,
    pub command: &'static str,
    pub native_resume_available: bool,
    pub acp_supported: bool,
    resume_template: &'static str,
}

impl RuntimeAdapter {
    pub fn for_runtime(runtime: AgentRuntime) -> Self {
        match runtime {
            AgentRuntime::Claude => Self {
                runtime,
                command: "claude",
                resume_template: "claude --resume {session_id}",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::Codex => Self {
                runtime,
                command: "codex",
                resume_template: "codex resume {session_id} --all",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::OpenCode => Self {
                runtime,
                command: "opencode",
                resume_template: "opencode resume {session_id}",
                native_resume_available: false,
                acp_supported: false,
            },
        }
    }

    pub fn resume_command(self, session_id: &str) -> String {
        self.resume_template.replace("{session_id}", session_id)
    }
}
