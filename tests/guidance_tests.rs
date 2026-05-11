use helm_agent::guidance::{
    add_installed_project_guidance_include, add_project_guidance_include,
    ensure_installed_main_agent_template, render_main_agent_prompt_from_template, GuidanceFile,
    GuidanceRuntime, MAIN_AGENT_TEMPLATE_FILE,
};
use tempfile::tempdir;

#[test]
fn add_agents_include_is_idempotent() {
    let project = tempdir().unwrap();
    let template_path = project.path().join(".helm-agent/main-agent-template.md");

    let first =
        add_project_guidance_include(project.path(), GuidanceFile::Agents, &template_path).unwrap();
    let second =
        add_project_guidance_include(project.path(), GuidanceFile::Agents, &template_path).unwrap();

    assert_eq!(first, project.path().join("AGENTS.md"));
    assert_eq!(second, project.path().join("AGENTS.md"));

    let agents = std::fs::read_to_string(project.path().join("AGENTS.md")).unwrap();
    assert_eq!(
        agents
            .matches(&format!("@{}", template_path.display()))
            .count(),
        1,
        "{agents}"
    );
}

#[test]
fn add_claude_include_preserves_existing_content_and_is_idempotent() {
    let project = tempdir().unwrap();
    let claude_file = project.path().join("CLAUDE.md");
    std::fs::write(
        &claude_file,
        "# Local Claude Rules\n\nKeep existing guidance.\n",
    )
    .unwrap();
    let template_path = project.path().join(".helm-agent/main-agent-template.md");

    add_project_guidance_include(project.path(), GuidanceFile::Claude, &template_path).unwrap();
    add_project_guidance_include(project.path(), GuidanceFile::Claude, &template_path).unwrap();

    let claude = std::fs::read_to_string(claude_file).unwrap();
    assert!(claude.starts_with("# Local Claude Rules\n\nKeep existing guidance.\n"));
    assert_eq!(
        claude
            .matches(&format!("@{}", template_path.display()))
            .count(),
        1,
        "{claude}"
    );
}

#[test]
fn codex_prompt_contains_base_template_and_codex_specific_guidance() {
    let prompt = render_main_agent_prompt_from_template(
        "# HelmAgent Main-Agent Operating Template\n\nShared rules.",
        GuidanceRuntime::Codex,
    );

    assert!(prompt.contains("# HelmAgent Main-Agent Operating Template"));
    assert!(prompt.contains("Runtime target: codex"));
    assert!(prompt.contains("ask before dispatching Codex"));
    assert!(prompt.contains("helm-agent task dispatch --runtime codex --confirm"));
}

#[test]
fn all_prompt_names_every_supported_runtime() {
    let prompt = render_main_agent_prompt_from_template("Shared rules.", GuidanceRuntime::All);

    assert!(prompt.contains("Runtime target: all"));
    assert!(prompt.contains("claude"));
    assert!(prompt.contains("codex"));
    assert!(prompt.contains("opencode"));
}

#[test]
fn installed_template_is_bootstrapped_from_fallback_when_missing() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    std::env::set_var("HELM_AGENT_HOME", home.path());

    let installed = ensure_installed_main_agent_template().unwrap();
    assert_eq!(installed, home.path().join(MAIN_AGENT_TEMPLATE_FILE));
    assert!(installed.exists());

    add_installed_project_guidance_include(project.path(), GuidanceFile::Agents).unwrap();
    let agents = std::fs::read_to_string(project.path().join("AGENTS.md")).unwrap();
    assert!(
        agents.contains(&format!("@{}", installed.display())),
        "{agents}"
    );

    std::env::remove_var("HELM_AGENT_HOME");
}
