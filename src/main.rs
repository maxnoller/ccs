mod auth;
mod config;
mod docker;
mod git;
mod mcp;
mod secrets;

use clap::Parser;
use std::path::PathBuf;

use config::Config;
use docker::{DockerRunner, RuntimeStatus};
use git::GitContext;

/// Claude Code Sandbox - Run Claude Code safely in Docker containers
#[derive(Parser, Debug)]
#[command(name = "ccs", version, about)]
struct Cli {
    /// Path to the project directory (defaults to current directory)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Create a new worktree and start sandbox in it
    #[arg(long = "new", value_name = "BRANCH")]
    new_worktree: Option<String>,

    /// Create a new branch when creating worktree (use with --new)
    #[arg(short = 'b', long = "branch", requires = "new_worktree")]
    create_branch: bool,

    /// Run directly in current directory without creating a worktree
    #[arg(long, conflicts_with = "new_worktree")]
    here: bool,

    /// Run container in detached mode (background)
    #[arg(short = 'd', long)]
    detach: bool,

    /// List running ccs sessions
    #[arg(long)]
    list: bool,

    /// Attach to a running ccs session
    #[arg(long, value_name = "CONTAINER")]
    attach: Option<String>,

    /// Show logs from a running/stopped ccs session
    #[arg(long, value_name = "CONTAINER")]
    logs: Option<String>,

    /// Stop a running ccs session
    #[arg(long, value_name = "CONTAINER")]
    stop: Option<String>,

    /// Rebuild the container image before starting
    #[arg(long)]
    build: bool,

    /// Print the docker/podman command without executing it
    #[arg(long)]
    dry_run: bool,

    /// Open config file in editor
    #[arg(long)]
    config: bool,

    /// Show status of container runtime, image, and config
    #[arg(long)]
    status: bool,

    /// Extra arguments to pass to Claude Code
    #[arg(last = true)]
    claude_args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle --config flag: open config file in editor
    if cli.config {
        return open_config_in_editor();
    }

    // Load configuration
    let config = Config::load()?;

    // Handle --status flag: show runtime status
    if cli.status {
        let status = RuntimeStatus::check(&config);
        status.print(&config);
        return Ok(());
    }

    // Handle --list flag: list running sessions
    if cli.list {
        return docker::list_sessions();
    }

    // Handle --attach flag: attach to running session
    if let Some(container) = &cli.attach {
        return docker::attach_session(container);
    }

    // Handle --logs flag: show logs from session
    if let Some(container) = &cli.logs {
        return docker::show_logs(container);
    }

    // Handle --stop flag: stop a running session
    if let Some(container) = &cli.stop {
        return docker::stop_session(container);
    }

    // Handle --build flag: rebuild container image
    if cli.build {
        return DockerRunner::build_image(&config);
    }

    // Determine project path
    let project_path = cli
        .path
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let project_path = project_path.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to resolve project path '{}': {}",
            project_path.display(),
            e
        )
    })?;

    // Set up git context (detect or create worktree)
    // Default behavior: auto-create worktree unless --here is specified
    let git_context = if let Some(branch_name) = &cli.new_worktree {
        // Explicit branch name provided with --new
        GitContext::create_worktree(&project_path, branch_name, cli.create_branch, &config)?
    } else if cli.here {
        // --here: run in current directory without creating worktree
        GitContext::detect(&project_path)?
    } else {
        // Default: auto-create worktree with generated branch name
        let branch_name = GitContext::generate_branch_name();
        match GitContext::create_worktree(&project_path, &branch_name, true, &config) {
            Ok(ctx) => ctx,
            Err(git::GitError::CannotCreateFromWorktree) => {
                // Already in a worktree, just use it
                GitContext::detect(&project_path)?
            }
            Err(e) => return Err(e.into()),
        }
    };

    // Generate MCP configuration with resolved secrets
    let mcp_config_path = mcp::generate_mcp_config(&config)?;

    // Run the Docker container (or print command if dry-run)
    let runner = DockerRunner::new(&config, &git_context, mcp_config_path)?;
    runner.run(&cli.claude_args, cli.detach, cli.dry_run)
}

fn open_config_in_editor() -> anyhow::Result<()> {
    let config_path = Config::config_path()?;

    // Ensure config directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create default config if it doesn't exist
    if !config_path.exists() {
        let default_config = Config::default();
        let toml_str = default_config.to_toml()?;
        std::fs::write(&config_path, toml_str)?;
        println!("Created default config at: {}", config_path.display());
    }

    // Open in editor
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&config_path)
        .status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    Ok(())
}
