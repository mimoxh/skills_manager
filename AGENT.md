# AGENT.md

本项目是一个桌面应用，用来集中管理本地 skills 仓库，并把选中的 skill 同步安装到多个 agent 的 skills 目录中。包名为 `skill-sync-manager`，产品名为 `Skills Manager`。UI 为 Tauri 2 + React + Tailwind CSS 的 WebView 桌面界面。

## 项目定位

- 主 UI 为 Tauri + React 桌面应用，默认首屏为 `Skills` 控制台，提供侧边栏导航和三个视图：概览、Skills、Agents。
- 后端通过 `AppService` 业务层扫描各 agent skills 目录、保存自定义 agent、安装/卸载/回滚 skill、导入 folder/zip/upload/URL。
- 本地状态用 JSON 文件保存（`state.json`），包括手动添加的 agents、已安装 fingerprint、操作历史和备份位置。
- 同步方式是目录级复制：每个 skill 以 manifest 的 `id` 作为目标子目录名安装到 agent 的 `skillsPath` 下。
- 支持从各 agent 目录自动识别 skills 并按标题去重分组；点击 skill 后通过双栏模态弹窗查看 SKILL.md 说明（Markdown 渲染）并选择目标 agent 和冲突策略进行同步或删除。

## 技术栈

- 前端：React 18、TypeScript、Vite 5、Tailwind CSS 4、lucide-react、react-markdown（通过 Tauri 2 桌面壳）。
- 后端：Rust 2021、serde/serde_json/serde_yaml、walkdir、sha2、chrono、dirs、thiserror、zip、regex。
- 数据库：应用本地数据目录下的 `skill-sync-manager/state.json`（JSON 文件存储）。
- 导入缓存：应用本地数据目录下的 `skill-sync-manager/imports`。

## 常用命令

在项目根目录运行：

```powershell
npm install
npm run desktop:dev      # 启动主 UI（Tauri + React）
npm run native:dev       # 启动主 UI（兼容旧脚本名，等同 tauri dev）
npm run native:build     # 构建 Tauri 桌面应用
npm run dev              # 仅启动 Vite 开发服务器
npm run build            # tsc + vite build
npm run test:rust        # cargo test
```

命令含义：

- `npm run desktop:dev`：启动 Tauri + React 桌面应用，Tauri 会先执行 `npm run dev` 启动 Vite（端口 5173）。
- `npm run native:dev`：兼容旧脚本名，当前同样执行 `tauri dev`，启动主 Tauri + React UI。
- `npm run native:build`：执行 `tauri build`，构建生产版本，生成 exe 到 `src-tauri/target/release/`。
- `npm run dev`：仅启动 Vite 开发服务器，固定端口 `5173`，绑定 `127.0.0.1`。
- `npm run build`：先运行 `tsc`，再运行 `vite build`。
- `npm run test:rust`：执行 `cargo test --manifest-path src-tauri/Cargo.toml`。

## 目录结构

