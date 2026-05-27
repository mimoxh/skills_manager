# AGENT.md

本项目是一个 Rust 桌面应用，用来集中管理本地 skills 仓库，并把选中的 skill 同步安装到多个 agent 的 skills 目录中。当前包名为 `skill-sync-manager`，产品名为 `Skills Manager`。主入口为原生 egui 桌面版，同时保留 Tauri 2 + React 作为备选 Web UI。

## 项目定位

- 主 UI 为原生 egui/eframe 桌面应用，提供侧边栏导航和五个视图：概览、Skills、Agents、同步、设置。
- 后端通过 `AppService` 业务层扫描本地 skills 仓库、检测 Codex/Claude skills 目录、保存自定义 agent、安装/卸载/回滚 skill、导入 folder/zip/upload。
- 本地状态用 SQLite 保存，包括主仓库路径、手动添加的 agents、已安装 fingerprint、操作历史和备份位置。
- 同步方式是目录级复制：每个 skill 以 manifest 的 `id` 作为目标子目录名安装到 agent 的 `skillsPath` 下。
- 支持从各 agent 目录自动识别 skills 并按标题去重分组，可选择最佳版本同步到其他 agent。

## 技术栈

- 原生 UI：eframe 0.29、egui 0.29、rfd 0.15（文件对话框）。
- 备选 Web UI：React 18、TypeScript、Vite、lucide-react（通过 Tauri 2 桌面壳）。
- 后端：Rust 2021、rusqlite、serde/serde_json/serde_yaml、walkdir、sha2、chrono、dirs、thiserror、zip。
- 数据库：应用本地数据目录下的 `skill-sync-manager/state.db`。
- 备份：应用本地数据目录下的 `skill-sync-manager/backups`。
- 导入缓存：应用本地数据目录下的 `skill-sync-manager/imports`。

## 常用命令

在项目根目录运行：

```powershell
npm install
npm run native:dev       # 启动原生 egui 桌面版（主入口）
npm run native:build     # 编译 release 版原生应用
npm run dev              # 启动 Vite（Web UI 用）
npm run build            # tsc + vite build（Web UI 用）
npm run tauri dev        # 启动 Tauri + React 桌面版（备选）
npm run test:rust        # cargo test
```

命令含义：

- `npm run native:dev`：直接运行 `cargo run` 启动原生 egui 桌面应用。
- `npm run native:build`：`cargo build --release` 编译原生应用。
- `npm run dev`：启动 Vite，固定端口 `1420`，供 Tauri devUrl 使用。
- `npm run build`：先运行 `tsc`，再运行 `vite build`。
- `npm run tauri dev`：启动 Tauri + React 桌面应用，Tauri 会先执行 `npm run dev`。
- `npm run test:rust`：执行 `cargo test --manifest-path src-tauri/Cargo.toml`。

## 目录结构

```text
.
├── index.html
├── package.json
├── tsconfig.json
├── src/                          # 备选 Web UI（React + Tauri）
│   ├── main.tsx
│   ├── App.tsx
│   ├── api.ts
│   ├── types.ts
│   └── styles.css
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs               # 入口，调用 run_native()
        ├── lib.rs                 # 模块导出 + Tauri/egui 双入口
        ├── commands.rs            # Tauri command 层（Web UI 用）
        ├── service.rs             # AppService 业务逻辑层
        ├── adapter.rs             # agent 适配层
        ├── store.rs               # SQLite 持久化层
        ├── manifest.rs            # skill manifest 扫描与解析
        ├── hash.rs                # SHA-256 fingerprint + 目录复制
        ├── models.rs              # 数据模型
        ├── native_app.rs          # 原生 egui 桌面 UI
        └── error.rs               # 统一错误类型
```

## 原生 UI 概览

`src-tauri/src/native_app.rs` 是基于 eframe/egui 的原生桌面 UI，frameless 窗口（1280x820），含自定义标题栏和侧边栏导航。

五个视图：

- **概览（Overview）**：显示 skill/agent/待同步数量指标卡、最近操作结果、导入和同步快捷入口。
- **Skills**：从各 agent 目录自动识别并按标题去重的 skill 列表，支持搜索、多选，右侧显示详情和副本信息。
- **Agents**：agent 列表与多选，右侧可添加自定义 agent（名称 + skills 目录路径）。
- **同步（Sync）**：同步矩阵（选中的 skill x agent），冲突策略选择（prompt / backupOverwrite / skip / rename），执行同步并显示结果。
- **设置（Settings）**：主仓库路径设置、应用数据/备份/导入缓存路径展示、关于信息。

