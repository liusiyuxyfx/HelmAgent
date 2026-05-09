use helm_agent::domain::{AgentRuntime, RiskLevel};
use helm_agent::policy::{DispatchDecision, PolicyInput};

#[test]
fn low_risk_free_read_task_can_auto_start() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::OpenCode,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::AutoStart);
}

#[test]
fn codex_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::Codex,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn paid_runtime_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::Claude,
        writes_files: false,
        paid_runtime: true,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn high_risk_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::High,
        runtime: AgentRuntime::Claude,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn medium_risk_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Medium,
        runtime: AgentRuntime::Claude,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn cross_project_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::OpenCode,
        writes_files: false,
        paid_runtime: false,
        cross_project: true,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn network_sensitive_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::OpenCode,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: true,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn low_risk_free_write_task_can_auto_start() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::OpenCode,
        writes_files: true,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::AutoStart);
}
