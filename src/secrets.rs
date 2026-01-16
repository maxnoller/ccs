use rayon::prelude::*;
use std::collections::HashMap;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecretsError {
    #[error("1Password CLI (op) not found. Install it from https://1password.com/downloads/command-line/")]
    OnePasswordNotFound,

    #[error("Bitwarden Secrets CLI (bws) not found. Install it from https://bitwarden.com/help/secrets-manager-cli/")]
    BitwardenNotFound,

    #[error("pass not found. Install it from https://www.passwordstore.org/")]
    PassNotFound,

    #[error("Failed to resolve secret '{0}': {1}")]
    ResolutionFailed(String, String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve secrets in a HashMap of environment variables
/// Secret references are replaced with their actual values
pub fn resolve_secrets(
    env: &HashMap<String, String>,
    backend: &str,
) -> Result<HashMap<String, String>, SecretsError> {
    env.par_iter()
        .map(|(key, value)| {
            let resolved_value = resolve_secret_value(value, backend)?;
            Ok((key.clone(), resolved_value))
        })
        .collect()
}

/// Resolve a single secret value
fn resolve_secret_value(value: &str, backend: &str) -> Result<String, SecretsError> {
    // Check if this is a secret reference
    if value.starts_with("op://") {
        resolve_1password_secret(value)
    } else if value.starts_with("bws://") {
        resolve_bitwarden_secret(value)
    } else if value.starts_with("pass://") {
        resolve_pass_secret(value)
    } else if value.starts_with("env://") {
        resolve_env_secret(value)
    } else {
        // Not a secret reference, return as-is
        // But if backend is specified, check if it should be resolved
        match backend {
            "1password" if value.contains("op://") => resolve_1password_secret(value),
            "bitwarden" if value.contains("bws://") => resolve_bitwarden_secret(value),
            "pass" if value.contains("pass://") => resolve_pass_secret(value),
            _ => Ok(value.to_string()),
        }
    }
}

/// Resolve a 1Password secret reference
/// Format: op://Vault/Item/Field
fn resolve_1password_secret(reference: &str) -> Result<String, SecretsError> {
    which::which("op").map_err(|_| SecretsError::OnePasswordNotFound)?;

    let output = Command::new("op").arg("read").arg(reference).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SecretsError::ResolutionFailed(
            reference.to_string(),
            stderr.to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve a Bitwarden Secrets Manager secret reference
/// Format: bws://project-id/secret-name or bws://secret-id
fn resolve_bitwarden_secret(reference: &str) -> Result<String, SecretsError> {
    which::which("bws").map_err(|_| SecretsError::BitwardenNotFound)?;

    // Extract the secret identifier from bws://...
    let secret_id = reference.strip_prefix("bws://").unwrap_or(reference);

    let output = Command::new("bws")
        .arg("secret")
        .arg("get")
        .arg(secret_id)
        .arg("--output")
        .arg("json")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SecretsError::ResolutionFailed(
            reference.to_string(),
            stderr.to_string(),
        ));
    }

    // Parse JSON output to get the value
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| SecretsError::ResolutionFailed(reference.to_string(), e.to_string()))?;

    json.get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            SecretsError::ResolutionFailed(
                reference.to_string(),
                "No 'value' field in response".to_string(),
            )
        })
}

/// Resolve a pass (password-store) secret reference
/// Format: pass://path/to/secret
fn resolve_pass_secret(reference: &str) -> Result<String, SecretsError> {
    which::which("pass").map_err(|_| SecretsError::PassNotFound)?;

    // Extract the path from pass://...
    let path = reference.strip_prefix("pass://").unwrap_or(reference);

    let output = Command::new("pass").arg("show").arg(path).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SecretsError::ResolutionFailed(
            reference.to_string(),
            stderr.to_string(),
        ));
    }

    // pass outputs the secret on the first line
    let secret = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    Ok(secret)
}

/// Resolve an environment variable reference
/// Format: env://VARIABLE_NAME
fn resolve_env_secret(reference: &str) -> Result<String, SecretsError> {
    let var_name = reference.strip_prefix("env://").unwrap_or(reference);

    std::env::var(var_name).map_err(|_| {
        SecretsError::ResolutionFailed(
            reference.to_string(),
            format!("Environment variable '{}' not set", var_name),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_secret_resolution() {
        std::env::set_var("TEST_SECRET_CCS", "test_value");
        let result = resolve_env_secret("env://TEST_SECRET_CCS").unwrap();
        assert_eq!(result, "test_value");
        std::env::remove_var("TEST_SECRET_CCS");
    }

    #[test]
    fn test_plain_value_passthrough() {
        let result = resolve_secret_value("plain_value", "env").unwrap();
        assert_eq!(result, "plain_value");
    }

    #[test]
    fn test_resolve_secrets_map() {
        std::env::set_var("TEST_SECRET_CCS_2", "secret_value");
        let mut env = HashMap::new();
        env.insert("PLAIN".to_string(), "plain_value".to_string());
        env.insert("SECRET".to_string(), "env://TEST_SECRET_CCS_2".to_string());

        let resolved = resolve_secrets(&env, "env").unwrap();
        assert_eq!(resolved.get("PLAIN").unwrap(), "plain_value");
        assert_eq!(resolved.get("SECRET").unwrap(), "secret_value");
        std::env::remove_var("TEST_SECRET_CCS_2");
    }

}
