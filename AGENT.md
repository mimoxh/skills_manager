# AGENT.md

本项目是一个 Tauri 2 + React + TypeScript + Rust 桌面应用，用来集中管理本地 skills 仓库，并把选中的 skill 同步安装到多个 agent 的 skills 目录中。当前包名为 `skill-sync-manager`，产品名为 `Skills Manager`。

## 项目定位

- 前端提供一个三栏管理界面：skills 列表、agents 列表、同步/冲突策略与操作结果。
- 后端通过 Tauri commands 扫描本地 skills 仓库、检测 Codex/Claude skills 目录、保存自定义 agent、安装/卸载/回滚 skill。
- 本地状态用 SQLite 保存，包括主仓库路径、手动添加的 agents、已安装 fingerprint、操作历史和备份位置。
- 同步方式是目录级复制：每个 skill 以 manifest 的 `id` 作为目标子目录名安装到 agent 的 `skillsPath` 下。

## 技术栈

- 前端：React 18、TypeScript、Vite、lucide-react。
- 桌面壳：Tauri 2。
- 后端：Rust 2021、rusqlite、serde/serde_json/serde_yaml、walkdir、sha2、chrono、dirs、thiserror。
- 数据库：应用本地数据目录下的 `skill-sync-manager/state.db`。
- 备份：应用本地数据目录下的 `skill-sync-manager/backups`。

## 常用命令

在项目根目录运行：

```powershell
npm install
npm run dev
npm run build
npm run tauri dev
npm run test:rust
```

命令含义：

- `npm run dev`：启动 Vite，固定端口 `1420`，供 Tauri devUrl 使用。
- `npm run build`：先运行 `tsc`，再运行 `vite build`。
- `npm run tauri dev`：启动完整桌面应用，Tauri 会先执行 `npm run dev`。
- `npm run test:rust`：执行 `cargo test --manifest-path src-tauri/Cargo.toml`。

## 目录结构

```text
.
├── index.html
├── package.json
├── tsconfig.json
├── src/
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
        ├── main.rs
        ├── lib.rs
        ├── commands.rs
        ├── adapter.rs
        ├── store.rs
        ├── manifest.rs
        ├── hash.rs
        ├── models.rs
        └── error.rs
```

## 前端概览

- `src/main.tsx` 挂载 React 应用。
- `src/App.tsx` 是当前唯一主界面组件，管理 repository、skills、agents、install states、选中项、冲突策略、操作结果和 busy 状态。
- `src/api.ts` 封装 Tauri `invoke`，命令名与 Rust command 一一对应。
- `src/types.ts` 定义前端 TypeScript 类型，字段使用 camelCase，与 Rust serde 输出保持一致。
- `src/styles.css` 定义全局布局、三栏工作区、列表卡片、同步矩阵、结果列表和响应式布局。

前端主要流程：

1. 首次加载调用 `refreshAll()`。
2. 并行读取 repository、扫描 skills、读取保存的 agents、检测内置 agents、读取安装状态。
3. 保存 repository 后会创建目录、写入设置、重新扫描。
4. 添加 custom agent 后会保存 agent profile，并将其加入已选 agent。
5. 安装时按选中的 skill 和 agent 组合调用 `install_skills`。
6. 对每条安装结果提供 rollback 和 uninstall 操作。

## Tauri API

`src-tauri/src/lib.rs` 注册这些 commands：

- `set_repository(path)`：校验非空路径，创建目录，保存主 skills 仓库路径。
- `get_repository()`：读取主 skills 仓库路径。
- `scan_skills()`：扫描主仓库内的 skill manifests。
- `detect_agents()`：检测内置 Codex/Claude skills 目录。
- `list_agents()`：读取手动保存的 agents。
- `add_agent(profile)`：校验并保存 agent profile。
- `remove_agent(agent_id)`：删除 agent 与相关安装记录。
- `list_install_state()`：对所有 skill/agent 组合计算安装状态。
- `preview_sync(agent_id)`：返回指定 agent 兼容的 skill 与状态。
- `install_skills(skill_ids, agent_ids, conflict_policy)`：批量安装或更新。
- `uninstall_skill(skill_id, agent_id)`：卸载并备份目标目录。
- `rollback_last(agent_id, skill_id)`：用最近一次备份恢复目标目录。

## 后端模块

- `commands.rs`：Tauri command 编排层，负责加载仓库/agents、调用 adapter、记录 store。
- `adapter.rs`：agent 适配层。当前实现是 `DirectoryAdapter`，适用于 Codex、Claude 和 custom。
- `store.rs`：SQLite 持久化层，包含 migrations、settings、agents、installs、operations 和 backup root。
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

## 当前注意事项

- `src/App.tsx` 中多处中文 UI 文案显示为乱码，并且部分字符串/JSX 看起来可能已经损坏。修改前端前应先运行 `npm run build` 或至少 `npx tsc --noEmit` 验证当前编译状态。
- 前端 `removeAgent` API 已封装，但 UI 中目前没有删除 agent 的入口。
- `previewSync` API 已封装，但主界面当前没有直接使用。
- `manifest.files` 目前只做非空校验；实际安装使用整个 skill 目录复制，不按 `files` 白名单过滤。
- `rename` 策略会把 skill 安装到带时间戳的新目录，但记录的 `skill_id` 仍是原 manifest id。后续卸载逻辑使用 `{skillsPath}/{skill_id}`，因此对 renamed 安装的卸载/回滚语义需要特别小心。
- `rollback_last()` 恢复文件系统但不更新 `installs` 表中的 fingerprint。
- `list_install_state()` 在 repository 未配置或扫描失败时会吞掉 skills 错误并返回空状态；`scan_skills()` 本身仍会返回错误。
- Tauri capability 当前只有 `core:default`。commands 是本应用自定义 invoke，不依赖额外 shell/fs 插件权限。

## 开发建议

- 后端改动后优先运行 `npm run test:rust`。
- 前端或类型改动后运行 `npm run build`。
- 涉及 Tauri command 参数时，同步检查 `src/api.ts`、`src/types.ts`、`src-tauri/src/models.rs` 和 command 函数签名。
- 涉及安装/卸载/回滚时，优先增加 Rust 单元测试，使用 `tempfile` 隔离源目录、目标目录和备份目录。
- 不要直接修改用户真实 `~/.codex/skills` 或 Claude skills 目录做测试；使用 custom agent 指向临时目录更安全。
