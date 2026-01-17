//! Lazy worktree cleanup
//!
//! Automatically cleans up orphaned worktrees on ccs startup.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::docker::ContainerRuntime;

/// Result of cleanup operation
#[derive(Debug, Default)]
pub struct CleanupResult {
    /// Worktrees that were removed
    pub removed: Vec<PathBuf>,
    /// Worktrees that were kept (have changes or running container)
    pub kept: Vec<PathBuf>,
    /// Errors encountered during cleanup
    pub errors: Vec<String>,
}

impl CleanupResult {
    /// Print a summary of the cleanup operation
    pub fn print_summary(&self) {
        if !self.removed.is_empty() {
            println!("Cleaned up {} orphaned worktree(s):", self.removed.len());
            for path in &self.removed {
                println!("  - {}", path.display());
            }
        }

        if !self.errors.is_empty() {
            eprintln!("Cleanup warnings:");
            for err in &self.errors {
                eprintln!("  - {}", err);
            }
        }
    }

    /// Check if any cleanup was performed
    pub fn had_changes(&self) -> bool {
        !self.removed.is_empty()
    }
}

/// Perform lazy cleanup of orphaned ccs worktrees
pub fn lazy_cleanup(config: &Config) -> CleanupResult {
    let mut result = CleanupResult::default();

    // Get the worktree base directory
    let data_dir = match dirs::data_dir() {
        Some(d) => d.join("ccs"),
        None => return result,
    };

    if !data_dir.exists() {
        return result;
    }

    // Get list of running ccs containers
    let running_containers = get_running_container_worktrees();

    // Iterate through repo directories in the ccs data dir
    let entries = match std::fs::read_dir(&data_dir) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let repo_dir = entry.path();
        if !repo_dir.is_dir() {
            continue;
        }

        // Each repo_dir contains worktree directories
        let worktrees = match std::fs::read_dir(&repo_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for wt_entry in worktrees.filter_map(|e| e.ok()) {
            let worktree_path = wt_entry.path();
            if !worktree_path.is_dir() {
                continue;
            }

            // Check if this worktree should be cleaned up
            match should_cleanup_worktree(&worktree_path, &running_containers) {
                CleanupDecision::Remove(reason) => match remove_worktree(&worktree_path, config) {
                    Ok(()) => {
                        result.removed.push(worktree_path);
                    }
                    Err(e) => {
                        result.errors.push(format!(
                            "{}: {} (reason: {})",
                            worktree_path.display(),
                            e,
                            reason
                        ));
                    }
                },
                CleanupDecision::Keep(reason) => {
                    // Only track kept worktrees for verbose output
                    if std::env::var("CCS_VERBOSE").is_ok() {
                        result.kept.push(worktree_path);
                        result.errors.push(format!("Kept: {}", reason));
                    }
                }
            }
        }

        // Remove empty repo directories
        if repo_dir
            .read_dir()
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(&repo_dir);
        }
    }

    result
}

enum CleanupDecision {
    Remove(String),
    Keep(String),
}

fn should_cleanup_worktree(
    worktree_path: &Path,
    running_containers: &[PathBuf],
) -> CleanupDecision {
    // Check if there's a running container using this worktree
    if running_containers.iter().any(|p| p == worktree_path) {
        return CleanupDecision::Keep("container is running".to_string());
    }

    // Check if this is a valid git worktree
    let git_file = worktree_path.join(".git");
    if !git_file.exists() {
        return CleanupDecision::Remove("not a git worktree".to_string());
    }

    // Check for uncommitted changes
    if has_uncommitted_changes(worktree_path) {
        return CleanupDecision::Keep("has uncommitted changes".to_string());
    }

    // Check if branch has unmerged commits
    if has_unmerged_commits(worktree_path) {
        return CleanupDecision::Keep("branch has unmerged commits".to_string());
    }

    // Check age - only clean up worktrees older than 1 hour
    if let Ok(metadata) = std::fs::metadata(worktree_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = std::time::SystemTime::now().duration_since(modified) {
                if duration.as_secs() < 3600 {
                    return CleanupDecision::Keep("recently modified".to_string());
                }
            }
        }
    }

    CleanupDecision::Remove("no changes, no running container".to_string())
}

