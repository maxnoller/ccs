# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Claude Code Sandbox (ccs) - A Rust CLI tool that runs Claude Code safely in Docker/Podman containers with the `--dangerously-skip-permissions` flag. Provides container isolation, git worktree support, MCP server integration, and multi-backend secrets management.

## Build Commands

```bash
cargo build --verbose              # Debug build
cargo build --release              # Release build
cargo install --path .             # Install locally
```

## Testing

```bash
cargo test --verbose               # Run all tests
cargo test <module>::tests         # Run tests for specific module (e.g., cargo test config::tests)
```

## Linting

```bash
cargo fmt -- --check               # Check formatting
cargo clippy -- -D warnings        # Run clippy (warnings as errors)
```

## Architecture

The codebase is organized into focused modules under `src/`:

- **main.rs** - CLI entry point using clap. Handles flags like --new (worktree), --detach, --list, --attach, --logs, --stop, --build, --config, --status
- **docker.rs** - Container runtime management. Auto-detects Docker/Podman (prefers Podman). Handles container lifecycle, resource limits, session management
- **config.rs** - Configuration from `~/.config/ccs/config.toml`. Supports template variables like `{repo_name}` in paths
- **git.rs** - Git context detection and worktree management. Handles the complex mount logic for normal repos vs worktrees (worktrees have a file `.git` pointing to shared `.git` dir)
- **auth.rs** - Claude credential discovery chain: ANTHROPIC_API_KEY env var → ~/.claude/.credentials.json (OAuth) → macOS Keychain → ~/.config/claude/auth.json
- **secrets.rs** - Secret resolution for MCP servers. Supports 4 backends: `op://` (1Password), `bws://` (Bitwarden), `pass://` (pass), `env://` (environment)
- **mcp.rs** - Converts MCP config from `~/.config/ccs/mcp.toml` (TOML) to Claude's JSON format with secrets resolved

## Key Design Patterns

- Credential discovery uses a fallback chain - check all sources in order until one succeeds
- Git worktree detection requires special handling since worktrees have a `.git` file (not directory) pointing to the parent repo's `.git` dir
- Secrets are resolved at container startup time, never logged or exposed
- Container names include timestamps for uniqueness: `ccs-{repo}-{timestamp}`

## Configuration Files

User config lives in `~/.config/ccs/`:
- `config.toml` - Main config (docker image, resource limits, volumes, env vars)
- `mcp.toml` - MCP server definitions with secret references

Example configs are in `config/` directory.

## Releasing

Releases are managed via release-please. Do not manually bump versions. Merge the release-please PR to create a new release.
