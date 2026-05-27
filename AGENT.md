# AGENT.md

本项目是一个桌面应用，用来集中管理本地 skills 仓库，并把选中的 skill 同步安装到多个 agent 的 skills 目录中。包名为 `skill-sync-manager`，产品名为 `Skills Manager`。当前主 UI 为 Tauri 2 + React + Tailwind CSS 的 WebView 桌面界面；原生 egui 桌面版仅作为 legacy 备用入口保留。

## 项目定位

- 主 UI 为 Tauri + React 桌面应用，默认首屏为 `Skills` 控制台，提供侧边栏导航和四个视图：概览、Skills、Agents、设置。
- 后端通过 `AppService` 业务层扫描本地 skills 仓库、检测 agent skills 目录、保存自定义 agent、安装/卸载/回滚 skill、导入 folder/zip/upload。
- 本地状态用 SQLite 保存，包括主仓库路径、手动添加的 agents、已安装 fingerprint、操作历史和备份位置。
- 同步方式是目录级复制：每个 skill 以 manifest 的 `id` 作为目标子目录名安装到 agent 的 `skillsPath` 下。
- 支持从各 agent 目录自动识别 skills 并按标题去重分组；点击 skill 后通过模态弹窗选择目标 agent 和冲突策略，再同步到指定 agent。
- 支持配置发现路径（discovery paths），自动扫描额外目录来发现 skills。

## 技术栈

- 主 UI：React 18、TypeScript、Vite 5、Tailwind CSS 4、shadcn/ui（Radix UI 原语）、lucide-react（通过 Tauri 2 桌面壳）。
- 备选原生 UI：eframe 0.29、egui 0.29、rfd 0.15。
- 后端：Rust 2021、rusqlite、serde/serde_json/serde_yaml、walkdir、sha2、chrono、dirs、thiserror、zip。
- 数据库：应用本地数据目录下的 `skill-sync-manager/state.db`。
- 备份：应用本地数据目录下的 `skill-sync-manager/backups`。
- 导入缓存：应用本地数据目录下的 `skill-sync-manager/imports`。

## 常用命令

在项目根目录运行：

```powershell
npm install
npm run desktop:dev      # 启动主 UI（Tauri + React）
npm run native:dev       # 启动主 UI（兼容旧脚本名，等同 tauri dev）
npm run native:legacy    # 启动 legacy egui 桌面版
npm run native:build     # 构建 Tauri 桌面应用
npm run dev              # 仅启动 Vite 开发服务器
npm run build            # tsc + vite build
npm run test:rust        # cargo test
```

命令含义：

- `npm run desktop:dev`：启动 Tauri + React 桌面应用，Tauri 会先执行 `npm run dev` 启动 Vite（端口 5173）。
- `npm run native:dev`：兼容旧脚本名，当前同样执行 `tauri dev`，启动主 Tauri + React UI。
- `npm run native:legacy`：直接运行 `cargo run --bin native` 启动 legacy egui 桌面应用。
- `npm run native:build`：执行 `tauri build`，构建生产版本，生成安装包到 `src-tauri/target/release/bundle/`。
- `npm run dev`：仅启动 Vite 开发服务器，固定端口 `5173`，绑定 `127.0.0.1`。
- `npm run build`：先运行 `tsc`，再运行 `vite build`。
- `npm run test:rust`：执行 `cargo test --manifest-path src-tauri/Cargo.toml`。

## 目录结构

