# Skills Manager 项目审查报告

> **审查日期**：2026-07-16
> **项目版本**：v0.2.4（package.json / tauri.conf.json / Cargo.toml 三处一致）
> **审查范围**：项目结构与文档 / Rust 后端（src-tauri/src） / React 前端（src）
> **性质**：只读审查报告，未改动任何代码。按优先级从高到低排列。

---

## 一、版本与构建一致性 ✅ 无问题

- `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处版本均为 `0.2.4`。
- 32 个 Tauri command 与 `lib.rs` 的 `generate_handler!` 注册一一对应，无孤儿/缺失 command。
- 前端 `api.ts` invoke 封装、`types.ts` camelCase 类型与 Rust `models.rs` serde 配置总体对应良好，`any` 类型零使用，无 TODO/FIXME 标记。

---

## 二、文档失同步 — 严重程度：中

### 2.1 AGENT.md 视图数量陈旧

- `AGENT.md:7` 写"三个视图：概览、Skills、Agents"；`:108` 写"四视图（含 MCP）"；`:251` 再次写"三个视图"。
- 实际 `App.tsx:17` 为 **5 视图**：`overview | skills | catalog | agents | mcp`，`CatalogView.tsx`（24 KB）功能完整。
- **建议**：统一改为 5 视图并补充 Catalog 说明。

### 2.2 AGENT.md 支持的 agent 列表与代码不符

- `AGENT.md:155`、`:189-209`、`:228` 仍列 Windsurf、Aider，但 `models.rs:81-100` 的 `AgentType` 枚举**无此两项**。
- 代码实有 9 变体：Codex、Claude、ClaudeCode、**ClaudeCowork**、Cursor、**Trae**、Custom、CherryStudio、OpenCode。
- 文档漏列 ClaudeCowork，保留已移除的 Windsurf/Aider。
- **建议**：以 `models.rs` 枚举为准重写，并更新 agent 检测路径表。

### 2.3 AGENT.md 目录树遗漏文件

未列出：`CatalogView.tsx`、`ImportAgentDialog.tsx`、`SkillInstallDialog.tsx`、`UserTagEditor.tsx`、`Toast.tsx`、`CommandBar.tsx` 等实际存在文件。

### 2.4 README.md 与 AGENT.md agent 列表不一致

README 列 8 类（与代码一致），AGENT.md 枚举段漏 ClaudeCowork。应统一到代码口径。

### 2.5 RELEASE.md 版本缺失

跳过 0.2.0、0.2.2，但 `release/` 目录存在 `SkillsManager-v0.2.2-windows-portable.zip`。建议补齐或说明跳号原因。

---

## 三、潜在 Bug / 语义问题 — 严重程度：中

### 3.1 🔴 MCP `ConflictPolicy::Rename` 实为覆盖

**文件**：`src-tauri/src/mcp_service.rs:195-213`、`:422-439`

```rust
ConflictPolicy::Rename => {
    // 不适用，直接覆盖
    if let Err(e) = adapter.update(agent, &config.name, config) { ... }
}
```

注释写"不适用，直接覆盖"，`Rename` 分支与 `BackupOverwrite` 行为相同。而 skills 域（`service.rs:484-489`）正确追加时间戳实现真正重命名。

**影响**：用户在 MCP 同步选"重命名"时实际覆盖既有配置，与 skill 域语义不一致。

**建议**：实现真正的重命名（追加时间戳），或前端禁用该策略并文案说明。

### 3.2 🔴 `uuid_v4` 非标准 UUID

**文件**：`src-tauri/src/cherry_db.rs:184`

用纳秒时间戳格式化为 32 位 hex，非 RFC 4122 UUIDv4。同纳秒并发会产生相同 ID，可能导致 Cherry Studio `skills` 表 INSERT 冲突。

**建议**：换用 `uuid` crate 生成标准 UUID。

### 3.3 `hash_dir` 的 `strip_prefix` 回退静默改变哈希

**文件**：`src-tauri/src/hash.rs:16`

```rust
.strip_prefix(path).unwrap_or(&file)
```

失败时回退到绝对路径，静默改变后续比对用哈希。正常路径下不会触发，但建议返回 `AppError` 而非回退。

### 3.4 `get_initial_data` 静默吞错

**文件**：`src-tauri/src/service.rs:111-115`

对 agents/skills/no_full_coverage 列表用 `unwrap_or_default()`，失败时前端拿到空数据且无错误提示。建议至少记录日志或向前端返回带 err 标识。

### 3.5 `start_catalog_refresh` 的 `mode` 参数被忽略

**文件**：`src-tauri/src/service.rs:268`（`_mode: Option<String>`）

前端传入但从未使用。建议实现或从命令签名移除。

### 3.6 MCP 适配器 `add()` 中 `unwrap()` 链

**文件**：`mcp_codex.rs:222+`、`mcp_claude.rs:221+`、`mcp_opencode.rs:289+`、`mcp_trae.rs:196+`

刚插入 key 后 `.as_table_mut().unwrap()`，逻辑上安全；但配置文件损坏时会 panic。建议传播 `AppError`。

---

## 四、Dead Code / 未用文件 — 严重程度：低（清理项）

### 4.1 Cloud-sync 冲突副本（6 个文件，已 gitignore）

| 文件 | 大小 |
|------|------|
| `src-tauri/src/service(MimoX's Desktop的冲突副本1_2026-05-31 10-12-41).rs` | ~42 KB |
| `src-tauri/src/service(MimoX's Desktop的冲突副本2_2026-05-31 11-26-33).rs` | ~43 KB |
| `src-tauri/src/service(Mimox_Win的冲突副本3_2026-06-11 16-21-41).rs` | ~68 KB |
| `src-tauri/src/models(MimoX's Desktop的冲突副本1_2026-05-31 11-52-24).rs` | ~6 KB |
| `src/api(MimoX's Desktop的冲突副本1_2026-05-31 11-26-34).ts` | — |
| `src/hooks/useAppState(MimoX's Desktop的冲突副本1_2026-05-31 11-26-35).ts` | — |

前端冲突副本由 `tsconfig.json:20` 的 `exclude: ["src/**/*冲突*"]` 规避编译，但应直接删除。

### 4.2 未引用的 Dead 文件 / 组件

- **`src/styles.css`**（808 行）：从未 import，全部 dead CSS。应用仅用 `index.css`。
- **`src/components/layout/CommandBar.tsx`**：已确认仅自引用，无任何 import。
- **`src/components/ui/`** 的 `badge/button/card/input/scroll-area/separator`：shadcn 脚手架产物，视图直接用 CSS class，从未 import。`Toast.tsx` 仍在用，保留。

### 4.3 未用 Dead API / Dead State

- **`api.ts`**：`detectAgents`、`listAgents`、`rollbackLast` 函数定义但无前端调用。
- **`useAppState.ts`**：`message`/`setMessage` 在多处被 set 但从不渲染（已被 `Toast` 取代），为 vestigial 状态。

### 4.4 Debug 残留

- `useAppState.ts:222` `console.warn(...)` 是前端唯一 console 调用。
- 根目录 `gcm-diagnose.log`（4.9 KB）属陈旧诊断产物。
- `preview.html`（37 KB，5 月 28 日）为早期 mockup，可删除。

### 4.5 `service.rs` 过大

~2900 行 / 106 KB，含全部业务逻辑与单测。建议后续按域拆分（skills / catalog / cherry / 测试）以降维护成本。属结构性优化，非必须。

### 4.6 空目录 `src-tauri/src/bin/`

无文件，疑为遗留脚手架。

---

## 五、安全配置 — 严重程度：低

### 5.1 CSP 关闭

**文件**：`src-tauri/tauri.conf.json:27`

```json
"csp": null
```

WebView 无内容安全策略限制。skill 描述经 ReactMarkdown 渲染，理论上存在本地 XSS 面。建议设置限制性 CSP（至少 `default-src 'self'`，远程图片按需放开）。

---

## 六、工程配置缺口 — 严重程度：低

### 6.1 无 ESLint 配置

`CatalogView.tsx:131` 有 `// eslint-disable-next-line` 暗示曾用 ESLint，但项目无 `.eslintrc` / `eslint.config.js`，规则未在构建/CI 中强制。

### 6.2 无前端测试框架

Rust 测约 73 个（11 个文件），前端零测试。`package.json` 无前端 lint/test script。

### 6.3 `.dev-logs/` 目录

约 20 个开发期日志文件，需确认是否应加入 `.gitignore`。

---

## 七、UI 文案语言一致性 — 严重程度：低

CLAUDE.md 要求"指标卡片、badge 等 UI 文案使用中文"。

当前混用程度较低，英文多为生态专有名词（Skills / Agents / MCP），大体可接受。仅 `OverviewView.tsx:21-23` 统计卡标题 `Skills` / `Agents` 偏英，可改为"Skills 数量" / "Agents 数量"以求一致。非违规，属风格统一建议。

---

## 建议修订优先级汇总

| 优先级 | 类别 | 编号 |
|--------|------|------|
| **高** | 语义 Bug | 3.1 MCP Rename 实为覆盖；3.2 非标 uuid_v4 |
| **中** | 文档同步 | 2.1–2.5 AGENT.md 视图/agent/目录树、RELEASE.md 跳号 |
| **中** | 错误处理 | 3.3–3.6 静默吞错 / unwrap panic / 忽略参数 |
| **低** | 清理 | 4.1–4.6 冲突副本与 dead code 删除 |
| **低** | 安全/工程 | 5.1 CSP；6.1–6.3 ESLint / 前端测试 / .dev-logs |

---

*本报告基于 Explore agents 并行探查并人工核实关键论断生成。所有文件路径与行号均引用项目 v0.2.4 版本快照。*
