use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchPlan {
    pub task_id: String,
    pub runtime: AgentRuntime,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPreview {
    pub tmux_session: String,
    pub start_command: String,
    pub attach_command: String,
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Launcher;

impl Launcher {
    pub fn new() -> Self {
        Self
    }

    pub fn dry_run(&self, dispatch: &DispatchPlan) -> LaunchPreview {
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let tmux_session = format!(
            "helm-agent-{task_id}-{runtime}",
            task_id = dispatch.task_id,
            runtime = dispatch.runtime.as_str()
        );
        let resume_command = adapter
            .native_resume_available
            .then(|| adapter.native_resume_template.to_string());

        LaunchPreview {
            start_command: format!(
                "tmux new-session -d -s {tmux_session} -c {cwd} {command}",
                cwd = dispatch.cwd.display(),
                command = adapter.command
            ),
            attach_command: format!("tmux attach -t {tmux_session}"),
            resume_command,
            tmux_session,
        }
    }
}