```text
.
├── index.html                    # HTML 模板（Inter 字体 CDN）
├── package.json
├── tsconfig.json
├── vite.config.ts                # Vite + Tailwind CSS 插件配置
├── src/                          # 主 UI（React + Tailwind CSS）
│   ├── main.tsx                  # React 入口
│   ├── App.tsx                   # 应用根组件（布局组合）
│   ├── index.css                 # Tailwind 入口 + 设计 token
│   ├── api.ts                    # Tauri invoke 封装
│   ├── types.ts                  # TypeScript 类型定义
│   ├── lib/
│   │   └── utils.ts              # cn() 工具函数
│   ├── hooks/
│   │   └── useAppState.ts        # 共享状态和业务逻辑
│   ├── components/
│   │   ├── ui/                   # shadcn/ui 基础组件
│   │   │   ├── button.tsx
│   │   │   ├── input.tsx
│   │   │   ├── card.tsx
│   │   │   ├── badge.tsx
│   │   │   ├── separator.tsx
│   │   │   ├── scroll-area.tsx
│   │   │   └── tooltip.tsx
│   │   ├── layout/               # 布局组件
│   │   │   ├── Titlebar.tsx      # 自定义无边框标题栏
│   │   │   ├── Sidebar.tsx       # 侧边栏导航
│   │   │   └── CommandBar.tsx    # 搜索栏 + 操作按钮
│   │   └── views/                # 页面视图
│   │       ├── OverviewView.tsx
│   │       ├── SkillsView.tsx
│   │       ├── AgentsView.tsx
│   │       └── SettingsView.tsx
│   └── styles.css                # 旧样式（已弃用，保留供参考）
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs               # 默认入口，调用 run() 启动 Tauri WebView
        ├── bin/
        │   └── native.rs         # legacy egui 入口，调用 run_native()
        ├── lib.rs                 # 模块导出 + Tauri/egui 双入口
        ├── commands.rs            # Tauri command 层
        ├── service.rs             # AppService 业务逻辑层
        ├── adapter.rs             # agent 适配层（DirectoryAdapter）
        ├── store.rs               # SQLite 持久化层
        ├── manifest.rs            # skill manifest 扫描与解析
        ├── hash.rs                # SHA-256 fingerprint + 目录复制
        ├── models.rs              # 数据模型
        ├── native_app.rs          # 备选原生 egui 桌面 UI
        └── error.rs               # 统一错误类型
```

## 主 UI 概览（Tauri + React）

`src/App.tsx` 是应用根组件，组合 Titlebar、Sidebar、CommandBar 和页面视图。默认进入 `skills`，所有状态逻辑提取到 `hooks/useAppState.ts`。

四个视图：

- **概览（OverviewView）**：指标卡、快速操作（导入文件夹/zip）和入口跳转。
- **Skills（SkillsView）**：现代控制台式首屏，包含导入区域、指标、可滚动 skill 列表和状态 badge。点击某个 skill 会打开同步模态弹窗，在弹窗内勾选目标 agent，并选择 `backupOverwrite` / `skip` / `rename` 冲突策略后执行 `syncGroupedSkill(title, null, targetAgentIds, conflictPolicy)`。
- **Agents（AgentsView）**：agent 列表、已安装/缺失统计和自定义 agent 添加表单。
- **设置（SettingsView）**：主仓库路径设置、发现路径管理（添加/删除额外扫描目录）、关于信息。

UI 特性：

- 现代控制台风格：列表为主、信息密度适中、状态徽标清晰，避免恢复独立同步页和全局“同步选中”按钮。
- 自定义无边框标题栏（可拖拽、最小化/最大化/关闭）。
- 侧边栏导航，活跃项有 accent 色条指示。
- 支持拖放导入（文件夹和 .zip 压缩包）。
- Tailwind CSS 4 + shadcn/ui 组件（基于 Radix UI 原语）。

## 备选原生 UI（egui）

`src-tauri/src/native_app.rs` 是基于 eframe/egui 的原生桌面 UI，frameless 窗口（1280x820），含自定义标题栏和侧边栏导航。通过 `npm run native:legacy` 启动。

注意：egui 版本在 Windows 11 上存在闪烁问题（glow OpenGL 后端 + frameless window + DWM 合成），已不推荐作为主 UI。

## Tauri API

`src-tauri/src/lib.rs` 注册这些 commands：

