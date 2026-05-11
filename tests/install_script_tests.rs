use std::process::Command;
use tempfile::tempdir;

fn run_install_script(args: &[&str]) -> (bool, String, String) {
    run_install_script_with_env(args, &[])
}

fn run_install_script_with_env(args: &[&str], envs: &[(&str, &str)]) -> (bool, String, String) {
    let output = Command::new("sh")
        .arg("install.sh")
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .expect("run install.sh");
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn install_dry_run_prints_install_steps() {
    let (success, stdout, stderr) = run_install_script(&["install", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("DRY-RUN"), "{stdout}");
    assert!(
        stdout.contains(
            "cargo install --git https://github.com/liusiyuxyfx/HelmAgent.git --locked --force"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("write env"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(stdout.contains("main-agent-template.md"), "{stdout}");
    assert!(stdout.contains("helm-agent task board"), "{stdout}");
}

#[test]
fn update_dry_run_reinstalls_without_data_deletion() {
    let (success, stdout, stderr) = run_install_script(&["update", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("cargo install --git"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(!stdout.contains("remove data"), "{stdout}");
}

#[test]
fn repair_dry_run_recreates_env_and_runs_doctor() {
    let (success, stdout, stderr) = run_install_script(&["repair", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("write env"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(stdout.contains("doctor"), "{stdout}");
}

#[test]
fn uninstall_dry_run_keeps_data_by_default() {
    let (success, stdout, stderr) = run_install_script(&["uninstall", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("cargo uninstall helm-agent"), "{stdout}");
    assert!(stdout.contains("keep data"), "{stdout}");
    assert!(!stdout.contains("remove data"), "{stdout}");
}

#[test]
fn uninstall_purge_dry_run_reports_data_removal() {
    let (success, stdout, stderr) = run_install_script(&["uninstall", "--purge", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("cargo uninstall helm-agent"), "{stdout}");
    assert!(stdout.contains("remove data"), "{stdout}");
}

#[test]
fn uninstall_purge_refuses_unsafe_home_before_mutation() {
    let (success, stdout, stderr) =
        run_install_script_with_env(&["uninstall", "--purge"], &[("HELM_AGENT_HOME", "/")]);

    assert!(!success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("refusing to purge unsafe HELM_AGENT_HOME"),
        "{stdout}"
    );
    assert!(!stdout.contains("cargo uninstall helm-agent"), "{stdout}");
}

#[test]
fn init_project_dry_run_prints_agents_target_and_template_include() {
    let project = tempdir().unwrap();
    let project_path = project.path().to_string_lossy().to_string();
    let (success, stdout, stderr) =
        run_install_script(&["init-project", project_path.as_str(), "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("AGENTS.md"), "{stdout}");
    assert!(
        stdout.contains(".helm-agent/main-agent-template.md"),
        "{stdout}"
    );
}

#[test]
fn init_project_writes_agents_include_and_installs_template() {
    let project = tempdir().unwrap();
    let home = tempdir().unwrap();
    let project_path = project.path().to_string_lossy().to_string();
    let home_path = home.path().to_string_lossy().to_string();

    let (success, stdout, stderr) = run_install_script_with_env(
        &["init-project", project_path.as_str()],
        &[("HELM_AGENT_HOME", home_path.as_str())],
    );

    assert!(success, "{stdout}\n{stderr}");
    let template_path = home.path().join("main-agent-template.md");
    let agents_file = project.path().join("AGENTS.md");
    assert!(template_path.exists(), "{stdout}");
    assert!(agents_file.exists(), "{stdout}");

    let agents = std::fs::read_to_string(agents_file).unwrap();
    assert!(
        agents.contains(&format!("@{}", template_path.to_string_lossy())),
        "{agents}"
    );

    let template = std::fs::read_to_string(template_path).unwrap();
    assert!(template.contains("HelmAgent"), "{template}");
}

#[test]
fn unknown_command_fails_with_usage() {
    let (success, stdout, stderr) = run_install_script(&["unknown"]);

    assert!(!success, "{stdout}\n{stderr}");
    assert!(stderr.contains("Usage:"), "{stderr}");
}

#[test]
fn install_docs_cover_core_lifecycle_commands() {
    let readme = std::fs::read_to_string("README.md").unwrap();
    let guide = std::fs::read_to_string("docs/install.md").unwrap();
    let combined = format!("{readme}\n{guide}");

    for required in [
        "install.sh | sh -s -- install",
        "install.sh | sh -s -- update",
        "install.sh | sh -s -- repair",
        "install.sh | sh -s -- doctor",
        "install.sh | sh -s -- uninstall",
        "install --dry-run",
        "uninstall --purge",
        "init-project",
        "HELM_AGENT_HOME",
        "HELM_AGENT_TEMPLATE_URL",
        "main-agent-template.md",
    ] {
        assert!(
            combined.contains(required),
            "missing `{required}` from install docs"
        );
    }
}
