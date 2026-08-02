use crate::error::{AppError, AppResult};
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
        hasher.update(fs::read(&file)?);
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