- `set_repository(path)`：校验非空路径，创建目录，保存主 skills 仓库路径。
- `get_repository()`：读取主 skills 仓库路径。
- `scan_skills()`：扫描主仓库内的 skill manifests。
- `import_skill_upload(file_name, files)`：上传导入 skill，支持 zip 和文件夹。
- `detect_agents()`：检测内置 agent skills 目录。
- `list_agents()`：读取手动保存的 agents。
- `add_agent(profile)`：校验并保存 agent profile。
- `remove_agent(agent_id)`：删除 agent 与相关安装记录。
- `list_install_state()`：对所有 skill/agent 组合计算安装状态。
- `scan_agent_skills()`：从各 agent 目录扫描 skills 并按标题去重分组。
- `preview_sync(agent_id)`：返回指定 agent 兼容的 skill 与状态。
- `install_skills(skill_ids, agent_ids, conflict_policy)`：批量安装或更新。
- `sync_grouped_skill(title, source_agent_id, target_agent_ids, conflict_policy)`：按分组标题同步 skill 到多个 agent。
- `uninstall_skill(skill_id, agent_id)`：卸载并备份目标目录。
- `rollback_last(agent_id, skill_id)`：用最近一次备份恢复目标目录。
- `add_discovery_path(path, label, skills_subdir)`：添加发现路径。
- `remove_discovery_path(path)`：删除发现路径。
- `list_discovery_paths()`：列出所有发现路径。

## 后端模块

- `main.rs`：默认应用入口，调用 `run()` 启动 Tauri + React WebView。
- `bin/native.rs`：legacy 应用入口，调用 `run_native()` 启动原生 egui 桌面版。
- `lib.rs`：模块导出，提供 `run()`（Tauri）和 `run_native()`（egui）两个入口。
- `service.rs`：`AppService` 业务逻辑层，封装 store + adapter，提供所有业务操作。Tauri commands 和 native UI 共用此层。
- `commands.rs`：Tauri command 编排层，每个 command 委托给 `AppService`。
- `native_app.rs`：备选原生 egui 桌面 UI。
- `adapter.rs`：agent 适配层。当前实现是 `DirectoryAdapter`，支持 Codex、Claude、ClaudeCode、Cursor、Windsurf、Aider 和 custom。
- `store.rs`：SQLite 持久化层，包含 migrations、settings、agents、installs、operations、discovery_paths、backup root 和 import root。
- `manifest.rs`：扫描和解析 `skill.json`、`skill.yaml`、`skill.yml`，并做基础 manifest 校验。
- `hash.rs`：计算目录 SHA-256 fingerprint，递归复制目录。
- `models.rs`：Rust 侧数据模型，serde 使用 camelCase。
- `error.rs`：统一 `AppError`，可序列化为 Tauri 前端可接收的字符串。

## Skill Manifest 约定

扫描器会在主仓库下最多 3 层深度寻找这些文件名：

- `skill.json`
- `skill.yaml`
- `skill.yml`

manifest 字段：

```json
{
  "id": "demo",
  "name": "Demo Skill",
  "version": "1.0.0",
  "description": "Optional description",
  "tags": ["optional"],
  "supportedAgents": ["codex", "claude", "claudeCode", "cursor", "windsurf", "aider", "custom"],
  "entry": "SKILL.md",
  "files": ["SKILL.md"]
}
```

必填字段：`id`、`name`、`version`、`supportedAgents`、`files`。

兼容性规则：

- `supportedAgents` 可包含具体 agent 类型或 `*`（兼容所有）。
- 安装目标目录固定为 `{agent.skillsPath}/{skill.id}`。

## Agent 检测

内置检测逻辑位于 `DirectoryAdapter::detect()`：

- Codex：`%USERPROFILE%\.codex\skills`
- Claude：`%APPDATA%\Claude\skills`
- Claude Code：`%USERPROFILE%\.claude\skills`
- Cursor：`%USERPROFILE%\.cursor\skills`
- Windsurf：`%USERPROFILE%\.windsurf\skills`
- Aider：`%USERPROFILE%\.aider\skills`

只有路径已存在时才会被自动检测到。自定义 agent 通过前端保存，后端会创建其 `skillsPath`。

## 数据模型

`models.rs` 定义的主要类型：

