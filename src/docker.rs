use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::config::Config;
use crate::git::GitContext;

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Neither Docker nor Podman found in PATH")]
    RuntimeNotFound,

    #[error("Docker/Podman command failed: {0}")]
    CommandFailed(String),

    #[error("Failed to execute command: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dockerfile not found at: {0}")]
    DockerfileNotFound(PathBuf),
}

/// Container runtime (Docker or Podman)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerRuntime {
    Docker,
    Podman,
}

impl ContainerRuntime {
    /// Detect available container runtime, preferring Podman
    pub fn detect() -> Result<Self, DockerError> {
        if which::which("podman").is_ok() {
            Ok(ContainerRuntime::Podman)
        } else if which::which("docker").is_ok() {
            Ok(ContainerRuntime::Docker)
        } else {
            Err(DockerError::RuntimeNotFound)
        }
    }

    /// Get the command name
    pub fn command(&self) -> &'static str {
        match self {
            ContainerRuntime::Docker => "docker",
            ContainerRuntime::Podman => "podman",
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            ContainerRuntime::Docker => "Docker",
            ContainerRuntime::Podman => "Podman",
        }
    }
}

pub struct DockerRunner {
    runtime: ContainerRuntime,
    config: Config,
    git_context: GitContext,
    mcp_config_path: Option<PathBuf>,
    container_name: String,
}

impl DockerRunner {
    /// Create a new Docker/Podman runner
    pub fn new(
        config: &Config,
        git_context: &GitContext,
        mcp_config_path: Option<PathBuf>,
    ) -> Result<Self, DockerError> {
        let runtime = ContainerRuntime::detect()?;
        let container_name = generate_container_name(&git_context.repo_name);

        Ok(DockerRunner {
            runtime,
            config: config.clone(),
            git_context: git_context.clone(),
            mcp_config_path,
            container_name,
        })
    }

    /// Build the container image
    pub fn build_image(config: &Config) -> anyhow::Result<()> {
        let runtime = ContainerRuntime::detect()?;

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
            .ok_or_else(|| DockerError::DockerfileNotFound(PathBuf::from("docker/Dockerfile")))?;

        if !dockerfile_path.exists() {
            return Err(DockerError::DockerfileNotFound(dockerfile_path).into());
        }

        let default_dir = PathBuf::from(".");
        let dockerfile_dir = dockerfile_path.parent().unwrap_or(&default_dir);

        println!(
            "Building image {} using {} from {}...",
            config.docker.image,
            runtime.name(),
            dockerfile_path.display()
        );

        let status = Command::new(runtime.command())
            .arg("build")
            .arg("-t")
            .arg(&config.docker.image)
            .arg("-f")
            .arg(&dockerfile_path)
            .arg(dockerfile_dir)
            .status()?;

        if !status.success() {
            return Err(
                DockerError::CommandFailed(format!("{} build failed", runtime.name())).into(),
            );
        }

        println!("Successfully built image: {}", config.docker.image);
        Ok(())
    }

