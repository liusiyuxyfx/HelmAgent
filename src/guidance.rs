use crate::paths::canonical_helm_agent_home;
use anyhow::{bail, Context, Result};
use std::fs::{self, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAIN_AGENT_TEMPLATE_FILE: &str = "main-agent-template.md";
pub const FALLBACK_MAIN_AGENT_TEMPLATE: &str = "docs/agent-integrations/main-agent-template.md";
pub const BUNDLED_MAIN_AGENT_TEMPLATE: &str =
    include_str!("../docs/agent-integrations/main-agent-template.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceRuntime {
    Claude,
    Codex,
    OpenCode,
    All,
}

impl GuidanceRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            GuidanceRuntime::Claude => "claude",
            GuidanceRuntime::Codex => "codex",
            GuidanceRuntime::OpenCode => "opencode",
            GuidanceRuntime::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceFile {
    Agents,
    Claude,
}

impl GuidanceFile {
    pub fn file_name(self) -> &'static str {
        match self {
            GuidanceFile::Agents => "AGENTS.md",
            GuidanceFile::Claude => "CLAUDE.md",
        }
    }
}

impl From<&GuidanceFile> for GuidanceFile {
    fn from(value: &GuidanceFile) -> Self {
        *value
    }
}

pub fn installed_main_agent_template_path() -> Result<PathBuf> {
    Ok(canonical_helm_agent_home()?.join(MAIN_AGENT_TEMPLATE_FILE))
}

pub fn fallback_main_agent_template_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FALLBACK_MAIN_AGENT_TEMPLATE)
}

pub fn main_agent_template_path() -> Result<PathBuf> {
    let installed = installed_main_agent_template_path()?;
    if installed_template_exists(&installed)? {
        return Ok(installed);
    }

    Ok(fallback_main_agent_template_path())
}

pub fn read_main_agent_template() -> Result<String> {
    let installed = installed_main_agent_template_path()?;
    if installed_template_exists(&installed)? {
        return read_existing_template_file(&installed);
    }

    Ok(BUNDLED_MAIN_AGENT_TEMPLATE.to_string())
}

pub fn ensure_installed_main_agent_template() -> Result<PathBuf> {
    let installed = installed_main_agent_template_path()?;
    if installed_template_exists(&installed)? {
        return Ok(installed);
    }

    write_new_installed_template(&installed)?;
    Ok(installed)
}

pub fn render_main_agent_prompt(runtime: GuidanceRuntime) -> Result<String> {
    Ok(render_main_agent_prompt_from_template(
        &read_main_agent_template()?,
        runtime,
    ))
}

pub fn render_main_agent_prompt_from_template(template: &str, runtime: GuidanceRuntime) -> String {
    let mut prompt = format!("Runtime: {}\n\n", runtime.as_str());
    prompt.push_str(template.trim_end());
    prompt.push_str("\n\n");
    prompt.push_str(runtime_guidance(runtime));
    prompt
}

pub fn add_installed_project_guidance_include(
    project_path: impl AsRef<Path>,
    guidance_file: impl Into<GuidanceFile>,
) -> Result<PathBuf> {
    let template_path = ensure_installed_main_agent_template()?;
    add_project_guidance_include(project_path, guidance_file, &template_path)
}

pub fn add_project_guidance_include(
    project_path: impl AsRef<Path>,
    guidance_file: impl Into<GuidanceFile>,
    template_path: impl AsRef<Path>,
) -> Result<PathBuf> {
    let project_path = project_path.as_ref();
    let guidance_file = guidance_file.into();
    fs::create_dir_all(project_path)
        .with_context(|| format!("create project directory {}", project_path.display()))?;

    let target_path = project_path.join(guidance_file.file_name());
    reject_symlink_guidance_file(&target_path)?;
    let include_line = format!("@{}", template_path.as_ref().display());
    update_guidance_file_no_follow(&target_path, &include_line)?;
    Ok(target_path)
}