fn has_uncommitted_changes(worktree_path: &Path) -> bool {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output();

    match output {
        Ok(o) => !o.stdout.is_empty(),
        Err(_) => true, // Assume changes if we can't check
    }
}

fn has_unmerged_commits(worktree_path: &Path) -> bool {
    // Get the current branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(worktree_path)
        .output();

    let branch = match branch_output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => return true, // Assume unmerged if we can't check
    };

    // Skip if this is main/master
    if branch == "main" || branch == "master" {
        return false;
    }

    // Check if branch has commits not in main/master
    // Try main first, then master
    for base in ["main", "master", "origin/main", "origin/master"] {
        let output = Command::new("git")
            .args(["log", &format!("{}..HEAD", base), "--oneline"])
            .current_dir(worktree_path)
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                return !o.stdout.is_empty();
            }
        }
    }

    // If we can't determine, assume there are unmerged commits
    true
}

fn get_running_container_worktrees() -> Vec<PathBuf> {
    let runtime = match ContainerRuntime::detect() {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let output = Command::new(runtime.command())
        .args(["ps", "--filter", "name=ccs-", "--format", "{{.Mounts}}"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse mount paths from container info
    // This is a simplified approach - mounts format varies
    stdout
        .lines()
        .filter_map(|line| {
            // Look for paths that look like our worktree paths
            line.split(',')
                .find(|part| part.contains("/.local/share/ccs/"))
                .map(|p| PathBuf::from(p.trim()))
        })
        .collect()
}

fn remove_worktree(worktree_path: &Path, _config: &Config) -> Result<(), String> {
    // First, try to find the main repo and remove the worktree properly
    let git_file = worktree_path.join(".git");

    if git_file.exists() && git_file.is_file() {
        // Read the .git file to find the main repo
        if let Ok(content) = std::fs::read_to_string(&git_file) {
            if let Some(gitdir) = content.strip_prefix("gitdir: ") {
                let gitdir = gitdir.trim();
                // Navigate up from .git/worktrees/<name> to the main repo
                if let Some(main_git) = PathBuf::from(gitdir)
                    .ancestors()
                    .find(|p| p.ends_with(".git"))
                {
                    if let Some(main_repo) = main_git.parent() {
                        // Try to remove worktree using git
                        let status = Command::new("git")
                            .args(["worktree", "remove", "--force"])
                            .arg(worktree_path)
                            .current_dir(main_repo)
                            .status();

                        if let Ok(s) = status {
                            if s.success() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: just remove the directory
    std::fs::remove_dir_all(worktree_path).map_err(|e| format!("failed to remove directory: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cleanup_result_summary() {
        let result = CleanupResult {
            removed: vec![PathBuf::from("/test/path")],
            kept: vec![],
            errors: vec![],
        };
        assert!(result.had_changes());
    }

    #[test]
    fn test_cleanup_result_empty() {
        let result = CleanupResult::default();
        assert!(!result.had_changes());
    }

    #[test]
    fn test_has_uncommitted_changes_clean() {
        let dir = TempDir::new().unwrap();

        // Initialize a git repo
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Create and commit a file
        fs::write(dir.path().join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "test"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        assert!(!has_uncommitted_changes(dir.path()));
    }

    #[test]
    fn test_has_uncommitted_changes_dirty() {
        let dir = TempDir::new().unwrap();

        // Initialize a git repo
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Create an uncommitted file
        fs::write(dir.path().join("test.txt"), "test").unwrap();

        assert!(has_uncommitted_changes(dir.path()));
    }
}
