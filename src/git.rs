use git2::Repository;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::config::Config;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Not a git repository: {0}")]
    NotARepo(PathBuf),

    #[error("Git error: {0}")]
    Git2(#[from] git2::Error),

    #[error("Failed to determine repository name")]
    NoRepoName,

    #[error("Cannot create worktree from within a worktree. Run from the main repository.")]
    CannotCreateFromWorktree,

    #[error("Worktree already exists: {0}")]
    WorktreeExists(PathBuf),

    #[error("Branch '{0}' already exists. Use -b to create from existing branch.")]
    BranchExists(String),

    #[error("Branch '{0}' not found. Use -b to create a new branch.")]
    BranchNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Git context for mounting in Docker
#[derive(Debug, Clone)]
pub struct GitContext {
    /// The working directory (worktree or repo root)
    pub workspace_path: PathBuf,

    /// The shared .git directory (for worktrees, this is the main repo's .git)
    /// None if this is not a worktree
    pub shared_git_dir: Option<PathBuf>,

    /// Name of the repository
    pub repo_name: String,

    /// Whether this is a worktree
    pub is_worktree: bool,
}

impl GitContext {
    /// Detect git context from a path
    pub fn detect(path: &PathBuf) -> Result<Self, GitError> {
        let repo = Repository::discover(path).map_err(|_| GitError::NotARepo(path.clone()))?;

        let is_worktree = repo.is_worktree();
        let workdir = repo
            .workdir()
            .ok_or_else(|| GitError::NotARepo(path.clone()))?;
        let workspace_path = workdir.to_path_buf();

        // Get the repository name from the path
        let repo_name = Self::extract_repo_name(&repo)?;

        let shared_git_dir = if is_worktree {
            // For worktrees, find the common/shared .git directory
            Self::find_common_git_dir(&repo)
        } else {
            None
        };

        Ok(GitContext {
            workspace_path,
            shared_git_dir,
            repo_name,
            is_worktree,
        })
    }

    /// Find the common git directory for a worktree
    fn find_common_git_dir(repo: &Repository) -> Option<PathBuf> {
        // repo.path() returns the .git directory (or .git/worktrees/<name> for worktrees)
        let git_path = repo.path();

        // For worktrees, the path is like: /path/to/main/.git/worktrees/<name>
        // We want: /path/to/main/.git
        if let Some(worktrees_parent) = git_path.parent() {
            if worktrees_parent
                .file_name()
                .map(|n| n == "worktrees")
                .unwrap_or(false)
            {
                return worktrees_parent.parent().map(|p| p.to_path_buf());
            }
        }

        None
    }

    /// Create a new worktree and return its context
    pub fn create_worktree(
        repo_path: &PathBuf,
        branch_name: &str,
        create_branch: bool,
        config: &Config,
    ) -> Result<Self, GitError> {
        let repo =
            Repository::discover(repo_path).map_err(|_| GitError::NotARepo(repo_path.clone()))?;

        // Don't allow creating worktrees from within a worktree
        if repo.is_worktree() {
            return Err(GitError::CannotCreateFromWorktree);
        }

        let repo_name = Self::extract_repo_name(&repo)?;

        // Determine worktree location
        let repo_parent = repo
            .workdir()
            .ok_or_else(|| GitError::NotARepo(repo_path.clone()))?
            .parent()
            .ok_or(GitError::NoRepoName)?;

        let worktree_base = config.resolve_worktree_path(&repo_name, repo_parent);

        // Create worktree base directory if it doesn't exist
        std::fs::create_dir_all(&worktree_base)?;

        let worktree_path = worktree_base.join(branch_name);

        if worktree_path.exists() {
            return Err(GitError::WorktreeExists(worktree_path));
        }

        // Determine the reference for the worktree
        let reference = if create_branch {
            // Create new branch from HEAD
            let head = repo.head()?;
            let head_commit = head.peel_to_commit()?;

            // Check if branch already exists
            if repo
                .find_branch(branch_name, git2::BranchType::Local)
                .is_ok()
            {
                return Err(GitError::BranchExists(branch_name.to_string()));
            }

            // Create the branch
            repo.branch(branch_name, &head_commit, false)?;

            format!("refs/heads/{}", branch_name)
        } else {
            // Use existing branch
            let branch = repo
                .find_branch(branch_name, git2::BranchType::Local)
                .map_err(|_| GitError::BranchNotFound(branch_name.to_string()))?;

            branch
                .get()
                .name()
                .ok_or(GitError::BranchNotFound(branch_name.to_string()))?
                .to_string()
        };

        // Create the worktree using git command (git2's worktree support is limited)
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.workdir().unwrap())
            .arg("worktree")
            .arg("add")
            .arg(&worktree_path)
            .arg(branch_name)
            .status()?;

        if !status.success() {
            return Err(GitError::Git2(git2::Error::from_str(
                "Failed to create worktree",
            )));
        }

        println!("Created worktree at: {}", worktree_path.display());
        println!("Branch: {}", reference);

        // Return context for the new worktree
        Ok(GitContext {
            workspace_path: worktree_path.canonicalize()?,
            shared_git_dir: Some(repo.path().to_path_buf()),
            repo_name,
            is_worktree: true,
        })
    }

    /// Extract repository name from the repository
    fn extract_repo_name(repo: &Repository) -> Result<String, GitError> {
        // For worktrees, we need to get the name from the main repo
        let git_dir = repo.path();

        // For worktrees, path is .git/worktrees/<name>, go up to find main repo
        let main_git_dir = if repo.is_worktree() {
            Self::find_common_git_dir(repo).unwrap_or_else(|| git_dir.to_path_buf())
        } else {
            git_dir.to_path_buf()
        };

        // The .git directory is inside the repo, so get its parent
        let repo_root: Option<&Path> = if main_git_dir.ends_with(".git") {
            main_git_dir.parent()
        } else {
            // Bare repo or other case
            Some(&main_git_dir)
        };

        repo_root
            .and_then(|p: &Path| p.file_name())
            .and_then(|n: &std::ffi::OsStr| n.to_str())
            .map(|s: &str| s.to_string())
            .ok_or(GitError::NoRepoName)
    }

    /// Get mount specifications for Docker
    pub fn docker_mounts(&self) -> Vec<(PathBuf, String)> {
        let mut mounts = vec![(self.workspace_path.clone(), "/workspace".to_string())];

        // For worktrees, also mount the shared .git directory
        if let Some(ref git_dir) = self.shared_git_dir {
            // Mount the parent of the .git directory to preserve the structure
            // The worktree's .git file points to ../../.git/worktrees/<name>
            // So we need to mount the shared .git at a path that matches
            mounts.push((git_dir.clone(), "/workspace/.git-main".to_string()));
        }

        mounts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_context_mounts() {
        let ctx = GitContext {
            workspace_path: PathBuf::from("/home/user/project"),
            shared_git_dir: None,
            repo_name: "project".to_string(),
            is_worktree: false,
        };

        let mounts = ctx.docker_mounts();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].1, "/workspace");
    }

    #[test]
    fn test_worktree_context_mounts() {
        let ctx = GitContext {
            workspace_path: PathBuf::from("/home/user/project-worktrees/feature"),
            shared_git_dir: Some(PathBuf::from("/home/user/project/.git")),
            repo_name: "project".to_string(),
            is_worktree: true,
        };

        let mounts = ctx.docker_mounts();
        assert_eq!(mounts.len(), 2);
    }
}
