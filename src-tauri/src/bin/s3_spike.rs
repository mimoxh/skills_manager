//! Phase 0：OpenList S3 网关连通性验证（不进正式构建产物路径）。
//!
//! 用法（PowerShell，密钥只从环境变量读取，勿写入仓库）：
//! ```powershell
//! $env:S3_ENDPOINT="http://127.0.0.1:5246"
//! $env:S3_ACCESS_KEY="..."
//! $env:S3_SECRET_KEY="..."
//! $env:S3_BUCKET="test"
//! cargo run --manifest-path src-tauri/Cargo.toml --bin s3_spike
//! ```
//! 覆盖：PUT/GET/LIST/DELETE、中文/空格 key、重复 PUT、分页、较大对象。

use skill_sync_manager_lib::sync::{S3Transport, SyncTransport};
use std::process::exit;

fn require_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("缺少环境变量 {name}，请设置后重试。");
            exit(2);
        }
    }
}

fn main() {
    let endpoint = require_env("S3_ENDPOINT");
    let access_key = require_env("S3_ACCESS_KEY");
    let secret_key = require_env("S3_SECRET_KEY");
    let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "test".to_string());
    let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let transport = match S3Transport::new(&endpoint, &bucket, &region, &access_key, &secret_key) {
        Ok(transport) => transport,
        Err(error) => {
            eprintln!("构造 S3Transport 失败: {error}");
            std::process::exit(2);
        }
    };

    let mut failures = 0usize;
    let mut check = |name: &str, result: Result<(), String>| match result {
        Ok(()) => println!("PASS  {name}"),
        Err(error) => {
            failures += 1;
            println!("FAIL  {name}: {error}");
        }
    };

    // 1. 简单 PUT → GET
    check("put/get ascii", (|| {
        transport
            .put("spike/hello.txt", b"hello openlist")
            .map_err(|e| e.to_string())?;
        let got = transport
            .get("spike/hello.txt")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "GET 返回空".to_string())?;
        if got != b"hello openlist" {
            return Err("GET 内容不一致".to_string());
        }
        Ok(())
    })());

    // 2. 中文 + 空格 key
    let cn_key = "spike/目录/带 空格.txt";
    check("put/get chinese+space", (|| {
        transport
            .put(cn_key, "中文内容".as_bytes())
            .map_err(|e| e.to_string())?;
        let got = transport
            .get(cn_key)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "GET 返回空".to_string())?;
        if got != "中文内容".as_bytes() {
            return Err("GET 内容不一致".to_string());
        }
        Ok(())
    })());

    // 3. 重复 PUT 同 key（幂等覆盖）
    check("repeated put", (|| {
        transport
            .put("spike/hello.txt", b"hello openlist v2")
            .map_err(|e| e.to_string())?;
        let got = transport
            .get("spike/hello.txt")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "GET 返回空".to_string())?;
        if got != b"hello openlist v2" {
            return Err("覆盖后内容不一致".to_string());
        }
        Ok(())
    })());

    // 4. LIST 前缀
    check("list prefix", (|| {
        let keys = transport.list("spike/").map_err(|e| e.to_string())?;
        if !keys.iter().any(|key| key.ends_with("hello.txt")) {
            return Err(format!("列表中缺少 hello.txt: {keys:?}"));
        }
        println!("      list keys: {keys:?}");
        Ok(())
    })());

    // 5. 较大对象（16MB）
    check("large object 16MB", (|| {
        let payload = vec![0x5au8; 16 * 1024 * 1024];
        transport
            .put("spike/large.bin", &payload)
            .map_err(|e| e.to_string())?;
        let got = transport
            .get("spike/large.bin")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "GET 返回空".to_string())?;
        if got.len() != payload.len() {
            return Err(format!("长度不一致: {} != {}", got.len(), payload.len()));
        }
        if got != payload {
            return Err("内容不一致".to_string());
        }
        Ok(())
    })());

    // 6. DELETE → GET None
    check("delete", (|| {
        transport.delete("spike/large.bin").map_err(|e| e.to_string())?;
        if transport
            .get("spike/large.bin")
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("删除后仍能 GET".to_string());
        }
        Ok(())
    })());

    // 清理
    for key in ["spike/hello.txt", cn_key] {
        let _ = transport.delete(key);
    }

    if failures > 0 {
        println!("\n{failures} 项失败");
        std::process::exit(1);
    }
    println!("\n全部通过");
}
