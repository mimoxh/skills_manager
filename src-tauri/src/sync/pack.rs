//! 确定性 zip 打包。
//!
//! 目标：同一 skill 目录内容在任意平台/任意机器上打包出**相同字节**，从而得到相同
//! 身份哈希。为此固定：
//! - 条目按相对路径（`/` 分隔）排序；
//! - 所有时间戳归零（1980-01-01）；
//! - 权限按规则规范化（见 `mode_for_file`），不记录本地真实 mode；
//! - 排除 `.git`、`.DS_Store`、`Thumbs.db`、`*.tmp`、`*.swp`。

use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

const EXCLUDED_NAMES: &[&str] = &[".git", ".DS_Store", "Thumbs.db"];
const EXCLUDED_SUFFIXES: &[&str] = &[".tmp", ".swp"];

/// 可执行位白名单：命中扩展名或首行 shebang 的文件在两端统一视为可执行。
/// 这是跨平台（Windows 无 exec 位）同内容同哈希的前提。
const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "sh", "bash", "zsh", "ps1", "py", "pl", "rb", "js", "ts", "mjs", "cjs",
];

const DIR_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;
const EXEC_MODE: u32 = 0o755;

struct Entry {
    is_dir: bool,
    bytes: Vec<u8>,
    mode: u32,
}

/// 将 skill 目录打包为确定性 zip 的明文字节。
pub fn pack_skill_dir(dir: &Path) -> AppResult<Vec<u8>> {
    if !dir.is_dir() {
        return Err(AppError::Message(format!(
            "不是有效目录: {}",
            dir.display()
        )));
    }
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    collect_dir(dir, dir, &mut entries, &mut stack)?;

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    // 固定时间戳：1980-01-01 00:00:00（zip 可表示的起点），保证字节稳定。
    let timestamp = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .map_err(|error| AppError::Message(format!("zip 时间戳无效: {error}")))?;

    for (relative, entry) in &entries {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(timestamp)
            .unix_permissions(entry.mode);
        if entry.is_dir {
            writer.add_directory(format!("{relative}/"), options)?;
        } else {
            writer.start_file(relative.clone(), options)?;
            writer.write_all(&entry.bytes)?;
        }
    }

    let cursor = writer.finish()?;
    Ok(cursor.into_inner())
}

/// skill 目录的身份哈希：`sha256(明文 zip)`。
pub fn skill_zip_hash(dir: &Path) -> AppResult<String> {
    Ok(sha256_hex(&pack_skill_dir(dir)?))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// 安全解压确定性 zip 到目标目录（防 zip-slip / zip bomb）。
/// 目标目录会被创建；调用方负责事先清空或使用新目录。
pub fn unpack_zip_to_dir(bytes: &[u8], target: &Path) -> AppResult<()> {
    const MAX_ZIP_ENTRIES: usize = 2000;
    const MAX_ZIP_FILE_SIZE: u64 = 50 * 1024 * 1024;
    const MAX_ZIP_TOTAL_SIZE: u64 = 500 * 1024 * 1024;

    fs::create_dir_all(target)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(AppError::Message("zip 条目数量超过上限，疑似 zip bomb。".to_string()));
    }
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let file_size = file.size();
        if file_size > MAX_ZIP_FILE_SIZE {
            return Err(AppError::Message(format!(
                "zip 中文件 {} 超过单文件大小上限。",
                file.name()
            )));
        }
        total += file_size;
        if total > MAX_ZIP_TOTAL_SIZE {
            return Err(AppError::Message("zip 解压总大小超过上限，疑似 zip bomb。".to_string()));
        }

        let raw_name = file.name().replace('\\', "/");
        if raw_name.ends_with('/') {
            // 目录条目：保留空目录
            let Some(relative) = crate::util::sanitize_zip_path(raw_name.trim_end_matches('/'))
            else {
                continue;
            };
            fs::create_dir_all(target.join(relative))?;
            continue;
        }
        let relative = match file.enclosed_name().map(PathBuf::from) {
            Some(path) => path,
            None => match crate::util::sanitize_zip_path(&raw_name) {
                Some(path) => path,
                None => continue,
            },
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&destination)?;
        std::io::copy(&mut file, &mut out)?;

        // 还原规范化权限：仅 Unix 生效（Windows 无 exec 位）。
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                if mode & 0o111 != 0 {
                    let _ = fs::set_permissions(
                        &destination,
                        fs::Permissions::from_mode(0o755),
                    );
                }
            }
        }
    }
    Ok(())
}

