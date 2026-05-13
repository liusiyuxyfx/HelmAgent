use crate::guidance::{COORDINATOR_SKILL_FILE, MAIN_AGENT_TEMPLATE_FILE};
use crate::paths::helm_agent_home;
use anyhow::{anyhow, bail, Context, Result};
use directories::BaseDirs;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const HELM_AGENT_BIN_DIR_ENV: &str = "HELM_AGENT_BIN_DIR";
const HELM_AGENT_CARGO_ROOT_ENV: &str = "HELM_AGENT_CARGO_ROOT";
const CARGO_INSTALL_ROOT_ENV: &str = "CARGO_INSTALL_ROOT";

pub fn print_install_doctor() -> Result<()> {
    let mut status = DoctorStatus { ok: true };
    let home = diagnostic_home()?;
    let bin_dir = helm_agent_bin_dir()?;

    println!("doctor: checking HelmAgent installation");
    for tool in ["cargo", "git", "rustc", "helm-agent"] {
        status.report_tool(tool);
    }

    status.report_home(&home);
    status.report_file("env", &home.join("env"));
    status.report_file("template", &home.join(MAIN_AGENT_TEMPLATE_FILE));
    status.report_file("coordinator skill", &home.join(COORDINATOR_SKILL_FILE));
    status.report_path_contains(&bin_dir);
    status.report_task_board(&home);

    if status.ok {
        Ok(())
    } else {
        bail!("HelmAgent doctor found missing or failed checks")
    }
}

struct DoctorStatus {
    ok: bool,
}

impl DoctorStatus {
    fn report_tool(&mut self, tool: &str) {
        if command_exists(tool) {
            println!("ok: {tool}");
        } else {
            println!("missing: {tool}");
            self.ok = false;
        }
    }

    fn report_home(&mut self, path: &Path) {
        if !path.is_absolute() {
            println!(
                "missing: HELM_AGENT_HOME must be absolute: {}",
                path.display()
            );
            self.ok = false;
            return;
        }
        if path.is_dir() {
            println!("ok: HELM_AGENT_HOME {}", path.display());
        } else {
            println!("missing: HELM_AGENT_HOME {}", path.display());
            self.ok = false;
        }
    }

    fn report_file(&mut self, label: &str, path: &Path) {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                println!("failed: {label} {} is a symlink", path.display());
                self.ok = false;
            }
            Ok(metadata) if metadata.is_file() => {
                println!("ok: {label} {}", path.display());
            }
            Ok(_) => {
                println!("failed: {label} {} is not a file", path.display());
                self.ok = false;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                println!("missing: {label} {}", path.display());
                self.ok = false;
            }
            Err(error) => {
                println!("failed: {label} {} ({error})", path.display());
                self.ok = false;
            }
        }
    }

    fn report_path_contains(&mut self, bin_dir: &Path) {
        if path_contains(bin_dir) {
            println!("ok: PATH contains {}", bin_dir.display());
        } else {
            println!("missing: PATH does not contain {}", bin_dir.display());
            self.ok = false;
        }
    }

    fn report_task_board(&mut self, home: &Path) {
        if !home.is_absolute() {
            println!("failed: helm-agent task board (HELM_AGENT_HOME must be absolute)");
            self.ok = false;
            return;
        }
        if !home.is_dir() {
            println!("failed: helm-agent task board (HELM_AGENT_HOME is missing)");
            self.ok = false;
            return;
        }

        match run_helm_agent_task_board(home) {
            Ok(()) => println!("ok: helm-agent task board"),
            Err(error) => {
                println!("failed: helm-agent task board ({error})");
                self.ok = false;
            }
        }
    }
}

fn run_helm_agent_task_board(home: &Path) -> Result<()> {
    let output = Command::new("helm-agent")
        .arg("task")
        .arg("board")
        .env("HELM_AGENT_HOME", home)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("execute helm-agent task board")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("{}", output.status);
        } else {
            bail!("{stderr}");
        }
    }
}

fn diagnostic_home() -> Result<PathBuf> {
    let home = helm_agent_home()?;
    if !home.is_absolute() {
        return Ok(home);
    }
    if home.exists() {
        return Ok(home.canonicalize()?);
    }
    Ok(home)
}

fn helm_agent_bin_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os(HELM_AGENT_BIN_DIR_ENV) {
        return Ok(PathBuf::from(path));
    }

    let cargo_root = env::var_os(HELM_AGENT_CARGO_ROOT_ENV)
        .or_else(|| env::var_os(CARGO_INSTALL_ROOT_ENV))
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_cargo_root)?;
    Ok(cargo_root.join("bin"))
}

fn default_cargo_root() -> Result<PathBuf> {
    let dirs = BaseDirs::new().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(dirs.home_dir().join(".cargo"))
}

fn path_contains(bin_dir: &Path) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|entry| entry == bin_dir)
}

fn command_exists(executable: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "command -v {}",
            shell_quote_for_process(executable)
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn shell_quote_for_process(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}
