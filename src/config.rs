use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to determine config directory")]
    NoConfigDir,

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_yaml::Error),
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Docker-related settings
    pub docker: DockerConfig,

    /// Worktree-related settings
    pub worktree: WorktreeConfig,

    /// Secrets backend configuration
    pub secrets: SecretsConfig,

    /// Path to the MCP servers configuration file
    pub mcp_config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DockerConfig {
    /// Docker image name
    pub image: String,

    /// Path to the Dockerfile (for building)
    pub dockerfile_path: Option<PathBuf>,

    /// Additional volumes to mount (host_path: container_path)
    pub extra_volumes: HashMap<String, String>,

    /// Additional environment variables
    pub extra_env: HashMap<String, String>,

    /// Container user (default: claude)
    pub user: String,

    /// Working directory in container
    pub workdir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreeConfig {
    /// Base path for creating new worktrees
    /// Supports {repo_name} placeholder
    pub base_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecretsConfig {
    /// Secrets backend: "1password", "bitwarden", "pass", or "env"
    pub backend: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            docker: DockerConfig::default(),
            worktree: WorktreeConfig::default(),
            secrets: SecretsConfig::default(),
            mcp_config_path: None,
        }
    }
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "ccs:latest".to_string(),
            dockerfile_path: None,
            extra_volumes: HashMap::new(),
            extra_env: HashMap::new(),
            user: "claude".to_string(),
            workdir: "/workspace".to_string(),
        }
    }
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            base_path: "../{repo_name}-worktrees".to_string(),
        }
    }
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            backend: "env".to_string(),
        }
    }
}

impl Config {
    /// Returns the path to the config file
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join("ccs").join("config.yaml"))
    }

    /// Returns the path to the MCP servers config file
    pub fn mcp_servers_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join("ccs").join("mcp.yaml"))
    }

    /// Load configuration from file, falling back to defaults
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_yaml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Resolve worktree base path with placeholders
    pub fn resolve_worktree_path(&self, repo_name: &str, repo_parent: &std::path::Path) -> PathBuf {
        let path_str = self
            .worktree
            .base_path
            .replace("{repo_name}", repo_name);

        let path = PathBuf::from(&path_str);

        if path.is_absolute() {
            // Handle ~ expansion for absolute paths
            if path_str.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    return home.join(&path_str[2..]);
                }
            }
            path
        } else {
            // Relative path is relative to repo's parent directory
            repo_parent.join(path)
        }
    }
}

/// MCP Server configuration (loaded from separate file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServersConfig {
    pub servers: HashMap<String, McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl McpServersConfig {
    /// Load MCP servers configuration from file
    pub fn load() -> Result<Option<Self>, ConfigError> {
        let mcp_path = Config::mcp_servers_path()?;

        if mcp_path.exists() {
            let contents = std::fs::read_to_string(&mcp_path)?;
            let config: McpServersConfig = serde_yaml::from_str(&contents)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.docker.image, "ccs:latest");
        assert_eq!(config.secrets.backend, "env");
    }

    #[test]
    fn test_worktree_path_resolution() {
        let config = Config::default();
        let repo_parent = PathBuf::from("/home/user/projects");

        let resolved = config.resolve_worktree_path("myrepo", &repo_parent);
        assert_eq!(
            resolved,
            PathBuf::from("/home/user/projects/../myrepo-worktrees")
        );
    }
}
