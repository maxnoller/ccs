use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::auth::{self, ClaudeCredentials, CredentialSource};
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
    credentials: ClaudeCredentials,
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
        let credentials = auth::discover_credentials();

        Ok(DockerRunner {
            runtime,
            config: config.clone(),
            git_context: git_context.clone(),
            mcp_config_path,
            container_name,
            credentials,
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
    pub fn run(&self, extra_args: &[String], detach: bool) -> anyhow::Result<()> {
        let mut cmd = Command::new(self.runtime.command());

        cmd.arg("run").arg("--name").arg(&self.container_name);

        if detach {
            // Detached mode - run in background, don't remove on exit
            cmd.arg("-d");
        } else {
            // Interactive mode - remove on exit
            cmd.arg("--rm");
            // Only use -it flags when we have a TTY
            if std::io::stdin().is_terminal() {
                cmd.arg("-it");
            } else {
                // Non-interactive mode - still need -i for stdin
                cmd.arg("-i");
            }
        }

        // Add resource limits
        if let Some(ref mem) = self.config.docker.memory_limit {
            cmd.arg("--memory").arg(mem);
        }
        if let Some(cpu) = self.config.docker.cpu_limit {
            cmd.arg("--cpus").arg(cpu.to_string());
        }

        // Load .env file from project if configured and exists
        let env_file_loaded = if self.config.docker.load_env_file {
            let env_path = self
                .git_context
                .workspace_path
                .join(&self.config.docker.env_file_path);
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
            cmd.arg("-v")
                .arg(format!("{}:{}", host_path.display(), container_path));
        }

        // Pass Claude credentials via environment variables (not mount)
        // This is more secure - the container gets the token but can't
        // access or modify host credential files
        for (key, value) in auth::get_credential_env_vars(&self.credentials) {
            cmd.arg("-e").arg(format!("{}={}", key, value));
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
            cmd.arg("-v")
                .arg(format!("{}:{}", expanded_host, container));
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

        if detach {
            println!("Starting Claude Code sandbox (detached)...");
        } else {
            // Set up proper TTY handling for interactive mode
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            println!("Starting Claude Code sandbox...");
        }
        println!("Runtime: {}", self.runtime.name());
        println!("Container: {}", self.container_name);
        println!("Workspace: {}", self.git_context.workspace_path.display());
        if self.git_context.is_worktree {
            println!("(Running in git worktree)");
        }
        // Show credential source
        match self.credentials.source {
            CredentialSource::None => {
                eprintln!("Warning: No Claude credentials found");
                eprintln!("  Run 'claude login' on host, or set ANTHROPIC_API_KEY");
            }
            ref source => {
                println!("Auth: {}", source);
            }
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

        if detach {
            let output = cmd.output()?;
            if output.status.success() {
                let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("Container started: {}", self.container_name);
                println!(
                    "Container ID: {}",
                    &container_id[..12.min(container_id.len())]
                );
                println!();
                println!("Commands:");
                println!("  ccs --list              # List running sessions");
                println!(
                    "  ccs --attach {}   # Attach to session",
                    self.container_name
                );
                println!("  ccs --logs {}     # View logs", self.container_name);
                println!("  ccs --stop {}     # Stop session", self.container_name);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DockerError::CommandFailed(stderr.to_string()).into());
            }
        } else {
            let status = cmd.status()?;
            if !status.success() {
                if let Some(code) = status.code() {
                    std::process::exit(code);
                }
                return Err(
                    DockerError::CommandFailed("Container exited with error".to_string()).into(),
                );
            }
        }

        Ok(())
    }
}

/// List all running ccs sessions
pub fn list_sessions() -> anyhow::Result<()> {
    let runtime = ContainerRuntime::detect()?;

    let output = Command::new(runtime.command())
        .args([
            "ps",
            "-a",
            "--filter",
            "name=ccs-",
            "--format",
            "table {{.Names}}\t{{.Status}}\t{{.CreatedAt}}",
        ])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() || stdout.lines().count() <= 1 {
            println!("No ccs sessions found.");
        } else {
            println!("{}", stdout);
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DockerError::CommandFailed(stderr.to_string()).into());
    }

    Ok(())
}

/// Attach to a running ccs session
pub fn attach_session(container: &str) -> anyhow::Result<()> {
    let runtime = ContainerRuntime::detect()?;

    // Resolve partial container name
    let container_name = resolve_container_name(runtime, container)?;

    println!("Attaching to {}...", container_name);
    println!("(Use Ctrl+P, Ctrl+Q to detach without stopping)\n");

    let status = Command::new(runtime.command())
        .args(["attach", &container_name])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
    }

    Ok(())
}

