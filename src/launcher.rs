use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub tmux_session: String,
    pub start_command: String,
    pub attach_command: String,
    pub resume_command: String,
}

impl LaunchPlan {
    pub fn dry_run(
        task_id: &str,
        runtime: AgentRuntime,
        cwd: &Path,
        native_session_id: &str,
    ) -> Self {
        let adapter = RuntimeAdapter::for_runtime(runtime);
        let tmux_session = format!("helm-agent-{task_id}-{runtime}", runtime = runtime.as_str());

        Self {
            start_command: format!(
                "tmux new-session -d -s {tmux_session} -c {cwd} {command}",
                cwd = cwd.display(),
                command = adapter.command
            ),
            attach_command: format!("tmux attach -t {tmux_session}"),
            resume_command: adapter.resume_command(native_session_id),
            tmux_session,
        }
    }
}
