use std::fs::File;
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[cfg(unix)]
fn fake_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

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

fn run_install_script_from_stdin(args: &[&str]) -> (bool, String, String) {
    let output = Command::new("sh")
        .arg("-s")
        .arg("--")
        .args(args)
        .stdin(Stdio::from(File::open("install.sh").unwrap()))
        .output()
        .expect("run install.sh from stdin");
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
    assert!(stdout.contains("cargo install --path"), "{stdout}");
    assert!(stdout.contains("--locked --force --root"), "{stdout}");
    assert!(stdout.contains("write env"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(stdout.contains("main-agent-template.md"), "{stdout}");
    assert!(
        stdout.contains("helm-agent project init --path /path/to/project --agent all"),
        "{stdout}"
    );
    assert!(
        stdout.contains("helm-agent agent prompt --runtime codex"),
        "{stdout}"
    );
    assert!(
        stdout.contains("helm-agent board serve --host 127.0.0.1 --port 8765"),
        "{stdout}"
    );
    assert!(stdout.contains("helm-agent task board"), "{stdout}");
}

#[test]
fn install_dry_run_uses_git_when_repo_override_is_set() {
    let (success, stdout, stderr) = run_install_script_with_env(
        &["install", "--dry-run"],
        &[("HELM_AGENT_REPO", "https://example.invalid/HelmAgent.git")],
    );

    assert!(success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains(
            "cargo install --git https://example.invalid/HelmAgent.git --locked --force --root"
        ),
        "{stdout}"
    );
}

#[test]
fn stdin_install_dry_run_uses_git_not_local_checkout() {
    let (success, stdout, stderr) = run_install_script_from_stdin(&["install", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("cargo install --git"), "{stdout}");
    assert!(!stdout.contains("cargo install --path"), "{stdout}");
}

#[test]
fn install_dry_run_uses_custom_cargo_root_for_install_and_path() {
    let root = tempdir().unwrap();
    let root_path = root.path().to_string_lossy().to_string();

    let (success, stdout, stderr) = run_install_script_with_env(
        &["install", "--dry-run"],
        &[("HELM_AGENT_CARGO_ROOT", root_path.as_str())],
    );

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains(&format!("--root {root_path}")), "{stdout}");
}

#[test]
fn update_dry_run_reinstalls_without_data_deletion() {
    let (success, stdout, stderr) = run_install_script(&["update", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("cargo install --path"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(!stdout.contains("remove data"), "{stdout}");
}

#[test]
fn repair_dry_run_recreates_env_and_runs_doctor() {
    let (success, stdout, stderr) = run_install_script(&["repair", "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(stdout.contains("write env"), "{stdout}");
    assert!(stdout.contains("install template"), "{stdout}");
    assert!(stdout.contains("cargo install --path"), "{stdout}");
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
fn uninstall_purge_refuses_parent_alias_even_with_custom_confirmation() {
    let home_alias = format!("{}/..", std::env::var("HOME").unwrap());
    let (success, stdout, stderr) = run_install_script_with_env(
        &["uninstall", "--purge"],
        &[
            ("HELM_AGENT_HOME", home_alias.as_str()),
            ("HELM_AGENT_ALLOW_CUSTOM_PURGE", "1"),
        ],
    );

    assert!(!success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("refusing to purge unsafe HELM_AGENT_HOME"),
        "{stdout}"
    );
    assert!(!stdout.contains("cargo uninstall helm-agent"), "{stdout}");
}

#[test]
fn uninstall_purge_refuses_relative_home_even_with_custom_confirmation() {
    let (success, stdout, stderr) = run_install_script_with_env(
        &["uninstall", "--purge"],
        &[
            ("HELM_AGENT_HOME", "relative-helm-agent"),
            ("HELM_AGENT_ALLOW_CUSTOM_PURGE", "1"),
        ],
    );

    assert!(!success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("refusing to purge unsafe HELM_AGENT_HOME"),
        "{stdout}"
    );
    assert!(!stdout.contains("cargo uninstall helm-agent"), "{stdout}");
}

#[test]
fn uninstall_purge_refuses_custom_home_without_extra_confirmation() {
    let home = tempdir().unwrap();
    let home_path = home.path().to_string_lossy().to_string();

    let (success, stdout, stderr) = run_install_script_with_env(
        &["uninstall", "--purge"],
        &[("HELM_AGENT_HOME", home_path.as_str())],
    );

    assert!(!success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("refusing to purge custom HELM_AGENT_HOME"),
        "{stdout}"
    );
    assert!(!stdout.contains("cargo uninstall helm-agent"), "{stdout}");
}

#[test]
fn doctor_dry_run_does_not_execute_helm_agent() {
    let bin_dir = tempdir().unwrap();
    let sentinel = bin_dir.path().join("executed");
    let fake_helm_agent = bin_dir.path().join("helm-agent");
    std::fs::write(
        &fake_helm_agent,
        format!(
            "#!/bin/sh\nprintf executed > '{}'\n",
            sentinel.to_string_lossy()
        ),
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&fake_helm_agent).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_helm_agent, perms).unwrap();
    }

    let existing_path = std::env::var("PATH").unwrap();
    let path = format!("{}:{existing_path}", bin_dir.path().to_string_lossy());
    let (success, stdout, stderr) =
        run_install_script_with_env(&["doctor", "--dry-run"], &[("PATH", path.as_str())]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("DRY-RUN: helm-agent task board"),
        "{stdout}"
    );
    assert!(!sentinel.exists(), "{stdout}");
}

#[test]
fn init_project_dry_run_prints_safe_cli_delegation() {
    let project = tempdir().unwrap();
    let project_path = project.path().to_string_lossy().to_string();
    let (success, stdout, stderr) =
        run_install_script(&["init-project", project_path.as_str(), "--dry-run"]);

    assert!(success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains(&format!(
            "helm-agent project init --path {project_path} --agent codex"
        )),
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
    let template_path = home
        .path()
        .canonicalize()
        .unwrap()
        .join("main-agent-template.md");
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

#[cfg(unix)]
#[test]
fn install_refuses_symlink_template_target() {
    use std::os::unix::fs::symlink;

    let home = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let bin = tempdir().unwrap();
    let outside_template = outside.path().join("main-agent-template.md");
    std::fs::write(&outside_template, "external template\n").unwrap();
    symlink(
        &outside_template,
        home.path().join("main-agent-template.md"),
    )
    .unwrap();

    for tool in ["cargo", "git", "rustc"] {
        fake_executable(&bin.path().join(tool));
    }

    let existing_path = std::env::var("PATH").unwrap();
    let path = format!("{}:{existing_path}", bin.path().to_string_lossy());
    let home_path = home.path().to_string_lossy().to_string();
    let (success, stdout, stderr) = run_install_script_with_env(
        &["install"],
        &[
            ("HELM_AGENT_HOME", home_path.as_str()),
            ("PATH", path.as_str()),
        ],
    );

    assert!(!success, "{stdout}\n{stderr}");
    assert!(
        stdout.contains("refusing to update symlink template"),
        "{stdout}"
    );
    assert_eq!(
        std::fs::read_to_string(outside_template).unwrap(),
        "external template\n"
    );
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
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" install",
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" update",
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" repair",
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" doctor",
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" uninstall",
        "-o \"$INSTALLER\" && sh \"$INSTALLER\" init-project",
        "helm-agent project init --path /path/to/project --agent all",
        "helm-agent agent prompt --runtime codex",
        "helm-agent board serve --host 127.0.0.1 --port 8765",
        "install --dry-run",
        "uninstall --purge",
        "init-project",
        "HELM_AGENT_HOME",
        "HELM_AGENT_CARGO_ROOT",
        "HELM_AGENT_TEMPLATE_URL",
        "main-agent-template.md",
        "@$HOME/.helm-agent/main-agent-template.md",
    ] {
        assert!(
            combined.contains(required),
            "missing `{required}` from install docs"
        );
    }
}
