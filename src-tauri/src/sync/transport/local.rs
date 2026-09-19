//! 本地目录传输：把对象的 key 映射为共享目录下的文件路径。
//!
//! 用于集成测试（模拟一个共享 bucket），生产环境可作为"文件夹同步"复用。

use super::SyncTransport;
use crate::error::{AppError, AppResult};
use crate::util::safe_relative_path;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct LocalDirTransport {
    root: PathBuf,
}

impl LocalDirTransport {
    pub fn new(root: impl Into<PathBuf>) -> AppResult<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 连通性自检：确保根目录可写并能列出对象。
    pub fn test_connection(&self) -> AppResult<()> {
        fs::create_dir_all(&self.root)?;
        self.list("").map(|_| ())
    }

    fn resolve(&self, key: &str) -> AppResult<PathBuf> {
        // 复用 safe_relative_path：拒绝绝对路径与 `..`，防止逃逸出 bucket 根。
        let relative = safe_relative_path(key)?;
        Ok(self.root.join(relative))
    }
}

impl SyncTransport for LocalDirTransport {
    fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // 先写临时文件再原子替换，避免读到半写对象。
        let tmp = path.with_extension("tmp-upload");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        let path = self.resolve(key)?;
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(path)?))
    }

    fn list(&self, prefix: &str) -> AppResult<Vec<String>> {
        let normalized_prefix = prefix.trim_start_matches('/');
        let mut keys = Vec::new();
        for entry in WalkDir::new(&self.root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = match entry.path().strip_prefix(&self.root) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let key = relative
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(value) => {
                        Some(value.to_string_lossy().to_string())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            if key.starts_with(normalized_prefix) {
                keys.push(key);
            }
        }
        keys.sort();
        Ok(keys)
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        let path = self.resolve(key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(dir.path()).unwrap();
        transport.put("blobs/abc.bin", b"payload").unwrap();
        assert_eq!(
            transport.get("blobs/abc.bin").unwrap().unwrap(),
            b"payload"
        );
    }

    #[test]
    fn missing_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(dir.path()).unwrap();
        assert!(transport.get("blobs/missing.bin").unwrap().is_none());
    }

    #[test]
    fn list_filters_by_prefix_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(dir.path()).unwrap();
        transport.put("devices/b.json", b"b").unwrap();
        transport.put("devices/a.json", b"a").unwrap();
        transport.put("blobs/x.bin", b"x").unwrap();
        assert_eq!(
            transport.list("devices/").unwrap(),
            vec!["devices/a.json".to_string(), "devices/b.json".to_string()]
        );
        assert_eq!(transport.list("").unwrap().len(), 3);
    }

    #[test]
    fn delete_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(dir.path()).unwrap();
        transport.put("blobs/abc.bin", b"payload").unwrap();
        transport.delete("blobs/abc.bin").unwrap();
        assert!(transport.get("blobs/abc.bin").unwrap().is_none());
        transport.delete("blobs/abc.bin").unwrap();
    }

    #[test]
    fn rejects_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(dir.path()).unwrap();
        assert!(transport.put("../escape.bin", b"x").is_err());
        assert!(transport.get("/etc/passwd").is_err());
    }
}