- `AgentType` 枚举：Codex / Claude / ClaudeCode / Cursor / Windsurf / Aider / Custom。
- `AgentProfile`：agent 配置（id、name、agent_type、skills_path、adapter_config）。
- `SkillManifest`：skill 清单。
- `SkillSummary`：扫描结果（manifest、source_path、fingerprint、manifest_path）。
- `GroupedSkill`：按标题聚合的 skill 分组（title、best_copy、copies、installed_agent_ids、missing_agent_ids）。
- `ConflictPolicy` 枚举：Prompt / BackupOverwrite / Skip / Rename。
- `InstallStatus` 枚举：Installed / Stale / Conflict / Missing。
- `InstallState`：单个 skill/agent 组合的安装状态。
- `InstallResult`：安装/同步操作结果。
- `SyncCandidate`：预览同步时的候选 skill 及其状态。
- `ImportSkillFile` / `ImportSkillResult`：上传导入相关。
- `DiscoveryPathEntry`：发现路径条目（path、label、skills_subdir）。

## 冲突策略

`ConflictPolicy` 支持：

- `prompt`：遇到已存在目标目录时返回错误，要求用户选择显式策略。
- `backupOverwrite`：先复制现有目标目录到 backup root，再删除目标并复制源 skill。
- `skip`：跳过已有目标目录。
- `rename`：把新 skill 安装到 `{skill.id}-{yyyyMMddHHmmss}`，不覆盖原目录。

## 当前注意事项

- Tauri + React 是主入口，`main.rs` 调用 `run()`；egui 只通过 `src-tauri/src/bin/native.rs` 和 `npm run native:legacy` 保留。
- `src-tauri/Cargo.toml` 设置了 `default-run = "skill-sync-manager"`；新增 bin 时不要移除，否则 `tauri dev` 会因为多 binary 无法判断启动目标。
- 当前 React UI 不再保留独立同步页，也不保留全局“同步选中”按钮；同步入口在 `SkillsView` 的单个 skill 点击模态弹窗中。
- 原生 UI 和 Web UI 均没有删除 agent 的入口，`remove_agent` API 已封装但未暴露在 UI 中。
- `previewSync` API 已封装，但前端当前没有直接使用。
- `manifest.files` 目前只做非空校验；实际安装使用整个 skill 目录复制，不按 `files` 白名单过滤。
- `rename` 策略会把 skill 安装到带时间戳的新目录，但记录的 `skill_id` 仍是原 manifest id。后续卸载逻辑使用 `{skillsPath}/{skill_id}`，因此对 renamed 安装的卸载/回滚语义需要特别小心。
- `rollback_last()` 恢复文件系统但不更新 `installs` 表中的 fingerprint。
- `list_install_state()` 在 repository 未配置或扫描失败时会吞掉 skills 错误并返回空状态。
- Tauri capability 当前只有 `core:default`。commands 是本应用自定义 invoke，不依赖额外 shell/fs 插件权限。
- Vite 开发服务器绑定 `127.0.0.1:5173`（因 Windows 防火墙限制，不使用 localhost）。

## 开发建议

- 后端改动后优先运行 `npm run test:rust`。
- 主 UI 改动后运行 `npm run desktop:dev` 查看效果。
- 原生 legacy UI 改动后运行 `npm run native:legacy` 查看效果。
- 前端类型改动后运行 `npm.cmd exec tsc -- --noEmit` 验证。
- 生产前端改动后运行 `npm.cmd run build`；如果 Windows 沙箱拦截 Vite/esbuild 读取配置，可在正常权限下重跑。
- 涉及业务逻辑时，修改 `service.rs`，Tauri commands 和 native UI 共用同一层。
- 涉及 Tauri command 参数时，同步检查 `src/api.ts`、`src/types.ts`、`src-tauri/src/models.rs` 和 command 函数签名。
- 涉及安装/卸载/回滚时，优先增加 Rust 单元测试，使用 `tempfile` 隔离源目录、目标目录和备份目录。
- 不要直接修改用户真实 agent skills 目录做测试；使用 custom agent 指向临时目录更安全。
- shadcn/ui 组件在 `src/components/ui/` 中，基于 Radix UI 原语，样式通过 Tailwind CSS 控制。
- 后续如果调整功能、架构、启动入口、脚本或主要 UI 流程，需要同步更新本文件；不要把未验证的旧行为写进文档。
