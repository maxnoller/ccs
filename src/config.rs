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
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
}

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Load .env file from project directory into container
    pub load_env_file: bool,

    /// Custom .env file path (relative to project, defaults to ".env")
    pub env_file_path: String,
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

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "ccs:latest".to_string(),
            dockerfile_path: None,
            extra_volumes: HashMap::new(),
            extra_env: HashMap::new(),
            user: "claude".to_string(),
            workdir: "/workspace".to_string(),
            load_env_file: true,
            env_file_path: ".env".to_string(),
        }
    }
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            base_path: "{data_dir}/ccs/{repo_name}".to_string(),
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
        Ok(config_dir.join("ccs").join("config.toml"))
    }

    /// Returns the path to the MCP servers config file
    pub fn mcp_servers_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join("ccs").join("mcp.toml"))
    }

    /// Load configuration from file, falling back to defaults
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Serialize config to TOML string
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Resolve worktree base path with placeholders
    /// Supports: {repo_name}, {data_dir} (XDG_DATA_HOME, defaults to ~/.local/share)
    pub fn resolve_worktree_path(&self, repo_name: &str, repo_parent: &std::path::Path) -> PathBuf {
        let mut path_str = self.worktree.base_path.replace("{repo_name}", repo_name);

        // Replace {data_dir} with XDG_DATA_HOME
        if path_str.contains("{data_dir}") {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
            path_str = path_str.replace("{data_dir}", &data_dir.to_string_lossy());
        }

        let path = PathBuf::from(&path_str);

        if path.is_absolute() {
            // Handle ~ expansion for absolute paths
            if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home) = dirs::home_dir() {
                    return home.join(stripped);
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
            let config: McpServersConfig = toml::from_str(&contents)?;
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
        assert!(config.docker.load_env_file);
    }

    #[test]
    fn test_worktree_path_resolution_with_data_dir() {
        let config = Config::default();
        let repo_parent = PathBuf::from("/home/user/projects");

        let resolved = config.resolve_worktree_path("myrepo", &repo_parent);
        // Default uses {data_dir}/ccs/{repo_name}, which resolves to XDG_DATA_HOME
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
        assert_eq!(resolved, data_dir.join("ccs").join("myrepo"));
    }

    #[test]
    fn test_worktree_path_resolution_relative() {
        let mut config = Config::default();
        config.worktree.base_path = "../{repo_name}-worktrees".to_string();
        let repo_parent = PathBuf::from("/home/user/projects");

        let resolved = config.resolve_worktree_path("myrepo", &repo_parent);
        assert_eq!(
            resolved,
            PathBuf::from("/home/user/projects/../myrepo-worktrees")
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = config.to_toml().unwrap();
        assert!(toml_str.contains("[docker]"));
        assert!(toml_str.contains("image = \"ccs:latest\""));
    }
}
