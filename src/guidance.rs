use crate::paths::helm_agent_home;
use anyhow::{bail, Context, Result};
use std::fs;
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
    Ok(helm_agent_home()?.join(MAIN_AGENT_TEMPLATE_FILE))
}

pub fn fallback_main_agent_template_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FALLBACK_MAIN_AGENT_TEMPLATE)
}

pub fn main_agent_template_path() -> Result<PathBuf> {
    let installed = installed_main_agent_template_path()?;
    if installed
        .try_exists()
        .with_context(|| format!("check installed template {}", installed.display()))?
    {
        return Ok(installed);
    }

    Ok(fallback_main_agent_template_path())
}

pub fn read_main_agent_template() -> Result<String> {
    let installed = installed_main_agent_template_path()?;
    if installed
        .try_exists()
        .with_context(|| format!("check installed template {}", installed.display()))?
    {
        return fs::read_to_string(&installed)
            .with_context(|| format!("read main-agent template {}", installed.display()));
    }

    Ok(BUNDLED_MAIN_AGENT_TEMPLATE.to_string())
}

pub fn ensure_installed_main_agent_template() -> Result<PathBuf> {
    let installed = installed_main_agent_template_path()?;
    if installed
        .try_exists()
        .with_context(|| format!("check installed template {}", installed.display()))?
    {
        return Ok(installed);
    }

    if let Some(parent) = installed.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create template directory {}", parent.display()))?;
    }
    fs::write(&installed, BUNDLED_MAIN_AGENT_TEMPLATE)
        .with_context(|| format!("write installed template {}", installed.display()))?;
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
    let mut content = if target_path
        .try_exists()
        .with_context(|| format!("check guidance file {}", target_path.display()))?
    {
        fs::read_to_string(&target_path)
            .with_context(|| format!("read guidance file {}", target_path.display()))?
    } else {
        String::new()
    };

    if content.lines().any(|line| line == include_line) {
        return Ok(target_path);
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

    fs::write(&target_path, content)
        .with_context(|| format!("write guidance file {}", target_path.display()))?;
    Ok(target_path)
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

fn runtime_guidance(runtime: GuidanceRuntime) -> &'static str {
    match runtime {
        GuidanceRuntime::Claude => {
            "## Runtime-Specific Guidance\n\nRuntime target: claude\n\n- Prefer Claude for low-risk free-agent delegation when it fits the task.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime claude <id>`.\n- Use `helm-agent task dispatch --runtime claude --confirm <id>` only after approval when policy requires confirmation.\n- Report the HelmAgent task status, attach command, resume command, and review state."
        }
        GuidanceRuntime::Codex => {
            "## Runtime-Specific Guidance\n\nRuntime target: codex\n\n- Start by running `helm-agent task board` before reporting task status.\n- Always ask before dispatching Codex unless the user already approved Codex for this task or workspace.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime codex <id>`.\n- Real Codex dispatch requires confirmation: `helm-agent task dispatch --runtime codex --confirm <id>`.\n- Report the HelmAgent task status, attach command, resume command, and review state."
        }
        GuidanceRuntime::OpenCode => {
            "## Runtime-Specific Guidance\n\nRuntime target: opencode\n\n- Prefer OpenCode for low-risk free-agent delegation when it fits the task.\n- Preview before dispatch: `helm-agent task dispatch --dry-run --runtime opencode <id>`.\n- Use `helm-agent task dispatch --runtime opencode --confirm <id>` only after approval when policy requires confirmation.\n- Report the HelmAgent task status, attach command, resume command, and review state."
        }
        GuidanceRuntime::All => {
            "## Runtime-Specific Guidance\n\nRuntime target: all\n\n- Supported runtimes: claude, codex, opencode.\n- Prefer free runtimes first: claude or opencode.\n- Ask before dispatching Codex unless the user already approved Codex for this task or workspace.\n- Preview with `helm-agent task dispatch --dry-run --runtime <claude|codex|opencode> <id>` before starting any child agent.\n- Report the HelmAgent task status, attach command, resume command, and review state."
        }
    }
}
