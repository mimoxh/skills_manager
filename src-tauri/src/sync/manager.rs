//! 后台同步执行器：串行化同步操作 + 60s 兜底轮询。
//!
//! 所有同步入口（手动 `sync_now`、后台轮询）经 `run_lock` 串行执行，避免同一
//! `state.json` / 中枢被并发修改。线程用 `std::thread` + `mpsc`（`ureq` 阻塞，
//! 不引入 async）。收到 `Shutdown` 或通道断开会退出。

use crate::service::AppService;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// 同步刚结束后忽略 watcher 事件的窗口，避免本机落中枢触发自同步。
const WATCHER_GRACE: Duration = Duration::from_secs(3);

enum Signal {
    SyncNow,
    Shutdown,
}

pub struct OperationGuard<'a> {
    manager: &'a SyncManager,
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.manager.end_operation();
    }
}

struct Inner {
    sender: Mutex<Option<Sender<Signal>>>,
    running: AtomicBool,
    run_lock: Mutex<()>,
    in_operation: AtomicBool,
    last_operation_end: Mutex<Option<Instant>>,
}

#[derive(Clone)]
pub struct SyncManager {
    inner: Arc<Inner>,
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                sender: Mutex::new(None),
                running: AtomicBool::new(false),
                run_lock: Mutex::new(()),
                in_operation: AtomicBool::new(false),
                last_operation_end: Mutex::new(None),
            }),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::SeqCst)
    }

    /// 串行执行锁：同步入口进入前获取。
    pub fn lock(&self) -> MutexGuard<'_, ()> {
        self.inner
            .run_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 标记同步操作开始/结束，供 watcher 抑制自触发。
    pub fn begin_operation(&self) {
        self.inner.in_operation.store(true, Ordering::SeqCst);
    }

    pub fn end_operation(&self) {
        self.inner.in_operation.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.inner.last_operation_end.lock() {
            *guard = Some(Instant::now());
        }
    }

    /// watcher 是否应忽略当前事件（操作中 / 刚结束的 grace 窗口）。
    pub fn is_suppressed(&self) -> bool {
        if self.inner.in_operation.load(Ordering::SeqCst) {
            return true;
        }
        if let Ok(guard) = self.inner.last_operation_end.lock() {
            if let Some(end) = *guard {
                return end.elapsed() < WATCHER_GRACE;
            }
        }
        false
    }

    /// RAII：进入作用域即标记操作中，drop 时结束并记录时间。
    pub fn operation_guard(&self) -> OperationGuard<'_> {
        self.begin_operation();
        OperationGuard { manager: self }
    }

    /// 启动后台轮询线程（幂等）。
    pub fn start(&self, service: AppService, poll_secs: u64) {
        if self.inner.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        match self.inner.sender.lock() {
            Ok(mut guard) => *guard = Some(sender),
            Err(poisoned) => *poisoned.into_inner() = Some(sender),
        }
        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            let poll = Duration::from_secs(poll_secs.max(5));
            loop {
                match receiver.recv_timeout(poll) {
                    Ok(Signal::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                    Ok(Signal::SyncNow) | Err(RecvTimeoutError::Timeout) => {
                        let _ = service.sync_auto_once();
                    }
                }
            }
            inner.running.store(false, Ordering::SeqCst);
        });
        // 启动后立即触发一次，避免等到第一个轮询周期。
        self.trigger();
    }

    /// 通知后台线程立即同步一次。
    pub fn trigger(&self) {
        if let Ok(guard) = self.inner.sender.lock() {
            if let Some(sender) = guard.as_ref() {
                let _ = sender.send(Signal::SyncNow);
            }
        }
    }

    /// 停止后台线程（幂等）。
    pub fn stop(&self) {
        if let Ok(mut guard) = self.inner.sender.lock() {
            if let Some(sender) = guard.take() {
                let _ = sender.send(Signal::Shutdown);
            }
        }
        self.inner.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_stop_toggle_running() {
        let service = AppService::in_memory().unwrap();
        let manager = SyncManager::new();
        assert!(!manager.is_running());
        manager.start(service, 60);
        assert!(manager.is_running());
        // 幂等：重复 start 不改变状态
        manager.start(AppService::in_memory().unwrap(), 60);
        assert!(manager.is_running());
        manager.stop();
        assert!(!manager.is_running());
    }

    #[test]
    fn auto_once_is_noop_when_disabled() {
        let service = AppService::in_memory().unwrap();
        assert!(service.sync_auto_once().is_ok());
    }
}
