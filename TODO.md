# TODO

Feature ideas for ccs.

## 1. `--dry-run` flag ✅

DONE - Implemented in commit c108233.

---

## 2. Default to worktree mode

Change the default behavior so `ccs` automatically creates a new branch and worktree from the current branch, rather than operating directly on the working directory. This is safer and prevents accidental modifications to the main repo.

**Current behavior:**
- `ccs` runs container against current directory
- `ccs --new BRANCH` creates worktree

**New behavior:**
- `ccs` auto-generates branch name (e.g., `ccs-<timestamp>` or `ccs-<short-hash>`) and creates worktree
- `ccs --here` or `ccs --no-worktree` runs against current directory (opt-out)
- `ccs --new BRANCH` still works as explicit branch name

**Files to modify:**
- `src/main.rs` - Change default behavior, add `--here` flag
- `src/git.rs` - Add auto-generated branch name logic

**Implementation:**
- Generate unique branch name when no `--new` specified and not `--here`
- Create worktree automatically in configured location
- Add `--here` flag to preserve old behavior for power users
- Print clear message about which worktree/branch is being used

---

## 3. `--completions <SHELL>` flag

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
