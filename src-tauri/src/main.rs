#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let service = match skill_sync_manager_lib::service::AppService::new() {
            Ok(s) => s,
            Err(e) => {
                skill_sync_manager_lib::cli::attach_console_if_needed(args.first().map(|s| s.as_str()));
                eprintln!("[skills-manager] 初始化失败: {}", e);
                std::process::exit(1);
            }
        };

        if let Err(e) = skill_sync_manager_lib::cli::run(service, &args) {
            eprintln!("[skills-manager] 错误: {}", e);
            std::process::exit(1);
        }
        return;
    }

    skill_sync_manager_lib::run();
}
