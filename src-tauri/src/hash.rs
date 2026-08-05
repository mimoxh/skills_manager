use crate::error::{AppError, AppResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use walkdir::WalkDir;

pub fn hash_dir(path: &Path) -> AppResult<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let relative = file
            .strip_prefix(path)
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

pub fn copy_dir_all(source: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir_all(target)?;
    for entry in WalkDir::new(source).into_iter().filter_map(Result::ok) {
        let relative = entry
            .path()
            .strip_prefix(source)
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