```text
.
├── index.html                    # HTML 模板（骨架屏 + Inter 字体 CDN）
├── package.json
├── tsconfig.json
├── vite.config.ts                # Vite + Tailwind CSS 插件配置
├── src/                          # 主 UI（React + Tailwind CSS）
│   ├── main.tsx                  # React 入口
│   ├── App.tsx                   # 应用根组件（布局组合）
│   ├── index.css                 # Tailwind 入口 + 设计 token + 全局样式
│   ├── api.ts                    # Tauri invoke 封装
│   ├── types.ts                  # TypeScript 类型定义
│   ├── lib/
│   │   └── utils.ts              # cn() 工具函数
│   ├── hooks/
│   │   └── useAppState.ts        # 共享状态和业务逻辑
│   ├── components/
│   │   ├── ui/                   # 基础 UI 组件
│   │   │   ├── button.tsx
│   │   │   ├── input.tsx
│   │   │   ├── card.tsx
│   │   │   ├── badge.tsx
│   │   │   ├── separator.tsx
│   │   │   ├── scroll-area.tsx
│   │   │   └── tooltip.tsx
│   │   ├── layout/               # 布局组件
│   │   │   ├── Titlebar.tsx      # 自定义无边框标题栏
│   │   │   └── Sidebar.tsx       # 侧边栏导航
│   │   └── views/                # 页面视图
│   │       ├── OverviewView.tsx
│   │       ├── SkillsView.tsx
│   │       └── AgentsView.tsx
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs               # 应用入口，调用 run() 启动 Tauri WebView
        ├── lib.rs                 # 模块导出 + Tauri 入口
        ├── commands.rs            # Tauri command 层
        ├── service.rs             # AppService 业务逻辑层
        ├── adapter.rs             # agent 适配层（DirectoryAdapter）
        ├── store.rs               # JSON 文件持久化层
        ├── manifest.rs            # skill manifest 扫描与解析
        ├── hash.rs                # SHA-256 fingerprint + 目录复制
        ├── models.rs              # 数据模型
        └── error.rs               # 统一错误类型
```

## 主 UI 概览（Tauri + React）

`src/App.tsx` 是应用根组件，组合 Titlebar、Sidebar 和页面视图。默认进入 `overview`，所有状态逻辑提取到 `hooks/useAppState.ts`。

三个视图：

- **概览（OverviewView）**：指标卡（Skills 总数、Agents 数、需同步数，均可点击跳转：Skills → Skills 视图、Agents → Agents 视图、需同步 → Skills 视图并筛选"需同步"）、快速操作（导入文件夹/zip）和入口跳转。
- **Skills（SkillsView）**：控制台式首屏，包含导入区域（支持文件夹/zip 两种方式）、指标卡（Skills 总数、完全覆盖、已部分覆盖、需同步，四卡片一行布局，均可点击筛选）、可滚动 skill 列表和状态 badge。导入时弹出 agent 选择对话框（多选+全选+冲突策略），直接写入选中 agent 的 skills 目录。每个 skill 条目显示标题、来源、版本、描述（description）和同步状态，右侧有书签按钮（标记/取消"无需全覆盖"）和删除按钮（点击弹出确认对话框后从所有 agent 删除）。标题栏右侧有刷新按钮。点击某个 skill 会打开双栏模态弹窗：左侧用 Markdown 渲染器展示 SKILL.md 说明（`readme` 字段），右侧勾选目标 agent（默认勾选已安装的 agents）并选择冲突策略。底部按钮栏包含"取消"、"无需全覆盖"（切换标记状态）和"同步"三个按钮。取消选中 agent 后点同步会同时为未选中的 agent 删除该 skill；全部取消选中时按钮变为红色"全部删除"。
- **Agents（AgentsView）**：双栏布局，左侧 agent 列表（标题栏右侧有刷新按钮），右侧详情面板。详情面板头部（agent 信息）和"已安装"/"缺失"标题固定不动，skill 条目在各自区域内独立滚动，每条显示标题和描述。缺失 skills 支持点击多选，选中后底部出现"添加"按钮批量安装到当前 agent。支持自定义 agent 添加；删除 agent 时弹出确认对话框（显示 agent 名称，二次确认后执行删除）。

UI 特性：

- 控制台风格：列表为主、信息密度适中、状态徽标清晰。
- 自定义无边框标题栏（可拖拽、最小化/最大化/关闭），按钮贴右并适配 Windows 11 窗口圆角。
- 侧边栏导航，活跃项有 accent 色条指示。
- 支持拖放导入（文件夹和 .zip 压缩包）。
- 双栏布局使用 `minmax()` 网格，窗口变窄时侧栏可自适应收缩。
- Tailwind CSS 4 + 自定义全局样式（`index.css`）。

## Tauri API

`src-tauri/src/lib.rs` 注册这些 commands：