UI 特性：

- 支持拖放导入（文件夹和 .zip 压缩包）。
- CJK 字体自动加载（微软雅黑、黑体、宋体）。
- 响应式布局，窄屏自动堆叠为上下结构。

主要流程：

1. 启动时加载 repository、agents、agent skills 分组。
2. 通过 command bar 搜索、导入、刷新、跳转同步页。
3. Skills/Agents 页面选择项目后，进入同步页执行。
4. 同步调用 `sync_grouped_skill()`，自动选择最佳版本复制到目标 agent。

## 备选 Web UI（Tauri + React）

`src/` 目录保留了基于 Tauri 2 + React 的 Web UI，通过 `npm run tauri dev` 启动。

- `src/main.tsx` 挂载 React 应用。
- `src/App.tsx` 是主界面组件，三栏布局：skills 列表、agents 列表、同步/冲突策略与操作结果。
- `src/api.ts` 封装 Tauri `invoke`，命令名与 Rust command 一一对应。
- `src/types.ts` 定义前端 TypeScript 类型，字段使用 camelCase，与 Rust serde 输出保持一致。
- `src/styles.css` 定义全局布局、三栏工作区、列表卡片、同步矩阵、结果列表和响应式布局。

## Tauri API

`src-tauri/src/lib.rs` 注册这些 commands（供 Web UI 使用）：

- `set_repository(path)`：校验非空路径，创建目录，保存主 skills 仓库路径。
- `get_repository()`：读取主 skills 仓库路径。
- `scan_skills()`：扫描主仓库内的 skill manifests。
- `import_skill_upload(file_name, files)`：上传导入 skill，支持 zip 和文件夹。
- `detect_agents()`：检测内置 Codex/Claude skills 目录。
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

## 后端模块

