//! 机密存储：`secretAccessKey` 与加密口令不进 `state.json`，存 OS 钥匙串。
//!
//! 测试提供内存实现，避免触碰真实系统钥匙串。

use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    fn set(&self, key: &str, value: &str) -> AppResult<()>;
    fn delete(&self, key: &str) -> AppResult<()>;
}

/// 固定 service 名下按 key 存取的钥匙串实现。
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(&self.service, key)
            .map_err(|error| AppError::Message(format!("打开钥匙串失败: {error}")))
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::Message(format!("读取钥匙串失败: {error}"))),
        }
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        self.entry(key)?
            .set_password(value)
            .map_err(|error| AppError::Message(format!("写入钥匙串失败: {error}")))
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        match self.entry(key)?.delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::Message(format!("删除钥匙串失败: {error}"))),
        }
    }
}

/// 内存实现（测试用）。
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self
            .map
            .lock()
            .map_err(|_| AppError::Message("Secret lock poisoned".to_string()))?
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        self.map
            .lock()
            .map_err(|_| AppError::Message("Secret lock poisoned".to_string()))?
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        self.map
            .lock()
            .map_err(|_| AppError::Message("Secret lock poisoned".to_string()))?
            .remove(key);
        Ok(())
    }
}

/// 密钥命名：`s3-secret`、`encrypt-password`。
pub const SECRET_ACCESS_KEY: &str = "s3-secret";
pub const ENCRYPT_PASSWORD: &str = "encrypt-password";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let store = MemorySecretStore::new();
        assert_eq!(store.get(SECRET_ACCESS_KEY).unwrap(), None);
        store.set(SECRET_ACCESS_KEY, "abc").unwrap();
        assert_eq!(
            store.get(SECRET_ACCESS_KEY).unwrap().as_deref(),
            Some("abc")
        );
        store.delete(SECRET_ACCESS_KEY).unwrap();
        assert_eq!(store.get(SECRET_ACCESS_KEY).unwrap(), None);
    }
}
