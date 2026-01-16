use serde::Deserialize;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::process::Command;

/// Discovered Claude credentials
#[derive(Debug, Clone)]
pub struct ClaudeCredentials {
    /// The type of credential discovered
    pub source: CredentialSource,
    /// OAuth access token (for Claude Max)
    pub oauth_token: Option<String>,
    /// API key (for Anthropic API)
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CredentialSource {
    /// From ANTHROPIC_API_KEY environment variable
    EnvApiKey,
    /// From ~/.claude/ credentials file
    ClaudeDir,
    /// From macOS Keychain
    #[cfg(target_os = "macos")]
    MacOsKeychain,
    /// From ~/.config/claude/ directory
    ConfigDir,
    /// No credentials found
    None,
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSource::EnvApiKey => write!(f, "ANTHROPIC_API_KEY env var"),
            CredentialSource::ClaudeDir => write!(f, "~/.claude/"),
            #[cfg(target_os = "macos")]
            CredentialSource::MacOsKeychain => write!(f, "macOS Keychain"),
            CredentialSource::ConfigDir => write!(f, "~/.config/claude/"),
            CredentialSource::None => write!(f, "none"),
        }
    }
}

/// Structure for parsing ~/.claude/.credentials.json
#[derive(Debug, Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthCredentials>,
}

#[derive(Debug, Deserialize)]
struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

/// Structure for parsing auth.json format
#[derive(Debug, Deserialize)]
struct AuthJsonFile {
    access_token: Option<String>,
    #[allow(dead_code)]
    refresh_token: Option<String>,
}

/// Discover Claude credentials from various sources
///
/// Checks in order:
/// 1. ANTHROPIC_API_KEY environment variable
/// 2. ~/.claude/.credentials.json (OAuth tokens)
/// 3. macOS Keychain (claude-auth)
/// 4. ~/.config/claude/auth.json
///
/// Returns credentials if found, with source information
pub fn discover_credentials() -> ClaudeCredentials {
    // 1. Check environment variable first
    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        if !api_key.is_empty() {
            return ClaudeCredentials {
                source: CredentialSource::EnvApiKey,
                oauth_token: None,
                api_key: Some(api_key),
            };
        }
    }

    // 2. Check ~/.claude/.credentials.json
    if let Some(creds) = check_claude_dir() {
        return creds;
    }

    // 3. Check macOS Keychain
    #[cfg(target_os = "macos")]
    if let Some(creds) = check_macos_keychain() {
        return creds;
    }

    // 4. Check ~/.config/claude/auth.json
    if let Some(creds) = check_config_dir() {
        return creds;
    }

    ClaudeCredentials {
        source: CredentialSource::None,
        oauth_token: None,
        api_key: None,
    }
}

/// Check ~/.claude/.credentials.json for OAuth tokens
fn check_claude_dir() -> Option<ClaudeCredentials> {
    let home = dirs::home_dir()?;
    let credentials_path = home.join(".claude").join(".credentials.json");

    if !credentials_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&credentials_path).ok()?;
    let creds: CredentialsFile = serde_json::from_str(&content).ok()?;

    if let Some(oauth) = creds.claude_ai_oauth {
        if let Some(token) = oauth.access_token {
            if !token.is_empty() {
                // Check if token is expired
                if let Some(expires_at) = oauth.expires_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);

                    if expires_at < now {
                        // Token expired, but we might still have a refresh token
                        // Claude Code will handle the refresh
                        eprintln!(
                            "Warning: OAuth token expired, Claude Code will attempt to refresh"
                        );
                    }
                }

                return Some(ClaudeCredentials {
                    source: CredentialSource::ClaudeDir,
                    oauth_token: Some(token),
                    api_key: None,
                });
            }
        }
    }

    None
}

/// Check macOS Keychain for Claude auth credentials
#[cfg(target_os = "macos")]
fn check_macos_keychain() -> Option<ClaudeCredentials> {
    // Try to get token from keychain using security command
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "claude-auth", "-w"])
        .output()
        .ok()?;

    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(ClaudeCredentials {
                source: CredentialSource::MacOsKeychain,
                oauth_token: Some(token),
                api_key: None,
            });
        }
    }

    None
}

/// Check ~/.config/claude/auth.json for credentials
fn check_config_dir() -> Option<ClaudeCredentials> {
    let home = dirs::home_dir()?;

    // Try multiple possible locations
    let paths = [
        home.join(".config").join("claude").join("auth.json"),
        home.join(".config").join("claude-code").join("auth.json"),
    ];

    for path in &paths {
        if let Some(creds) = try_parse_auth_json(path) {
            return Some(creds);
        }
    }

    None
}

/// Try to parse an auth.json file
fn try_parse_auth_json(path: &PathBuf) -> Option<ClaudeCredentials> {
    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(path).ok()?;
    let auth: AuthJsonFile = serde_json::from_str(&content).ok()?;

    if let Some(token) = auth.access_token {
        if !token.is_empty() {
            return Some(ClaudeCredentials {
                source: CredentialSource::ConfigDir,
                oauth_token: Some(token),
                api_key: None,
            });
        }
    }

    None
}

/// Get environment variables to pass to the container based on discovered credentials
pub fn get_credential_env_vars(creds: &ClaudeCredentials) -> Vec<(String, String)> {
    let mut vars = Vec::new();

    if let Some(ref api_key) = creds.api_key {
        vars.push(("ANTHROPIC_API_KEY".to_string(), api_key.clone()));
    }

    if let Some(ref token) = creds.oauth_token {
        // Claude Code uses CLAUDE_CODE_OAUTH_TOKEN for OAuth authentication
        vars.push(("CLAUDE_CODE_OAUTH_TOKEN".to_string(), token.clone()));
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_source_display() {
        assert_eq!(
            format!("{}", CredentialSource::EnvApiKey),
            "ANTHROPIC_API_KEY env var"
        );
        assert_eq!(format!("{}", CredentialSource::ClaudeDir), "~/.claude/");
        assert_eq!(
            format!("{}", CredentialSource::MacOsKeychain),
            "macOS Keychain"
        );
    }
}
