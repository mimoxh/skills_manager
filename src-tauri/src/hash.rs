use crate::error::{AppError, AppResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn hash_dir(path: &Path) -> AppResult<String> {
    let path = resolve_dir_source(path);
    let mut files = Vec::new();
    for entry in WalkDir::new(&path).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let relative = file
            .strip_prefix(&path)
            .map_err(|_| AppError::Message(format!("路径前缀剥离失败: {}", file.display())))?
            .to_string_lossy();
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        // 流式读取，避免大文件整体读入内存
        let mut reader = fs::File::open(&file)?;
        std::io::copy(&mut reader, &mut hasher)?;
        hasher.update(b"\0");
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 若路径是目录软链/Junction，解析为真实目录，避免 WalkDir 不跟链导致空复制。
pub fn resolve_dir_source(path: &Path) -> PathBuf {
    if is_symlink_path(path) {
        if let Ok(real) = fs::canonicalize(path) {
            return real;
        }
        if let Some(target) = read_symlink_target(path) {
            if target.is_relative() {
                if let Some(parent) = path.parent() {
                    return parent.join(target);
                }
            }
            return target;
        }
    }
    path.to_path_buf()
}

pub fn copy_dir_all(source: &Path, target: &Path) -> AppResult<()> {
    let source = resolve_dir_source(source);
    fs::create_dir_all(target)?;
    for entry in WalkDir::new(&source).into_iter().filter_map(Result::ok) {
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(|_| AppError::Message(format!("路径前缀剥离失败: {}", entry.path().display())))?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

/// 路径是否为符号链接/Junction（不跟随链接）。
pub fn is_symlink_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// 读取符号链接/Junction 的目标路径（跟随语义由平台 API 决定）。
pub fn read_symlink_target(path: &Path) -> Option<PathBuf> {
    fs::read_link(path).ok()
}

#[cfg(windows)]
fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, target)
}

#[cfg(not(windows))]
fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

/// Windows Junction：本地目录无需管理员/开发者模式。
#[cfg(windows)]
fn create_dir_junction(source: &Path, target: &Path) -> std::io::Result<()> {
    let status = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(target)
        .arg(source)
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("mklink /J failed"))
    }
}

#[cfg(not(windows))]
fn create_dir_junction(_source: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("junction unsupported"))
}

#[cfg(windows)]
fn remove_symlink(path: &Path) -> std::io::Result<()> {
    fs::remove_dir(path).or_else(|_| fs::remove_file(path))
}

#[cfg(not(windows))]
fn remove_symlink(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

/// 安全移除目录、文件或软链接。如果路径是软链接，仅删除该链接节点，绝不伤及源目录。
pub fn remove_dir_or_symlink(path: &Path) -> AppResult<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            remove_symlink(path)?;
            return Ok(());
        }
    }
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// 优先创建目录链接（Symlink，失败则 Junction），最后回退复制。
/// 返回 Ok(true) 表示创建了链接，Ok(false) 表示使用了普通复制。
pub fn symlink_or_copy_dir(source: &Path, target: &Path) -> AppResult<bool> {
    remove_dir_or_symlink(target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if create_dir_symlink(source, target).is_ok() || create_dir_junction(source, target).is_ok() {
        return Ok(true);
    }
    copy_dir_all(source, target)?;
    Ok(false)
}

/// 计算任意可序列化值的稳定指纹（sha256 over 键排序后的 JSON 表示）。
/// 与 Debug 格式化 + HashMap 迭代序不同，同一配置在任何进程/时刻算出同一指纹，
/// 用于 MCP 配置变更检测。
pub fn stable_fingerprint<T: Serialize>(value: &T) -> AppResult<String> {
    let json = serde_json::to_value(value).map_err(AppError::from)?;
    let mut hasher = Sha256::new();
    hash_value(&mut hasher, &json);
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_value(hasher: &mut Sha256, value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // 对象键排序后依次哈希，消除 HashMap 迭代顺序的不确定性
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                hasher.update(key.as_bytes());
                hasher.update(b"\0");
                hash_value(hasher, &map[key]);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                hash_value(hasher, item);
            }
        }
        _ => {
            hasher.update(value.to_string().as_bytes());
        }
    }
    hasher.update(b"\0");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn copy_dir_all_follows_symlink_source() {
        let real = tempfile::tempdir().unwrap();
        fs::write(real.path().join("SKILL.md"), "real-content").unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let link = link_parent.path().join("demo");
        symlink_or_copy_dir(real.path(), &link).unwrap();
        let target = tempfile::tempdir().unwrap();
        let dest = target.path().join("out");
        copy_dir_all(&link, &dest).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "real-content"
        );
    }

    #[test]
    fn stable_fingerprint_is_deterministic_across_map_order() {
        // HashMap 迭代顺序随机，但指纹必须跨插入顺序一致
        let mut a = HashMap::new();
        a.insert("b".to_string(), "2".to_string());
        a.insert("a".to_string(), "1".to_string());
        let mut b = HashMap::new();
        b.insert("a".to_string(), "1".to_string());
        b.insert("b".to_string(), "2".to_string());
        assert_eq!(stable_fingerprint(&a).unwrap(), stable_fingerprint(&b).unwrap());
    }

    #[test]
    fn stable_fingerprint_differs_for_different_values() {
        assert_ne!(
            stable_fingerprint(&"hello").unwrap(),
            stable_fingerprint(&"world").unwrap()
        );
    }
}


