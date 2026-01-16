mod config;
mod docker;
mod git;
mod mcp;
mod secrets;

use clap::Parser;
use std::path::PathBuf;

use config::Config;
use docker::DockerRunner;
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

    /// Rebuild the container image before starting
    #[arg(long)]
    build: bool,

    /// Open config file in editor
    #[arg(long)]
    config: bool,

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
    let git_context = if let Some(branch_name) = &cli.new_worktree {
        GitContext::create_worktree(&project_path, branch_name, cli.create_branch, &config)?
    } else {
        GitContext::detect(&project_path)?
    };

    // Generate MCP configuration with resolved secrets
    let mcp_config_path = mcp::generate_mcp_config(&config)?;

    // Run the Docker container
    let runner = DockerRunner::new(&config, &git_context, mcp_config_path)?;
    runner.run(&cli.claude_args)
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
        let yaml = serde_yaml::to_string(&default_config)?;
        std::fs::write(&config_path, yaml)?;
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

// Use anyhow for convenient error handling in main
use anyhow;
