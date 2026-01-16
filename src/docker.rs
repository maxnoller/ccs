use std::path::PathBuf;
use std::process::{Command, Stdio};
use thiserror::Error;

use crate::config::Config;
use crate::git::GitContext;

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker is not installed or not in PATH")]
    DockerNotFound,

    #[error("Docker command failed: {0}")]
    CommandFailed(String),

    #[error("Failed to execute docker: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dockerfile not found at: {0}")]
    DockerfileNotFound(PathBuf),

}

pub struct DockerRunner {
    config: Config,
    git_context: GitContext,
    mcp_config_path: Option<PathBuf>,
}

impl DockerRunner {
    /// Create a new Docker runner
    pub fn new(
        config: &Config,
        git_context: &GitContext,
        mcp_config_path: Option<PathBuf>,
    ) -> Result<Self, DockerError> {
        // Verify docker is available
        which::which("docker").map_err(|_| DockerError::DockerNotFound)?;

        Ok(DockerRunner {
            config: config.clone(),
            git_context: git_context.clone(),
            mcp_config_path,
        })
    }

    /// Build the Docker image
    pub fn build_image(config: &Config) -> anyhow::Result<()> {
        which::which("docker").map_err(|_| DockerError::DockerNotFound)?;

        // Find Dockerfile
        let dockerfile_path = config
            .docker
            .dockerfile_path
            .clone()
            .or_else(|| {
                // Look in common locations
                let candidates = [
                    PathBuf::from("docker/Dockerfile"),
                    PathBuf::from("Dockerfile"),
                ];
                candidates.into_iter().find(|p| p.exists())
            })
            .ok_or_else(|| {
                DockerError::DockerfileNotFound(PathBuf::from("docker/Dockerfile"))
            })?;

        if !dockerfile_path.exists() {
            return Err(DockerError::DockerfileNotFound(dockerfile_path).into());
        }

        let default_dir = PathBuf::from(".");
        let dockerfile_dir = dockerfile_path.parent().unwrap_or(&default_dir);

        println!("Building image {} from {}...", config.docker.image, dockerfile_path.display());

        let status = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(&config.docker.image)
            .arg("-f")
            .arg(&dockerfile_path)
            .arg(dockerfile_dir)
            .status()?;

        if !status.success() {
            return Err(DockerError::CommandFailed("docker build failed".to_string()).into());
        }

        println!("Successfully built image: {}", config.docker.image);
        Ok(())
    }

    /// Run the Docker container with Claude Code
    pub fn run(&self, extra_args: &[String]) -> anyhow::Result<()> {
        let mut cmd = Command::new("docker");

        cmd.arg("run")
            .arg("--rm")
            .arg("-it")
            .arg("--name")
            .arg(format!("ccs-{}", self.git_context.repo_name));

        // Add volume mounts for git context
        for (host_path, container_path) in self.git_context.docker_mounts() {
            cmd.arg("-v").arg(format!(
                "{}:{}",
                host_path.display(),
                container_path
            ));
        }

        // Mount Claude auth directory (read-only)
        if let Some(home) = dirs::home_dir() {
            let claude_dir = home.join(".claude");
            if claude_dir.exists() {
                cmd.arg("-v").arg(format!(
                    "{}:/home/{}/.claude:ro",
                    claude_dir.display(),
                    self.config.docker.user
                ));
            }
        }

        // Mount MCP config if available
        if let Some(ref mcp_path) = self.mcp_config_path {
            cmd.arg("-v").arg(format!(
                "{}:/home/{}/.claude.json:ro",
                mcp_path.display(),
                self.config.docker.user
            ));
        }

        // Add extra volumes from config
        for (host, container) in &self.config.docker.extra_volumes {
            let expanded_host = shellexpand::tilde(host);
            cmd.arg("-v").arg(format!("{}:{}", expanded_host, container));
        }

        // Add environment variables from config
        for (key, value) in &self.config.docker.extra_env {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }

        // Set working directory
        cmd.arg("-w").arg(&self.config.docker.workdir);

        // Use the configured image
        cmd.arg(&self.config.docker.image);

        // Add any extra arguments for Claude
        for arg in extra_args {
            cmd.arg(arg);
        }

        // Set up proper TTY handling
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        println!("Starting Claude Code sandbox...");
        println!("Workspace: {}", self.git_context.workspace_path.display());
        if self.git_context.is_worktree {
            println!("(Running in git worktree)");
        }
        println!();

        let status = cmd.status()?;

        if !status.success() {
            if let Some(code) = status.code() {
                std::process::exit(code);
            }
            return Err(DockerError::CommandFailed("Container exited with error".to_string()).into());
        }

        Ok(())
    }
}

// Need shellexpand for ~ expansion in volume paths
mod shellexpand {
    pub fn tilde(path: &str) -> std::borrow::Cow<'_, str> {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return std::borrow::Cow::Owned(format!("{}{}", home.display(), &path[1..]));
            }
        }
        std::borrow::Cow::Borrowed(path)
    }
}
