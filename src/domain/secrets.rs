use std::collections::HashMap;

use super::Result;

/// Abstraction for storing and retrieving secret values.
///
/// The platform implementation stores secrets in the OS keychain (Secret Service
/// on Linux, Keychain on macOS, Credential Manager on Windows). The in-memory
/// implementation is used for testing and as a fallback when the keychain is
/// unavailable (headless/CI).
pub trait SecretStore {
    /// Store a secret value under the given key.
    fn store(&mut self, key: &str, value: &str) -> Result<()>;

    /// Retrieve a secret value by key. Returns `None` if the key is not found.
    fn get(&self, key: &str) -> Result<Option<String>>;

    /// Delete a secret by key.
    fn delete(&mut self, key: &str) -> Result<()>;

    /// Returns true if this store persists data across app restarts.
    fn is_persistent(&self) -> bool;
}

/// In-memory secret store backed by a `HashMap`.
///
/// Useful for testing, CI, and as a fallback when the platform keychain is
/// unavailable. Secrets are lost when the process exits.
#[derive(Clone, Default)]
pub struct MemorySecretStore {
    secrets: HashMap<String, String>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }
}

impl SecretStore for MemorySecretStore {
    fn store(&mut self, key: &str, value: &str) -> Result<()> {
        self.secrets.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.secrets.get(key).cloned())
    }

    fn delete(&mut self, key: &str) -> Result<()> {
        self.secrets.remove(key);
        Ok(())
    }

    fn is_persistent(&self) -> bool {
        false
    }
}

/// Generate an opaque secret key for the given workspace, request, and auth field.
///
/// Format: `apie/{workspace_id}/{request_id}/auth/{field}`
pub fn secret_key(workspace_id: &str, request_id: &str, field: &str) -> String {
    format!("apie/{workspace_id}/{request_id}/auth/{field}")
}

/// Generate a secret key with a random suffix for new secrets.
pub fn new_secret_key(workspace_id: &str, request_id: &str, field: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("apie/{workspace_id}/{request_id}/auth/{field}/{ts}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_roundtrip() {
        let mut store = MemorySecretStore::new();
        store.store("key1", "secret_value").unwrap();
        assert_eq!(store.get("key1").unwrap(), Some("secret_value".to_string()));
        assert_eq!(store.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_memory_store_delete() {
        let mut store = MemorySecretStore::new();
        store.store("key1", "value1").unwrap();
        store.delete("key1").unwrap();
        assert_eq!(store.get("key1").unwrap(), None);
    }

    #[test]
    fn test_secret_key_format() {
        let key = secret_key("ws1", "req1", "token");
        assert_eq!(key, "apie/ws1/req1/auth/token");
    }

    #[test]
    fn test_new_secret_key_is_unique() {
        let k1 = new_secret_key("ws1", "req1", "token");
        let k2 = new_secret_key("ws1", "req1", "token");
        assert_ne!(k1, k2);
        assert!(k1.starts_with("apie/ws1/req1/auth/token/"));
    }

    #[test]
    fn test_is_persistent() {
        let store = MemorySecretStore::new();
        assert!(!store.is_persistent());
    }
}