fn update_guidance_file_no_follow(target_path: &Path, include_line: &str) -> Result<()> {
    let mut file = open_guidance_file_no_follow(target_path)
        .with_context(|| format!("open guidance file {}", target_path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect guidance file {}", target_path.display()))?;
    if !metadata.file_type().is_file() {
        bail!(
            "refuse to update non-file guidance file {}",
            target_path.display()
        );
    }

    let mut content = String::new();
    file.read_to_string(&mut content)
        .with_context(|| format!("read guidance file {}", target_path.display()))?;

    if content.lines().any(|line| line == include_line) {
        return Ok(());
    }

    if content.is_empty() {
        content.push_str(&include_line);
        content.push('\n');
    } else {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        content.push_str(&include_line);
        content.push('\n');
    }

    file.set_len(0)
        .with_context(|| format!("truncate guidance file {}", target_path.display()))?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("rewind guidance file {}", target_path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("write guidance file {}", target_path.display()))?;
    Ok(())
}

fn reject_symlink_guidance_file(target_path: &Path) -> Result<()> {
    match fs::symlink_metadata(target_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "refuse to update symlink guidance file {}",
                target_path.display()
            )
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("inspect guidance file {}", target_path.display()))
        }
    }
}

#[cfg(unix)]
fn open_guidance_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_guidance_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
}

fn installed_template_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_installed_template_file(path, &metadata)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("inspect main-agent template {}", path.display()))
        }
    }
}

fn validate_installed_template_file(path: &Path, metadata: &Metadata) -> Result<()> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        bail!(
            "refuse to use symlink main-agent template {}",
            path.display()
        );
    }
    if !file_type.is_file() {
        bail!(
            "refuse to use non-file main-agent template {}",
            path.display()
        );
    }

    let home = canonical_helm_agent_home()?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("canonicalize main-agent template {}", path.display()))?;
    if !canonical.starts_with(&home) {
        bail!(
            "refuse to use main-agent template outside HelmAgent home: {}",
            canonical.display()
        );
    }

    Ok(())
}

fn write_new_installed_template(path: &Path) -> Result<()> {
    match create_new_template_file(path) {
        Ok(mut file) => file
            .write_all(BUNDLED_MAIN_AGENT_TEMPLATE.as_bytes())
            .with_context(|| format!("write installed template {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            installed_template_exists(path)?;
            Ok(())
        }
        Err(error) => {
            Err(error).with_context(|| format!("create installed template {}", path.display()))
        }
    }
}

fn read_existing_template_file(path: &Path) -> Result<String> {
    let mut file = open_template_file_no_follow(path)
        .with_context(|| format!("open main-agent template {}", path.display()))?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .with_context(|| format!("read main-agent template {}", path.display()))?;
    Ok(content)
}

#[cfg(unix)]
fn open_template_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_template_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(unix)]
fn create_new_template_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn create_new_template_file(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn runtime_guidance(runtime: GuidanceRuntime) -> &'static str {
    match runtime {
        GuidanceRuntime::Claude => {
            "## Runtime-Specific Guidance\n\nRuntime target: claude\n\n- Prefer Claude for low-risk free-agent delegation when it fits the task.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime claude <id>`.\n- Use `helm-agent task brief <id>` when preparing a child-agent handoff.\n- Use `helm-agent task dispatch --runtime claude --confirm <id>` only after approval when policy requires confirmation.\n- Report the HelmAgent task status, attach command, resume command, brief path, and review state."
        }
        GuidanceRuntime::Codex => {
            "## Runtime-Specific Guidance\n\nRuntime target: codex\n\n- Start by running `helm-agent task board` before reporting task status.\n- Always ask before dispatching Codex unless the user already approved Codex for this task or workspace.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime codex <id>`.\n- Use `helm-agent task brief <id>` when preparing a child-agent handoff.\n- Real Codex dispatch requires confirmation: `helm-agent task dispatch --runtime codex --confirm <id>`.\n- Report the HelmAgent task status, attach command, resume command, brief path, and review state."
        }
        GuidanceRuntime::OpenCode => {
            "## Runtime-Specific Guidance\n\nRuntime target: opencode\n\n- Prefer OpenCode for low-risk free-agent delegation when it fits the task.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime opencode <id>`.\n- Use `helm-agent task brief <id>` when preparing a child-agent handoff.\n- Use `helm-agent task dispatch --runtime opencode --confirm <id>` only after approval when policy requires confirmation.\n- Report the HelmAgent task status, attach command, resume command, brief path, and review state."
        }
        GuidanceRuntime::All => {
            "## Runtime-Specific Guidance\n\nRuntime target: all\n\n- Supported runtimes: claude, codex, opencode.\n- Prefer free runtimes first: claude or opencode.\n- Ask before dispatching Codex unless the user already approved Codex for this task or workspace.\n- Preview with `helm-agent task dispatch --dry-run --runtime <claude|codex|opencode> <id>` before starting any child agent.\n- Use `helm-agent task brief <id>` when preparing a child-agent handoff.\n- Report the HelmAgent task status, attach command, resume command, brief path, and review state."
        }
    }
}