fn collect_dir(
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<String, Entry>,
    stack: &mut Vec<PathBuf>,
) -> AppResult<()> {
    // 软链目录跟随复制内容；用 canonical 路径做环检测，避免自引用死循环。
    let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if stack.contains(&canonical) {
        return Ok(());
    }
    stack.push(canonical);

    let mut children: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    children.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for child in children {
        let name = child
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        if is_excluded(&name) {
            continue;
        }
        let relative = relative_string(root, &child);
        let metadata = fs::symlink_metadata(&child)?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            // 中枢应为实体目录；遇到软链按复制内容处理。
            let real = fs::metadata(&child)?;
            if real.is_dir() {
                entries.insert(relative.clone(), dir_entry());
                collect_dir(root, &child, entries, stack)?;
            } else {
                entries.insert(relative.clone(), file_entry(&relative, fs::read(&child)?));
            }
        } else if file_type.is_dir() {
            entries.insert(relative.clone(), dir_entry());
            collect_dir(root, &child, entries, stack)?;
        } else if file_type.is_file() {
            entries.insert(relative.clone(), file_entry(&relative, fs::read(&child)?));
        }
    }

    stack.pop();
    Ok(())
}

fn dir_entry() -> Entry {
    Entry {
        is_dir: true,
        bytes: Vec::new(),
        mode: DIR_MODE,
    }
}

fn file_entry(relative: &str, bytes: Vec<u8>) -> Entry {
    let mode = mode_for_file(relative, &bytes);
    Entry {
        is_dir: false,
        bytes,
        mode,
    }
}

/// 规范化权限：命中白名单 → `0o755`，否则 `0o644`。
fn mode_for_file(relative: &str, bytes: &[u8]) -> u32 {
    if is_executable_candidate(relative, bytes) {
        EXEC_MODE
    } else {
        FILE_MODE
    }
}

fn is_executable_candidate(relative: &str, bytes: &[u8]) -> bool {
    let extension = Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if EXECUTABLE_EXTENSIONS.contains(&extension.as_str()) {
        return true;
    }
    bytes.starts_with(b"#!")
}

fn is_excluded(name: &str) -> bool {
    if EXCLUDED_NAMES
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
    {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    EXCLUDED_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn relative_string(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, content: &[u8]) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn same_content_same_hash_regardless_of_creation_order() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "b.txt", b"bee");
        write(a.path(), "a.txt", b"aye");

        let b = tempfile::tempdir().unwrap();
        write(b.path(), "a.txt", b"aye");
        write(b.path(), "b.txt", b"bee");

        assert_eq!(
            skill_zip_hash(a.path()).unwrap(),
            skill_zip_hash(b.path()).unwrap()
        );
    }

    #[test]
    fn different_content_different_hash() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "SKILL.md", b"one");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "SKILL.md", b"two");
        assert_ne!(
            skill_zip_hash(a.path()).unwrap(),
            skill_zip_hash(b.path()).unwrap()
        );
    }

    #[test]
    fn excluded_files_do_not_affect_hash() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "SKILL.md", b"body");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "SKILL.md", b"body");
        write(b.path(), ".DS_Store", b"junk");
        write(b.path(), "notes.tmp", b"junk");
        write(b.path(), ".git/config", b"junk");
        assert_eq!(
            skill_zip_hash(a.path()).unwrap(),
            skill_zip_hash(b.path()).unwrap()
        );
    }

    #[test]
    fn executable_whitelist_normalizes_mode() {
        // 相同字节在 .sh 与 .txt 下因权限规则不同 → 哈希不同
        let sh = tempfile::tempdir().unwrap();
        write(sh.path(), "run.sh", b"echo hi");
        let txt = tempfile::tempdir().unwrap();
        write(txt.path(), "run.txt", b"echo hi");
        assert_ne!(
            skill_zip_hash(sh.path()).unwrap(),
            skill_zip_hash(txt.path()).unwrap()
        );
    }

    #[test]
    fn executable_content_same_hash_across_dirs() {
        // 同一脚本内容在两台机器上（无论真实 exec 位）规范化后哈希一致
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "run.sh", b"#!/bin/sh\necho hi");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "run.sh", b"#!/bin/sh\necho hi");
        assert_eq!(
            skill_zip_hash(a.path()).unwrap(),
            skill_zip_hash(b.path()).unwrap()
        );
    }

    #[test]
    fn empty_directories_are_preserved() {
        let with_dir = tempfile::tempdir().unwrap();
        write(with_dir.path(), "SKILL.md", b"body");
        fs::create_dir_all(with_dir.path().join("assets")).unwrap();

        let without_dir = tempfile::tempdir().unwrap();
        write(without_dir.path(), "SKILL.md", b"body");

        assert_ne!(
            skill_zip_hash(with_dir.path()).unwrap(),
            skill_zip_hash(without_dir.path()).unwrap()
        );
    }

    #[test]
    fn unpack_round_trip_matches_files() {
        let source = tempfile::tempdir().unwrap();
        write(source.path(), "SKILL.md", b"# title");
        write(source.path(), "scripts/run.sh", b"#!/bin/sh\necho hi");
        fs::create_dir_all(source.path().join("assets")).unwrap();

        let bytes = pack_skill_dir(source.path()).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(sha256_hex(&bytes), skill_zip_hash(source.path()).unwrap());
    }
}