- `main.rs`：应用入口，调用 `run_native()` 启动原生 egui 桌面版。
- `lib.rs`：模块导出，提供 `run()`（Tauri）和 `run_native()`（egui）两个入口。
- `service.rs`：`AppService` 业务逻辑层，封装 store + adapter，提供所有业务操作（set_repository、scan_skills、import_folder/zip/upload、scan_agent_skills、sync_grouped_skill、install_skills、uninstall_skill、rollback_last 等）。Tauri commands 和 native UI 共用此层。
- `commands.rs`：Tauri command 编排层，每个 command 委托给 `AppService`。仅 Web UI 使用。
- `native_app.rs`：原生 egui 桌面 UI（~1400 行），包含 NativeSkillsApp、五个视图、自定义标题栏、拖放导入、CJK 字体加载、主题配色。
- `adapter.rs`：agent 适配层。当前实现是 `DirectoryAdapter`，适用于 Codex、Claude 和 custom。
- `store.rs`：SQLite 持久化层，包含 migrations、settings、agents、installs、operations、backup root 和 import root。
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
  "supportedAgents": ["codex", "claude", "custom"],
  "entry": "SKILL.md",
  "files": ["SKILL.md"]
}
```

必填字段：

- `id`
- `name`
- `version`
- `supportedAgents`
- `files`

兼容性规则：

- `supportedAgents` 可包含具体 agent 类型：`codex`、`claude`、`custom`。
- `supportedAgents` 包含 `*` 时表示兼容所有 agent 类型。
- 安装目标目录固定为 `{agent.skillsPath}/{skill.id}`。

## Agent 检测

内置检测逻辑位于 `DirectoryAdapter::detect()`：

- Codex：`%USERPROFILE%\.codex\skills`
- Claude：`%APPDATA%\Claude\skills`
- Claude fallback：`%USERPROFILE%\.claude\skills`

只有路径已存在时才会被自动检测到。自定义 agent 通过前端保存，后端会创建其 `skillsPath`。

## 安装状态

状态由 `DirectoryAdapter::diff()` 计算：

- `missing`：不兼容、目标目录不存在，或没有安装。
- `installed`：目标目录 fingerprint 与源 skill fingerprint 一致。
- `stale`：目标 fingerprint 等于上次记录的 fingerprint，但源 skill 已变化。
- `conflict`：目标目录被应用外修改，fingerprint 与源和上次记录都不一致。

fingerprint 由 `hash_dir()` 对目录内所有文件的相对路径和内容计算 SHA-256。文件列表先排序，因此稳定性依赖相对路径和文件内容。

## 冲突策略

`ConflictPolicy` 支持：

- `prompt`：遇到已存在目标目录时返回错误，要求用户选择显式策略。
- `backupOverwrite`：先复制现有目标目录到 backup root，再删除目标并复制源 skill。
- `skip`：跳过已有目标目录。
- `rename`：把新 skill 安装到 `{skill.id}-{yyyyMMddHHmmss}`，不覆盖原目录。

注意：当前 `install()` 只要目标存在就按冲突策略处理，不会单独区分目标是否已经是 current。前端在 `prompt` 模式下只预检查 `conflict` 状态，但后端仍会拒绝任何已有目标目录。

## 数据库表

`store.rs` 的 migration 创建：

- `settings(key, value)`：保存 repository 路径。
- `agents(id, name, agent_type, skills_path, adapter_config)`：保存手动 agent。
- `installs(agent_id, skill_id, fingerprint, target_path, installed_at)`：记录当前安装 fingerprint。
- `operations(id, agent_id, skill_id, action, target_path, backup_path, created_at)`：记录安装、更新、卸载等操作历史。

`rollback_last()` 依赖 `operations` 中最近一条非空 `backup_path`。

## 数据模型

`models.rs` 定义的主要类型：

- `SkillManifest`：skill 清单（id、name、version、description、tags、supportedAgents、entry、files）。
- `SkillSummary`：扫描结果，包含 manifest、source_path、fingerprint、manifest_path。
- `AgentType` 枚举：Codex / Claude / Custom。
- `AgentProfile`：agent 配置（id、name、agent_type、skills_path、adapter_config）。
- `AgentSkillCopy`：agent 目录中的单个 skill 副本（agent_id、skill_path、title、version、fingerprint、updated_at）。
- `GroupedSkill`：按标题聚合的 skill 分组（title、best_copy、copies、installed_agent_ids、missing_agent_ids）。
- `ConflictPolicy` 枚举：Prompt / BackupOverwrite / Skip / Rename。
- `InstallStatus` 枚举：Installed / Stale / Conflict / Missing。
- `InstallState`：单个 skill/agent 组合的安装状态。
- `InstallResult`：安装/同步操作结果。
- `SyncCandidate`：预览同步时的候选 skill 及其状态。
- `ImportSkillFile`：上传导入的文件（relative_path + bytes）。
- `ImportSkillResult`：导入结果（imported、skipped、message）。

## 当前注意事项

- 原生 UI（egui）是主入口，`main.rs` 直接调用 `run_native()`。Tauri + React Web UI 作为备选保留。
- `src/App.tsx` 中多处中文 UI 文案显示为乱码，并且部分字符串/JSX 看起来可能已经损坏。修改 Web UI 前应先运行 `npm run build` 或至少 `npx tsc --noEmit` 验证当前编译状态。
- 原生 UI 和 Web UI 均没有删除 agent 的入口，`remove_agent` API 已封装但未暴露在 UI 中。
- `previewSync` API 已封装，但原生 UI 和 Web UI 当前都没有直接使用。
- `manifest.files` 目前只做非空校验；实际安装使用整个 skill 目录复制，不按 `files` 白名单过滤。
- `rename` 策略会把 skill 安装到带时间戳的新目录，但记录的 `skill_id` 仍是原 manifest id。后续卸载逻辑使用 `{skillsPath}/{skill_id}`，因此对 renamed 安装的卸载/回滚语义需要特别小心。
- `rollback_last()` 恢复文件系统但不更新 `installs` 表中的 fingerprint。
- `list_install_state()` 在 repository 未配置或扫描失败时会吞掉 skills 错误并返回空状态；`scan_skills()` 本身仍会返回错误。
- Tauri capability 当前只有 `core:default`。commands 是本应用自定义 invoke，不依赖额外 shell/fs 插件权限。
- `is_stacked()` 在 `native_app.rs` 中始终返回 `true`，当前布局固定为上下堆叠模式。

## 开发建议

- 后端改动后优先运行 `npm run test:rust`。
- 原生 UI 改动后运行 `npm run native:dev` 查看效果。
- Web UI 或类型改动后运行 `npm run build`。
- 涉及业务逻辑时，修改 `service.rs`，Tauri commands 和 native UI 共用同一层。
- 涉及 Tauri command 参数时，同步检查 `src/api.ts`、`src/types.ts`、`src-tauri/src/models.rs` 和 command 函数签名。
- 涉及安装/卸载/回滚时，优先增加 Rust 单元测试，使用 `tempfile` 隔离源目录、目标目录和备份目录。
- 不要直接修改用户真实 `~/.codex/skills` 或 Claude skills 目录做测试；使用 custom agent 指向临时目录更安全。