/// Show logs from a ccs session
pub fn show_logs(container: &str) -> anyhow::Result<()> {
    let runtime = ContainerRuntime::detect()?;

    // Resolve partial container name
    let container_name = resolve_container_name(runtime, container)?;

    let status = Command::new(runtime.command())
        .args(["logs", "-f", &container_name])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
    }

    Ok(())
}

/// Stop a running ccs session
pub fn stop_session(container: &str) -> anyhow::Result<()> {
    let runtime = ContainerRuntime::detect()?;

    // Resolve partial container name
    let container_name = resolve_container_name(runtime, container)?;

    println!("Stopping {}...", container_name);

    let status = Command::new(runtime.command())
        .args(["stop", &container_name])
        .status()?;

    if status.success() {
        println!("Stopped.");

        // Also remove the container
        let _ = Command::new(runtime.command())
            .args(["rm", &container_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    } else {
        return Err(DockerError::CommandFailed("Failed to stop container".to_string()).into());
    }

    Ok(())
}

/// Resolve a partial container name to full name
fn resolve_container_name(runtime: ContainerRuntime, partial: &str) -> anyhow::Result<String> {
    // If it already starts with ccs-, use as-is
    let search_name = if partial.starts_with("ccs-") {
        partial.to_string()
    } else {
        format!("ccs-{}", partial)
    };

    // Try to find matching container
    let output = Command::new(runtime.command())
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name={}", search_name),
            "--format",
            "{{.Names}}",
        ])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let names: Vec<String> = stdout
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        match names.len() {
            0 => Err(anyhow::anyhow!("No container found matching '{}'", partial)),
            1 => Ok(names[0].clone()),
            _ => {
                // Check for exact match
                if let Some(exact) = names.iter().find(|n| n.as_str() == search_name) {
                    Ok(exact.clone())
                } else {
                    Err(anyhow::anyhow!(
                        "Multiple containers match '{}': {}",
                        partial,
                        names.join(", ")
                    ))
                }
            }
        }
    } else {
        Ok(search_name)
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
    pub credentials: ClaudeCredentials,
}

impl RuntimeStatus {
    /// Check the status of the container runtime environment
    pub fn check(config: &Config) -> Self {
        let runtime = ContainerRuntime::detect().ok();
        let runtime_version = runtime.and_then(get_runtime_version);
        let image_exists = runtime
            .map(|r| check_image_exists(r, &config.docker.image))
            .unwrap_or(false);
        let running_containers = runtime.map(list_ccs_containers).unwrap_or_default();

        let config_path = Config::config_path().ok();
        let config_exists = config_path.as_ref().map(|p| p.exists()).unwrap_or(false);

        let mcp_config_path = Config::mcp_servers_path().ok();
        let mcp_config_exists = mcp_config_path
            .as_ref()
            .map(|p| p.exists())
            .unwrap_or(false);

        let credentials = auth::discover_credentials();

        RuntimeStatus {
            runtime,
            runtime_version,
            image_exists,
            running_containers,
            config_path,
            config_exists,
            mcp_config_path,
            mcp_config_exists,
            credentials,
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

        // Credentials
        match self.credentials.source {
            CredentialSource::None => {
                println!("Claude credentials: NOT FOUND");
                println!("  Run 'claude login' on host, or set ANTHROPIC_API_KEY");
            }
            ref source => {
                println!(
                    "Claude credentials: {} ({})",
                    source,
                    if self.credentials.api_key.is_some() {
                        "API key"
                    } else {
                        "OAuth token"
                    }
                );
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
        .args(["ps", "--filter", "name=ccs-", "--format", "{{.Names}}"])
        .output();

    match output {
        Ok(Output { status, stdout, .. }) if status.success() => String::from_utf8_lossy(&stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect(),
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
