use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

use crate::config::{Config, McpServersConfig};
use crate::secrets::{resolve_secrets, SecretsError};

#[derive(Error, Debug)]
pub enum McpError {
    #[error("Failed to read MCP config: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse MCP config: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Failed to serialize MCP config: {0}")]
    SerializeError(#[from] serde_json::Error),

    #[error("Secrets error: {0}")]
    SecretsError(#[from] SecretsError),

    #[error("Config error: {0}")]
    ConfigError(#[from] crate::config::ConfigError),

    #[error("Failed to persist temp file: {0}")]
    TempFilePersist(#[from] tempfile::PathPersistError),
}

/// Claude Code MCP configuration format (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMcpConfig {
    pub mcp_servers: HashMap<String, ClaudeMcpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMcpServer {
    pub command: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

/// Generate MCP configuration file with resolved secrets
/// Returns the path to the generated config file
pub fn generate_mcp_config(config: &Config) -> Result<Option<PathBuf>, McpError> {
    // Load MCP servers config
    let mcp_servers = match McpServersConfig::load()? {
        Some(servers) => servers,
        None => return Ok(None),
    };

    // Convert to Claude MCP format and resolve secrets
    let mut claude_config = ClaudeMcpConfig {
        mcp_servers: HashMap::new(),
    };

    for (name, server) in mcp_servers.servers {
        // Parse command into command + args
        let parts: Vec<&str> = server.command.split_whitespace().collect();
        let (command, implicit_args) = if parts.is_empty() {
            (server.command.clone(), vec![])
        } else {
            (
                parts[0].to_string(),
                parts[1..].iter().map(|s| s.to_string()).collect(),
            )
        };

        // Combine implicit args with explicit args
        let mut all_args = implicit_args;
        all_args.extend(server.args.clone());

        // Resolve secrets in environment variables
        let resolved_env = resolve_secrets(&server.env, &config.secrets.backend)?;

        claude_config.mcp_servers.insert(
            name,
            ClaudeMcpServer {
                command,
                args: all_args,
                env: resolved_env,
            },
        );
    }

    // Write to temporary file
    let temp_file = tempfile::Builder::new()
        .prefix("ccs-mcp-")
        .suffix(".json")
        .tempfile()?;

    let config_json = serde_json::to_string_pretty(&claude_config)?;
    std::fs::write(temp_file.path(), &config_json)?;

    // Keep the file (don't delete on drop)
    let path = temp_file.into_temp_path().keep()?;

    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_mcp_config_serialization() {
        let config = ClaudeMcpConfig {
            mcp_servers: HashMap::from([(
                "github".to_string(),
                ClaudeMcpServer {
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-github".to_string(),
                    ],
                    env: HashMap::from([("GITHUB_TOKEN".to_string(), "test-token".to_string())]),
                },
            )]),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("mcpServers"));
        assert!(json.contains("github"));
    }
}
