use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// Secrets store — stores API keys and tokens separate from main config.
/// File is written with restrictive permissions (0o600 on Unix).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsStore {
    /// Raw key-value pairs (key name → secret value)
    secrets: HashMap<String, String>,
}

impl SecretsStore {
    /// Load secrets from file, or create empty if not found.
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let store: SecretsStore = serde_json::from_str(&data)?;
            info!("Loaded {} secrets from {}", store.secrets.len(), path.display());
            Ok(store)
        } else {
            Ok(Self {
                secrets: HashMap::new(),
            })
        }
    }

    /// Save secrets to file with restrictive permissions.
    pub fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(&self)?;
        std::fs::write(path, &data)?;

        // Set file permissions to 0o600 (owner read/write only) on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }

        info!("Saved {} secrets to {} (0o600)", self.secrets.len(), path.display());
        Ok(())
    }

    /// Get a secret by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|s| s.as_str())
    }

    /// Set a secret.
    pub fn set(&mut self, key: String, value: String) {
        self.secrets.insert(key, value);
    }

    /// Remove a secret.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.secrets.remove(key)
    }

    /// Check if a secret exists.
    pub fn has(&self, key: &str) -> bool {
        self.secrets.contains_key(key)
    }

    /// Get the default secrets file path.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".pocketclaw/secrets.json")
    }
}

/// Mask a secret value for safe display in logs.
/// Shows first 4 and last 4 chars, the rest as `****`.
pub fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &value[..4], &value[value.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret() {
        assert_eq!(mask_secret("short"), "****");
        assert_eq!(mask_secret("sk-1234567890abcdef"), "sk-1****cdef");
    }
}