- `get_initial_data()`：获取初始数据（skills、agents）。
- `import_skill_upload(file_name, files, target_agent_ids, conflict_policy)`：上传导入 skill，支持 zip 和文件夹，直接写入选中 agent 的 skills 目录。
- `detect_agents()`：检测内置 agent skills 目录。
- `list_agents()`：读取手动保存的 agents。
- `add_agent(profile)`：校验并保存 agent profile。
- `remove_agent(agent_id)`：删除 agent 与相关安装记录。
- `scan_agent_skills()`：从各 agent 目录扫描 skills 并按标题去重分组。
- `sync_grouped_skill(title, source_agent_id, target_agent_ids, conflict_policy)`：按分组标题同步 skill 到多个 agent。
- `uninstall_skill(skill_id, agent_id)`：卸载目标目录（不备份）。
- `uninstall_skill_from_agents(skill_id, agent_ids)`：批量卸载，从多个 agent 中删除指定 skill。
- `rollback_last(agent_id, skill_id)`：用最近一次备份恢复目标目录。
- `toggle_no_full_coverage(title)`：切换 skill 的"无需全覆盖"标记状态，返回新的布尔值。标记后该 skill 不再计入"需同步"统计。

## 后端模块

- `main.rs`：应用入口，调用 `run()` 启动 Tauri + React WebView。
- `lib.rs`：模块导出，提供 `run()` 入口。
- `service.rs`：`AppService` 业务逻辑层，封装 store + adapter，提供所有业务操作。
- `commands.rs`：Tauri command 编排层，每个 command 委托给 `AppService`。
- `adapter.rs`：agent 适配层。当前实现是 `DirectoryAdapter`，支持 Codex、Claude、ClaudeCode、Cursor、Windsurf、Aider 和 custom。
- `store.rs`：JSON 文件持久化层，状态存储在 `state.json` 中（无主仓库概念，skill 直接写入 agent 目录）。AppState 包含 agents、installs、discovery_paths、operations、next_operation_id 和 no_full_coverage_titles（HashSet，存储"无需全覆盖"标记）。
- `manifest.rs`：扫描和解析 `skill.json`、`skill.yaml`、`skill.yml`，并做基础 manifest 校验。
- `hash.rs`：计算目录 SHA-256 fingerprint，递归复制目录。
- `models.rs`：Rust 侧数据模型，serde 使用 camelCase。
- `error.rs`：统一 `AppError`，可序列化为 Tauri 前端可接收的字符串。

## Skill Manifest 约定

扫描器会在目录下最多 3 层深度寻找这些文件名：

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
- `GroupedSkill`：按标题聚合的 skill 分组（title、best_copy、copies、installed_agent_ids、missing_agent_ids、description、readme）。`readme` 为 SKILL.md 的正文内容（去除 frontmatter），用于弹窗中的 Markdown 渲染。
- `ConflictPolicy` 枚举：Prompt / BackupOverwrite / Skip / Rename。
- `InstallResult`：安装/同步操作结果。
- `ImportSkillFile` / `ImportSkillResult`：上传导入相关。
- `InitialData`：初始数据（skills、agents、no_full_coverage_titles）。`no_full_coverage_titles` 存储被标记为"无需全覆盖"的 skill 标题列表。

## 冲突策略

`ConflictPolicy` 支持：

- `prompt`：遇到已存在目标目录时返回错误，要求用户选择显式策略。
- `backupOverwrite`：先复制现有目标目录到 backup root，再删除目标并复制源 skill。
- `skip`：跳过已有目标目录。
- `rename`：把新 skill 安装到 `{skill.id}-{yyyyMMddHHmmss}`，不覆盖原目录。

## 当前注意事项