    /// Run the container with Claude Code
    pub fn run(&self, extra_args: &[String]) -> anyhow::Result<()> {
        let mut cmd = Command::new(self.runtime.command());

        cmd.arg("run")
            .arg("--rm")
            .arg("-it")
            .arg("--name")
            .arg(&self.container_name);

        // Add resource limits
        if let Some(ref mem) = self.config.docker.memory_limit {
            cmd.arg("--memory").arg(mem);
        }
        if let Some(cpu) = self.config.docker.cpu_limit {
            cmd.arg("--cpus").arg(cpu.to_string());
        }

        // Load .env file from project if configured and exists
        let env_file_loaded = if self.config.docker.load_env_file {
            let env_path = self.git_context.workspace_path.join(&self.config.docker.env_file_path);
            if env_path.exists() {
                cmd.arg("--env-file").arg(&env_path);
                true
            } else {
                false
            }
        } else {
            false
        };

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
        println!("Runtime: {}", self.runtime.name());
        println!("Container: {}", self.container_name);
        println!("Workspace: {}", self.git_context.workspace_path.display());
        if self.git_context.is_worktree {
            println!("(Running in git worktree)");
        }
        if env_file_loaded {
            println!("Loaded .env: {}", self.config.docker.env_file_path);
        }
        if let Some(ref mem) = self.config.docker.memory_limit {
            println!("Memory limit: {}", mem);
        }
        if let Some(cpu) = self.config.docker.cpu_limit {
            println!("CPU limit: {}", cpu);
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

/// Generate a unique container name with timestamp
fn generate_container_name(repo_name: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Use last 6 digits for readability
    let short_ts = timestamp % 1_000_000;
    format!("ccs-{}-{}", repo_name, short_ts)
}

/// Status information about the container runtime environment
#[derive(Debug)]
pub struct RuntimeStatus {
    pub runtime: Option<ContainerRuntime>,
    pub runtime_version: Option<String>,
    pub image_exists: bool,
    pub running_containers: Vec<String>,
    pub config_path: Option<PathBuf>,
    pub config_exists: bool,
    pub mcp_config_path: Option<PathBuf>,
    pub mcp_config_exists: bool,
}

impl RuntimeStatus {
    /// Check the status of the container runtime environment
    pub fn check(config: &Config) -> Self {
        let runtime = ContainerRuntime::detect().ok();
        let runtime_version = runtime.and_then(|r| get_runtime_version(r));
        let image_exists = runtime
            .map(|r| check_image_exists(r, &config.docker.image))
            .unwrap_or(false);
        let running_containers = runtime
            .map(|r| list_ccs_containers(r))
            .unwrap_or_default();

        let config_path = Config::config_path().ok();
        let config_exists = config_path.as_ref().map(|p| p.exists()).unwrap_or(false);

        let mcp_config_path = Config::mcp_servers_path().ok();
        let mcp_config_exists = mcp_config_path.as_ref().map(|p| p.exists()).unwrap_or(false);

        RuntimeStatus {
            runtime,
            runtime_version,
            image_exists,
            running_containers,
            config_path,
            config_exists,
            mcp_config_path,
            mcp_config_exists,
        }
    }

    /// Print status in a human-readable format
    pub fn print(&self, config: &Config) {
        println!("=== CCS Status ===\n");

        // Runtime
        match &self.runtime {
            Some(r) => {
                let version = self.runtime_version.as_deref().unwrap_or("unknown");
                println!("Container runtime: {} ({})", r.name(), version);
            }
            None => {
                println!("Container runtime: NOT FOUND");
                println!("  Install Docker or Podman to use ccs");
            }
        }

        // Image
        println!(
            "Image '{}': {}",
            config.docker.image,
            if self.image_exists {
                "available"
            } else {
                "NOT FOUND (run: ccs --build)"
            }
        );

        // Running containers
        if self.running_containers.is_empty() {
            println!("Running ccs containers: none");
        } else {
            println!("Running ccs containers:");
            for name in &self.running_containers {
                println!("  - {}", name);
            }
        }

        println!();

        // Config files
        if let Some(ref path) = self.config_path {
            println!(
                "Config: {} ({})",
                path.display(),
                if self.config_exists {
                    "exists"
                } else {
                    "not created"
                }
            );
        }

        if let Some(ref path) = self.mcp_config_path {
            println!(
                "MCP config: {} ({})",
                path.display(),
                if self.mcp_config_exists {
                    "exists"
                } else {
                    "not created"
                }
            );
        }

        // Resource limits
        println!();
        println!("Resource limits:");
        match &config.docker.memory_limit {
            Some(mem) => println!("  Memory: {}", mem),
            None => println!("  Memory: unlimited"),
        }
        match config.docker.cpu_limit {
            Some(cpu) => println!("  CPU: {} cores", cpu),
            None => println!("  CPU: unlimited"),
        }
    }
}

fn get_runtime_version(runtime: ContainerRuntime) -> Option<String> {
    let output = Command::new(runtime.command())
        .arg("--version")
        .output()
        .ok()?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        // Extract just the version number
        Some(version.trim().to_string())
    } else {
        None
    }
}

fn check_image_exists(runtime: ContainerRuntime, image: &str) -> bool {
    let output = Command::new(runtime.command())
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    output.map(|s| s.success()).unwrap_or(false)
}

fn list_ccs_containers(runtime: ContainerRuntime) -> Vec<String> {
    let output = Command::new(runtime.command())
        .args([
            "ps",
            "--filter",
            "name=ccs-",
            "--format",
            "{{.Names}}",
        ])
        .output();

    match output {
        Ok(Output { status, stdout, .. }) if status.success() => {
            String::from_utf8_lossy(&stdout)
                .lines()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        _ => vec![],
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
