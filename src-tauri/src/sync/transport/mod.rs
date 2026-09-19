//! 传输层：引擎与具体后端解耦，所有传输只提供 `put/get/list/delete` 原语。
//!
//! - `local`：把 key 映射为本地文件路径，用于无网络的集成测试，也可作为"本地文件夹传输"。
//! - `s3`：OpenList S3 网关（Phase 0 验证后接入）。

pub mod local;
pub mod s3;

pub use local::LocalDirTransport;
pub use s3::S3Transport;

/// `local://D:/path` / `local:///tmp/path` / Windows 绝对路径 → 本地共享目录。
/// `http(s)://` 等其它 scheme 交给 S3。
pub fn local_root_from_endpoint(endpoint: &str) -> Option<std::path::PathBuf> {
    use std::path::{Path, PathBuf};
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }
    if let Some(rest) = endpoint.strip_prefix("local://") {
        let mut rest = rest.trim();
        // local:///D:/foo → D:/foo（去掉多出来的前导斜杠）
        if rest.len() >= 3 && rest.starts_with('/') && rest.as_bytes()[2] == b':' {
            rest = &rest[1..];
        }
        if rest.is_empty() {
            return None;
        }
        return Some(PathBuf::from(rest));
    }
    if endpoint.contains("://") {
        return None;
    }
    let path = Path::new(endpoint);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    None
}

pub fn is_local_endpoint(endpoint: &str) -> bool {
    local_root_from_endpoint(endpoint).is_some()
}

use crate::error::AppResult;

pub trait SyncTransport: Send + Sync {
    fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()>;
    fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>>;
    fn list(&self, prefix: &str) -> AppResult<Vec<String>>;
    fn delete(&self, key: &str) -> AppResult<()>;
    /// 连通性自检；默认对根前缀做一次 list。
    fn test_connection(&self) -> AppResult<()> {
        self.list("").map(|_| ())
    }
}
