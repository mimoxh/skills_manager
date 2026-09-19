//! 单机双设备同步演示：本地文件夹当共享 bucket，无需第二台电脑 / S3。
//!
//! ```powershell
//! cargo run --manifest-path src-tauri/Cargo.toml --bin sync_local_demo
//! # 可选：指定共享目录
//! $env:SYNC_DEMO_BUCKET="D:/tmp/skills-sync-bucket"
//! cargo run --manifest-path src-tauri/Cargo.toml --bin sync_local_demo
//! ```

use skill_sync_manager_lib::models::{AgentProfile, AgentType, SyncConfig};
use skill_sync_manager_lib::service::AppService;
use skill_sync_manager_lib::sync::{MemorySecretStore, SecretStore, ENCRYPT_PASSWORD};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn write_skill(root: &Path, dir: &str, title: &str, markdown: &str) {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    let manifest = serde_json::json!({
        "id": dir,
        "name": title,
        "version": "1.0.0",
        "supportedAgents": ["*"],
        "files": ["SKILL.md"]
    });
    fs::write(skill_dir.join("skill.json"), manifest.to_string()).expect("write skill.json");
    fs::write(skill_dir.join("SKILL.md"), markdown).expect("write SKILL.md");
}

fn make_device(
    label: &str,
    data_dir: &Path,
    hub: &Path,
    bucket: &Path,
) -> AppService {
    let secrets = Arc::new(MemorySecretStore::new());
    secrets
        .set(ENCRYPT_PASSWORD, "sync-demo-password")
        .expect("set password");
    let service = AppService::from_store(
        Arc::new(
            skill_sync_manager_lib::store::AppStore::with_data_dir(data_dir.to_path_buf())
                .expect("open store"),
        ),
        secrets,
    )
    .expect("build service");
    service
        .add_agent(AgentProfile {
            id: format!("universal:{}", hub.display()),
            name: format!("Universal Hub ({label})"),
            agent_type: AgentType::Universal,
            skills_path: hub.to_string_lossy().to_string(),
            adapter_config: None,
            user_tags: Vec::new(),
            supports_universal: true,
        })
        .expect("register hub");
    service
        .sync_set_config(
            SyncConfig {
                enabled: true,
                endpoint: format!("local://{}", bucket.display()),
                bucket: String::new(),
                access_key_id: String::new(),
                encrypt: true,
                poll_secs: 3600,
                ..SyncConfig::default()
            },
            None,
            Some("sync-demo-password".into()),
        )
        .expect("set sync config");
    service
}

fn main() {
    let root = std::env::temp_dir().join(format!(
        "skills-sync-demo-{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    let bucket = std::env::var_os("SYNC_DEMO_BUCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("bucket"));
    let data_a = root.join("device-a-data");
    let data_b = root.join("device-b-data");
    let hub_a = root.join("device-a-hub");
    let hub_b = root.join("device-b-hub");
    for dir in [&bucket, &data_a, &data_b, &hub_a, &hub_b] {
        fs::create_dir_all(dir).expect("create demo dirs");
    }

    println!("== Skills 单机双设备同步演示 ==");
    println!("共享 bucket : {}", bucket.display());
    println!("工作目录    : {}", root.display());

    let device_a = make_device("A", &data_a, &hub_a, &bucket);
    let device_b = make_device("B", &data_b, &hub_b, &bucket);

    let status_a = device_a.sync_status().expect("status A");
    let status_b = device_b.sync_status().expect("status B");
    println!("设备 A id={} name={}", status_a.device_id, status_a.device_name);
    println!("设备 B id={} name={}", status_b.device_id, status_b.device_name);
    println!("A configured={}  B configured={}", status_a.configured, status_b.configured);

    write_skill(&hub_a, "demo-local-sync", "Demo Local Sync", "# Demo\n\nwritten on device A");
    println!("\n[1] 设备 A 写入 skill 并同步…");
    let after_a = device_a.sync_now().expect("sync A");
    println!("    A last_error={:?}", after_a.last_error);
    println!(
        "    A 中枢路径={}",
        device_a.ensure_hub_agent().map(|h| h.skills_path).unwrap_or_default()
    );

    println!("[2] 设备 B 同步拉取…");
    let after_b = device_b.sync_now().expect("sync B");
    println!("    B last_error={:?}", after_b.last_error);

    let pulled = hub_b.join("demo-local-sync").join("SKILL.md");
    if pulled.exists() {
        let body = fs::read_to_string(&pulled).unwrap_or_default();
        println!("    OK：B 中枢已出现 skill，内容片段：{}", body.lines().next().unwrap_or(""));
    } else {
        eprintln!("    FAIL：B 未拉到 demo-local-sync");
        std::process::exit(1);
    }

    println!("\n[3] 制造双端修改冲突…");
    write_skill(&hub_a, "demo-local-sync", "Demo Local Sync", "# Demo\n\nedit from A");
    device_a.sync_now().expect("sync A after edit");
    write_skill(&hub_b, "demo-local-sync", "Demo Local Sync", "# Demo\n\nedit from B");
    let after_conflict = device_b.sync_now().expect("sync B after edit");
    let conflicts = device_b.sync_list_conflicts().expect("list conflicts");
    println!("    B pending_conflicts={}", after_conflict.pending_conflicts);
    for conflict in &conflicts {
        println!(
            "    conflict skill={} kind={:?} local={:?} remote={:?}",
            conflict.name, conflict.kind, conflict.local_hash, conflict.remote_hash
        );
    }
    if let Some(conflict) = conflicts.first() {
        println!("[4] 用「远端覆盖」解决 {}", conflict.name);
        device_b
            .sync_resolve_conflict(&conflict.skill_id, skill_sync_manager_lib::models::SyncConflictChoice::Remote)
            .expect("resolve conflict");
        let body = fs::read_to_string(hub_b.join("demo-local-sync").join("SKILL.md")).unwrap_or_default();
        println!("    解决后 B 内容：{}", body.lines().last().unwrap_or(""));
    }

    println!("\n演示完成。设置页也可把 Endpoint 填成 local://{} 做 UI 验证。", bucket.display());
    println!("同机双开第二个桌面实例时设置：SKILLS_MANAGER_DATA_DIR=<另一数据目录>");
}
