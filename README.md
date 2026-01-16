# Claude Code Sandbox (ccs)

A Docker-based sandbox for running [Claude Code](https://github.com/anthropics/claude-code) with `--dangerously-skip-permissions` safely. Provides isolation, git worktree support, and MCP server integration with secrets management.

## Features

- **Container isolation**: Run Claude Code with full permissions safely in Docker or Podman
- **Podman support**: Auto-detects Podman as an alternative to Docker (rootless by default)
- **Resource limits**: Configure memory and CPU limits to prevent runaway processes
- **Project .env files**: Automatically loads `.env` from project directory into container
- **Git worktree support**: Automatic detection and creation of git worktrees
- **MCP servers**: Configure MCP servers with secret injection from password managers
- **Ephemeral sessions**: Fresh container each run, no state leakage
- **Authentication sharing**: Mounts host `~/.claude/` for seamless auth

## Installation

### Quick Install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/maxnoller/ccs/main/install.sh | bash
```

This downloads a pre-built binary or builds from source if needed. Installs to `~/.local/bin/` by default.

### Using Cargo

```bash
cargo install --git https://github.com/maxnoller/ccs
```

### From Source

```bash
git clone https://github.com/maxnoller/ccs && cd ccs
cargo install --path .
```

### Build the Container Image

After installing ccs, build the Docker/Podman image:

```bash
ccs --build
```

### Update

Re-run the install script or cargo command to update to the latest version.

## Quick Start

```bash
# Check setup status
ccs --status

# Run in current directory
cd ~/myproject
ccs

# Run in specific directory
ccs /path/to/project

# Create a new worktree and start sandbox
ccs --new feature-branch

# Create new branch + worktree
ccs --new feature-branch -b

# Pass extra args to Claude
ccs -- --verbose
```

## Configuration

### Main Config: `~/.config/ccs/config.toml`

```bash
# Create from example
mkdir -p ~/.config/ccs
cp config/config.example.toml ~/.config/ccs/config.toml

# Or use the built-in editor command
ccs --config
```

Key settings:

```toml
[docker]
image = "ccs:latest"         # Container image name
memory_limit = "4g"          # Memory limit (optional)
cpu_limit = 2.0              # CPU cores limit (optional)
load_env_file = true         # Load .env from project (default: true)
env_file_path = ".env"       # Path to .env file

[docker.extra_volumes]
# "~/.ssh" = "/home/claude/.ssh:ro"

[docker.extra_env]
# EDITOR = "vim"

[worktree]
base_path = "../{repo_name}-worktrees"

[secrets]
backend = "env"              # 1password, bitwarden, pass, or env
```

### Project .env Files

By default, ccs loads `.env` files from your project directory into the container. This allows Claude to start your application with the correct environment variables:

```bash
# Your project's .env file is automatically loaded
cd ~/myproject
cat .env
# DATABASE_URL=postgres://localhost/mydb
# API_KEY=secret123

ccs  # .env is passed to the container via --env-file
```

To disable: set `load_env_file = false` in config.

### MCP Servers: `~/.config/ccs/mcp.toml`

```bash
cp config/mcp-servers.example.toml ~/.config/ccs/mcp.toml
```

Configure MCP servers with secret references:

```toml
[servers.github]
command = "npx -y @modelcontextprotocol/server-github"
[servers.github.env]
# 1Password
GITHUB_TOKEN = "op://Development/GitHub Token/token"
# Or Bitwarden
# GITHUB_TOKEN = "bws://secret-id"
# Or pass
# GITHUB_TOKEN = "pass://github/token"
# Or environment variable
# GITHUB_TOKEN = "env://GITHUB_TOKEN"
```

## Git Worktrees

The sandbox automatically detects and handles git worktrees:

```bash
# From main repo - creates worktree in sibling directory
cd ~/projects/myrepo
ccs --new feature-x
# Creates: ~/projects/myrepo-worktrees/feature-x

# From existing worktree - mounts both worktree and shared .git
cd ~/projects/myrepo-worktrees/feature-x
ccs
```

Configure worktree location in `config.toml`:

```toml
[worktree]
# Relative to repo parent (default)
base_path = "../{repo_name}-worktrees"

# Or absolute path
# base_path = "~/worktrees/{repo_name}"
```

## Secrets Backends

### 1Password

```toml
[secrets]
backend = "1password"
```

Reference format: `op://Vault/Item/Field`

Requires: [1Password CLI](https://1password.com/downloads/command-line/)

### Bitwarden Secrets Manager

```toml
[secrets]
backend = "bitwarden"
```

Reference format: `bws://secret-id`

Requires: [Bitwarden Secrets CLI](https://bitwarden.com/help/secrets-manager-cli/)

### pass (Password Store)

```toml
[secrets]
backend = "pass"
```

Reference format: `pass://path/to/secret`

Requires: [pass](https://www.passwordstore.org/)

### Environment Variables

```toml
[secrets]
backend = "env"
```

Reference format: `env://VARIABLE_NAME`

## Security Model

| Boundary | Protection |
|----------|------------|
| Filesystem | Only `/workspace` writable; host system isolated |
| Network | Container has network access (for Claude API + MCP) |
| Resources | Optional memory and CPU limits prevent runaway processes |
| Secrets | Injected at runtime; never persisted in image |
| Credentials | `~/.claude/` mounted read-only |
| Ephemerality | Fresh container each run |

## CLI Reference

```
ccs [OPTIONS] [PATH] [-- CLAUDE_ARGS...]

Arguments:
  [PATH]  Project directory (default: current directory)

Options:
  --new <BRANCH>   Create worktree and start sandbox
  -b, --branch     Create new branch with --new
  --build          Rebuild container image
  --config         Open config in $EDITOR
  --status         Show runtime, image, and config status
  -h, --help       Print help
  -V, --version    Print version
```

## Project Structure

```
ccs/
├── Cargo.toml              # Rust manifest
├── src/
│   ├── main.rs             # CLI entry point
│   ├── config.rs           # Configuration
│   ├── docker.rs           # Container operations
│   ├── git.rs              # Git/worktree handling
│   ├── mcp.rs              # MCP config generation
│   └── secrets.rs          # Secret resolution
├── docker/
│   └── Dockerfile          # Container image
├── config/
│   ├── config.example.toml
│   └── mcp-servers.example.toml
└── README.md
```

## License

MIT