- Tauri + React 是唯一入口，`main.rs` 调用 `run()`。
- `src-tauri/Cargo.toml` 设置了 `default-run = "skill-sync-manager"`。
- 当前 React UI 有三个视图：概览、Skills、Agents。同步入口在 `SkillsView` 的单个 skill 点击模态弹窗中。
- AgentsView 采用双栏布局：左侧 agent 列表，右侧详情面板（显示已安装/缺失 skills）。详情面板采用 flex 纵向布局，card-header 固定顶部，card-body 内部"已安装"/"缺失"标题固定，skill 条目列表各自独立滚动（`overflow-y: auto`）。缺失 skills 支持单击多选，选中后显示"添加"按钮批量同步。
- 同步弹窗默认勾选已安装该 skill 的 agents（`installedAgentIds`），而非未安装的。
- AgentsView 支持删除 agent（点击垃圾桶图标，弹出确认对话框显示 agent 名称，确认后执行删除）。
- `manifest.files` 目前只做非空校验；实际安装使用整个 skill 目录复制，不按 `files` 白名单过滤。
- skill 的 `description` 字段从 `skill.json`/`skill.yaml`/`skill.yml` 的 `description` 字段读取，也支持从 `SKILL.md` frontmatter 的 `description` 字段读取。frontmatter 解析支持 YAML 块标量语法（`|`、`>`、`|-`、`>-`、`|+`、`>+`），可正确解析多行描述。无效描述（仅包含符号/标点，不含字母或汉字）会被过滤。`readme` 字段从 SKILL.md 正文提取（去除 frontmatter），当 skill.json 和 SKILL.md 共存时两者都会读取。两者传递到前端的 `GroupedSkill` 和 `AgentSkillCopy` 中。
- `rename` 策略会把 skill 安装到带时间戳的新目录，但记录的 `skill_id` 仍是原 manifest id。后续卸载逻辑使用 `{skillsPath}/{skill_id}`，因此对 renamed 安装的卸载语义需要特别小心。
- 卸载（`uninstall_skill`）直接删除目标目录，不创建备份。`rollback_last()` 恢复文件系统但不更新 `installs` 中的 fingerprint。
- SkillsView 支持"无需全覆盖"标记：用户可通过列表项的书签按钮或同步弹窗底部的"无需全覆盖"按钮标记 skill。标记后该 skill 不再计入"需同步"统计，而是归入"已部分覆盖"。标记存储在 store 的 `no_full_coverage_titles` 中，持久化到 `state.json`。
- SkillsView 支持批量删除：列表中每个 skill 右侧有删除按钮，点击弹出确认对话框后从所有 agent 删除；弹窗中取消选中 agent 后点同步也会为未选中的 agent 执行删除。
- Tauri capability 当前只有 `core:default`。commands 是本应用自定义 invoke，不依赖额外 shell/fs 插件权限。
- Vite 开发服务器绑定 `127.0.0.1:5173`（因 Windows 防火墙限制，不使用 localhost）。
- `.card` 组件使用 `min-height: 0; flex-shrink: 1` 以支持在 flex 容器中正确收缩和滚动。

## 开发建议

- 后端改动后优先运行 `npm run test:rust`。
- 主 UI 改动后运行 `npm run desktop:dev` 查看效果。
- 前端类型改动后运行 `npx tsc --noEmit` 验证。
- 生产前端改动后运行 `npm run build`。
- 涉及业务逻辑时，修改 `service.rs`。
- 涉及 Tauri command 参数时，同步检查 `src/api.ts`、`src/types.ts`、`src-tauri/src/models.rs` 和 command 函数签名。
- 涉及安装/卸载/回滚时，优先增加 Rust 单元测试，使用 `tempfile` 隔离源目录、目标目录和备份目录。
- 不要直接修改用户真实 agent skills 目录做测试；使用 custom agent 指向临时目录更安全。
- 构建 exe：`npm run native:build`，生成到 `src-tauri/target/release/skill-sync-manager.exe`，可复制到项目根目录作为 `SkillsManager.exe`。打包安装程序需要 `.ico` 图标文件（`tauri.conf.json` 中 `bundle.icon` 需包含 `icons/icon.ico`）。
