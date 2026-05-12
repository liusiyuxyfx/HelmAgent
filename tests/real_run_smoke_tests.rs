use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[cfg(unix)]
fn fake_helm_agent(path: &std::path::Path, log_path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
            log_path.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn real_run_quickstart_documents_safe_and_real_paths() {
    let docs = fs::read_to_string("docs/quickstart-real-run.md").expect("quickstart doc exists");

    assert!(docs.contains("make real-run-dry-run"), "{docs}");
    assert!(
        docs.contains("HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux"),
        "{docs}"
    );
    assert!(
        docs.contains("HELM_AGENT_REAL_RUN_CONFIRM=1") && docs.contains("make real-run-acp"),
        "{docs}"
    );
    assert!(docs.contains("helm-agent acp agent check"), "{docs}");
    assert!(
        docs.contains("HELM_AGENT_REAL_RUN_HOME=\"$HELM_AGENT_HOME\""),
        "{docs}"
    );
    assert!(
        docs.contains("helm-agent board serve --host 127.0.0.1 --port 8765"),
        "{docs}"
    );
    assert!(docs.contains("helm-agent task review"), "{docs}");
}

#[test]
fn makefile_exposes_real_run_targets() {
    let makefile = fs::read_to_string("Makefile").expect("Makefile exists");

    assert!(makefile.contains("real-run-dry-run"), "{makefile}");
    assert!(makefile.contains("real-run-tmux"), "{makefile}");
    assert!(makefile.contains("real-run-acp"), "{makefile}");
    assert!(
        makefile.contains("scripts/real_run_smoke.sh --mode dry-run"),
        "{makefile}"
    );
    assert!(
        makefile.contains("scripts/real_run_smoke.sh --mode tmux"),
        "{makefile}"
    );
    assert!(
        makefile.contains("scripts/real_run_smoke.sh --mode acp"),
        "{makefile}"
    );
}

#[test]
fn real_run_script_keeps_real_dispatch_opt_in_and_cleanup_safe() {
    let script =
        fs::read_to_string("scripts/real_run_smoke.sh").expect("real run smoke script exists");

    assert!(script.contains("HELM_AGENT_REAL_RUN_CONFIRM"), "{script}");
    assert!(script.contains("+%Y%m%d%H%M%S"), "{script}");
    assert!(script.contains("mktemp -d"), "{script}");
    assert!(script.contains("trap cleanup EXIT"), "{script}");
    assert!(script.contains("task dispatch"), "{script}");
    assert!(script.contains("acp agent check"), "{script}");
    assert!(script.contains("task sync --all"), "{script}");
}

#[cfg(unix)]
#[test]
fn real_run_tmux_refuses_without_confirmation_before_invoking_helm() {
    let tmp = tempdir().unwrap();
    let fake = tmp.path().join("helm-agent");
    let log = tmp.path().join("helm.log");
    fake_helm_agent(&fake, &log);

    let output = Command::new("sh")
        .arg("scripts/real_run_smoke.sh")
        .arg("--mode")
        .arg("tmux")
        .env("HELM_AGENT_BIN", fake.to_string_lossy().to_string())
        .output()
        .expect("run real-run tmux script");

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Refusing real tmux dispatch"), "{stderr}");
    assert!(!log.exists(), "script invoked helm before confirmation");
}

#[cfg(unix)]
#[test]
fn real_run_acp_refuses_without_confirmation_before_invoking_helm() {
    let tmp = tempdir().unwrap();
    let fake = tmp.path().join("helm-agent");
    let log = tmp.path().join("helm.log");
    fake_helm_agent(&fake, &log);

    let output = Command::new("sh")
        .arg("scripts/real_run_smoke.sh")
        .arg("--mode")
        .arg("acp")
        .env("HELM_AGENT_BIN", fake.to_string_lossy().to_string())
        .output()
        .expect("run real-run acp script");

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Refusing real ACP dispatch"), "{stderr}");
    assert!(!log.exists(), "script invoked helm before confirmation");
}

#[cfg(unix)]
#[test]
fn confirmed_real_tmux_preserves_owned_state_for_review() {
    let tmp = tempdir().unwrap();
    let fake = tmp.path().join("helm-agent");
    let log = tmp.path().join("helm.log");
    fake_helm_agent(&fake, &log);

    let output = Command::new("sh")
        .arg("scripts/real_run_smoke.sh")
        .arg("--mode")
        .arg("tmux")
        .env("HELM_AGENT_BIN", fake.to_string_lossy().to_string())
        .env("HELM_AGENT_REAL_RUN_CONFIRM", "1")
        .env("HELM_AGENT_REAL_RUN_ID", "PM-20260512-REALTEST")
        .output()
        .expect("run confirmed real-run tmux script");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("kept HELM_AGENT_HOME="), "{stdout}");
    assert!(stdout.contains("kept project="), "{stdout}");
    let home = stdout
        .lines()
        .find_map(|line| line.strip_prefix("kept HELM_AGENT_HOME="))
        .expect("kept home line");
    let project = stdout
        .lines()
        .find_map(|line| line.strip_prefix("kept project="))
        .expect("kept project line");
    assert!(std::path::Path::new(home).exists(), "{stdout}");
    assert!(std::path::Path::new(project).exists(), "{stdout}");

    let log_content = fs::read_to_string(&log).unwrap();
    assert!(
        log_content.contains("task dispatch PM-20260512-REALTEST"),
        "{log_content}"
    );
    assert!(
        log_content.contains("--confirm --send-brief"),
        "{log_content}"
    );

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}
