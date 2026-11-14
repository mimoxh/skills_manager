//! Skills 双端同步（Phase 1 本地模型）。
//!
//! 本模块当前包含与网络无关、纯本地可测的构件：
//! - `pack`：确定性 zip 打包（跨端同一内容 → 同一字节 → 同一哈希）。
//! - `crypto`：口令派生密钥 + 确定性 nonce 的 AEAD 加解密。
//!
//! 传输层、同步引擎、watcher / poller 在后续 Phase 追加。

pub mod crypto;
pub mod engine;
pub mod manager;
pub mod pack;
pub mod runtime;
pub mod secrets;
pub mod transport;
pub mod watcher;

pub use crypto::Crypto;
pub use engine::{decide, merge_agent_tags, merge_tags, MergeDecision, VersionState};
pub use manager::SyncManager;
pub use pack::{pack_skill_dir, sha256_hex, skill_zip_hash, unpack_zip_to_dir};
pub use runtime::{ensure_kdf_salt, GcOutcome, PublishOutcome, PullOutcome, SyncRuntime};
pub use secrets::{
    KeyringSecretStore, MemorySecretStore, SecretStore, ENCRYPT_PASSWORD, SECRET_ACCESS_KEY,
};
pub use transport::{
    is_local_endpoint, local_root_from_endpoint, LocalDirTransport, S3Transport, SyncTransport,
};
pub use watcher::{spawn_hub_watcher, HubWatcher};
