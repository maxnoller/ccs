# TODO

Feature roadmap for ccs. See [GitHub Issues](https://github.com/maxnoller/ccs/issues) for details.

## Completed

- [x] `--dry-run` flag - Show docker command without executing (c108233)
- [x] Default to worktree mode - Auto-create worktrees in `~/.local/share/ccs/` (f4d8e9d)

## In Progress

| Issue | Feature | Description |
|-------|---------|-------------|
| [#8](https://github.com/maxnoller/ccs/issues/8) | `ccs init` | First-run wizard (non-interactive friendly) |
| [#9](https://github.com/maxnoller/ccs/issues/9) | Shell completions | `--completions bash/zsh/fish` |
| [#10](https://github.com/maxnoller/ccs/issues/10) | Smart naming | UUID session IDs + LLM-generated branch names |
| [#11](https://github.com/maxnoller/ccs/issues/11) | Remove resource limits | Simplify config by removing memory/cpu limits |
| [#12](https://github.com/maxnoller/ccs/issues/12) | Per-project config | `.ccs.toml` in repo root |
| [#13](https://github.com/maxnoller/ccs/issues/13) | Improved status | Session details, disk usage, credential expiry |
| [#14](https://github.com/maxnoller/ccs/issues/14) | Auto cleanup | Automatic worktree cleanup on session end |
| [#15](https://github.com/maxnoller/ccs/issues/15) | Better errors | Error messages with actionable suggestions |
| [#16](https://github.com/maxnoller/ccs/issues/16) | `logs --follow` | Real-time log tailing |
| [#17](https://github.com/maxnoller/ccs/issues/17) | `ccs exec` | Run commands in running containers |
| [#18](https://github.com/maxnoller/ccs/issues/18) | `--preview-mcp` | Show resolved MCP config with redacted secrets |
