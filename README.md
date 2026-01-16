# Claude Code Sandbox (ccs)

A Docker-based sandbox for running [Claude Code](https://github.com/anthropics/claude-code) with `--dangerously-skip-permissions` safely. Provides isolation, git worktree support, and MCP server integration with secrets management.

## Features

- **Container isolation**: Run Claude Code with full permissions safely in Docker
- **Git worktree support**: Automatic detection and creation of git worktrees
- **MCP servers**: Configure MCP servers with secret injection from password managers
- **Ephemeral sessions**: Fresh container each run, no state leakage
- **Authentication sharing**: Mounts host `~/.claude/` for seamless auth

## Installation

```bash
# Clone and build
git clone <repo-url> && cd ccs
cargo install --path .

# Build the Docker image
ccs --build
```

## Quick Start

```bash
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

### Main Config: `~/.config/ccs/config.yaml`

```bash
# Create from example
mkdir -p ~/.config/ccs
cp config/config.example.yaml ~/.config/ccs/config.yaml

# Or use the built-in editor command
ccs --config
```

Key settings:

```yaml
docker:
  image: ccs:latest          # Docker image name
  extra_volumes: {}          # Additional mounts
  extra_env: {}              # Additional env vars

worktree:
  base_path: "../{repo_name}-worktrees"  # Where to create worktrees

secrets:
  backend: env               # 1password, bitwarden, pass, or env
```

### MCP Servers: `~/.config/ccs/mcp.yaml`

```bash
cp config/mcp-servers.example.yaml ~/.config/ccs/mcp.yaml
```

Configure MCP servers with secret references:

```yaml
servers:
  github:
    command: npx -y @modelcontextprotocol/server-github
    env:
      # 1Password
      GITHUB_TOKEN: "op://Development/GitHub Token/token"
      # Or Bitwarden
      # GITHUB_TOKEN: "bws://secret-id"
      # Or pass
      # GITHUB_TOKEN: "pass://github/token"
      # Or environment variable
      # GITHUB_TOKEN: "env://GITHUB_TOKEN"
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

Configure worktree location in `config.yaml`:

```yaml
worktree:
  # Relative to repo parent (default)
  base_path: "../{repo_name}-worktrees"

  # Or absolute path
  base_path: "~/worktrees/{repo_name}"
```

## Secrets Backends

### 1Password

```yaml
secrets:
  backend: 1password
```

Reference format: `op://Vault/Item/Field`

Requires: [1Password CLI](https://1password.com/downloads/command-line/)

### Bitwarden Secrets Manager

```yaml
secrets:
  backend: bitwarden
```

Reference format: `bws://secret-id`

Requires: [Bitwarden Secrets CLI](https://bitwarden.com/help/secrets-manager-cli/)

### pass (Password Store)

```yaml
secrets:
  backend: pass
```

Reference format: `pass://path/to/secret`

Requires: [pass](https://www.passwordstore.org/)

### Environment Variables

```yaml
secrets:
  backend: env
```

Reference format: `env://VARIABLE_NAME`

## Security Model

| Boundary | Protection |
|----------|------------|
| Filesystem | Only `/workspace` writable; host system isolated |
| Network | Container has network access (for Claude API + MCP) |
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
  --build          Rebuild Docker image
  --config         Open config in $EDITOR
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
│   ├── docker.rs           # Docker operations
│   ├── git.rs              # Git/worktree handling
│   ├── mcp.rs              # MCP config generation
│   └── secrets.rs          # Secret resolution
├── docker/
│   └── Dockerfile          # Container image
├── config/
│   ├── config.example.yaml
│   └── mcp-servers.example.yaml
└── README.md
```

## License

MIT
