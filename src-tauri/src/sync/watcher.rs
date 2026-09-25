//! 中枢文件监听：事件驱动 + 防抖 + 操作抑制。
//!
//! 监听程序所在目录 `skills/` 的增删改，防抖后触发一次同步。为避免"本机同步落中枢
//! → 触发上传 → 再同步"的自触发循环：
//! - 同步操作进行中 / 刚结束后一段时间（grace）忽略事件（`should_skip`）；
//! - 忽略 `.git` 与同步临时目录（`.sync-tmp-*`）产生的事件。

use crate::error::{AppError, AppResult};
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 防抖窗口：最后一次事件后静置这么久才触发。
const DEBOUNCE: Duration = Duration::from_millis(1500);

/// 持有底层 watcher；drop 即停止监听（回调停止 → 后台线程退出）。
pub struct HubWatcher {
    _watcher: notify::RecommendedWatcher,
}

pub fn spawn_hub_watcher<F, S>(hub_dir: PathBuf, on_change: F, should_skip: S) -> AppResult<HubWatcher>
where
    F: Fn() + Send + 'static,
    S: Fn() -> bool + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let _ = tx.send(result);
    })
    .map_err(|error| AppError::Message(format!("创建文件监听失败: {error}")))?;
    watcher
        .watch(&hub_dir, RecursiveMode::Recursive)
        .map_err(|error| AppError::Message(format!("监听中枢目录失败: {error}")))?;

    std::thread::spawn(move || {
        loop {
            // 等待首个相关事件
            let first = match rx.recv() {
                Ok(Ok(event)) if is_relevant(&event) => event,
                Ok(Ok(_)) | Ok(Err(_)) => continue,
                Err(_) => break,
            };
            let _ = first;

            // 防抖：静置 DEBOUNCE 内的后续事件一并吞掉
            let deadline = Instant::now() + DEBOUNCE;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match rx.recv_timeout(remaining) {
                    Ok(_) => continue,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            if !should_skip() {
                on_change();
            }
        }
    });

    Ok(HubWatcher { _watcher: watcher })
}

fn is_relevant(event: &notify::Event) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return false,
    }
    event.paths.iter().any(|path| {
        let text = path.to_string_lossy().replace('\\', "/");
        !text.contains("/.git/")
            && !text.contains("/.sync-tmp-")
            && !text.ends_with("/.DS_Store")
            && !text.ends_with("/Thumbs.db")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn triggers_after_hub_write_with_debounce() {
        let hub = tempfile::tempdir().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let _watcher = spawn_hub_watcher(
            hub.path().to_path_buf(),
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            || false,
        )
        .unwrap();

        std::fs::write(hub.path().join("SKILL.md"), b"hello").unwrap();

        // 最长等待 6s，至少触发一次
        let deadline = Instant::now() + Duration::from_secs(6);
        while counter.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn skip_callback_suppresses_trigger() {
        let hub = tempfile::tempdir().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let _watcher = spawn_hub_watcher(
            hub.path().to_path_buf(),
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            || true,
        )
        .unwrap();

        std::fs::write(hub.path().join("a.txt"), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(2500));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
