# TODO

Feature ideas for ccs.

## 1. `--dry-run` flag

Show the exact docker/podman command that would be executed without actually running it. Useful for debugging and understanding what ccs is doing.

**Files to modify:**
- `src/main.rs` - Add CLI flag
- `src/docker.rs` - Add dry-run logic to return command instead of executing

**Implementation:**
- Add `--dry-run` flag to clap Args
- Modify `run_container` to optionally return the command string instead of executing
- Print the full command with all arguments, volumes, env vars, etc.

---

## 2. `--completions <SHELL>` flag

Generate shell completions for bash/zsh/fish using clap's `clap_complete` crate.

**Files to modify:**
- `Cargo.toml` - Add `clap_complete` dependency
- `src/main.rs` - Add flag and completion generation logic

**Implementation:**
- Add `clap_complete` to dependencies
- Add `--completions` flag that accepts shell type (bash, zsh, fish, powershell)
- Generate and print completions to stdout

---

## 3. `--preview-mcp` flag

Show the resolved MCP config JSON (with secrets redacted) that would be passed to Claude. Useful for debugging MCP server configuration.

**Files to modify:**
- `src/main.rs` - Add CLI flag
- `src/mcp.rs` - Add function to generate redacted preview

**Implementation:**
- Add `--preview-mcp` flag to clap Args
- Create a version of MCP config generation that redacts secret values
- Pretty-print the JSON to stdout
