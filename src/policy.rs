use crate::domain::{AgentRuntime, RiskLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchDecision {
    AutoStart,
    ConfirmRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyInput {
    pub risk: RiskLevel,
    pub runtime: AgentRuntime,
    pub writes_files: bool,
    pub paid_runtime: bool,
    pub cross_project: bool,
    pub network_sensitive: bool,
}

impl PolicyInput {
    pub fn evaluate(self) -> DispatchDecision {
        if self.risk == RiskLevel::High
            || self.paid_runtime
            || self.runtime == AgentRuntime::Codex
            || self.cross_project
            || self.network_sensitive
        {
            return DispatchDecision::ConfirmRequired;
        }

        DispatchDecision::AutoStart
    }
}
