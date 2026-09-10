---
feature: universal-hub-optim
status: delivered
updated: 2026-02-14
branch: develop
commits: 6c077a6..WIP
---

# Universal Hub 优化（P0–P2）

## Report

**What was built** — 将 Universal Hub 从「可管理目录」升级为「唯一真源 + 扇出」拓扑：`supportsUniversal` 变为用户可改的能力标志；sync/catalog/import 先 `materialize_into_hub` 再 `fanout_from_hub`（原生兼容跳过，禁止 Agent→Agent 软链）；Windows 链接降级 symlink→Junction→copy；冲突备份/删除识别软链；写入并合并 `.skill-lock.json`；Agents 可切换原生兼容，Skills 区分中枢/软链/实体徽章。

**Verification** — `cargo test --lib`：82 passed；`npm run build`：成功。Review 发现的 CRITICAL（WalkDir 不跟软链源导致空复制）与 MAJOR（`exists()` 误判断链）已修复，并补 `sync_from_symlink_source_materializes_real_hub_copy`、`copy_dir_all_follows_symlink_source` 测试。

**Journey log** — 1) 沙箱阻止 `git worktree add`，改在主工作区落地；2) 测试曾扫到真实 `~/.agents`，`detect_agents` 在 cfg(test) 置空；3) Hub `Skip` 语义保持「不覆盖已有中枢内容」，fanout 使用现存中枢目录。

## [S1] Problem

Universal Hub 脚手架已落地（`~/.agents/skills` 作为 Agent、软链优先同步、`.skill-lock.json` 只读解析），但未闭环产品目标：

1. `supports_universal` 仅在 `skills_path` 以 `/.agents/skills` 结尾时为 true，内置 harness（Codex/Claude/Claude Code 等）永远为 false，「中枢已有 → 兼容 Agent 视为覆盖」逻辑空转。
2. 安装拓扑允许 Agent→Agent 软链，卸载来源后目标断链，中枢不是唯一真源。
3. Catalog 安装仍 `copy_dir_all`，与同步路径行为不一致。
4. `resolve_install_conflict` 对软链目标执行 `copy_dir_all`/`remove_dir_all`，备份可能为空或误伤。
5. Windows `symlink_dir` 需开发者模式/管理员；可用 Junction 免特权降级。
6. 本应用不写 `.skill-lock.json`，自装技能在 UI 无来源。
7. `list_agents` 已保存 profile 优先，旧数据无法纠正 `supports_universal`。

## [S2] Design

### [S2.1] 能力模型

- `supports_universal` 语义：该 Agent **是否原生扫描** `~/.agents/skills`（有效能力，非路径后缀自证）。
- 默认关闭；Universal Agent / 路径为中枢时强制 true。
- 用户可在 Agents 视图为非中枢 Agent 勾选「原生兼容中枢」，经 `add_agent`（upsert）持久化。
- 路径判定统一为规范化后以 `/.agents/skills` 结尾（兼容 `\`）。

```text
effective_supports_universal(agent) =
  type == Universal || is_universal_skills_path(skills_path) || profile.supports_universal
```

### [S2.2] 安装拓扑（强制先入中枢再扇出）

```text
源副本（任意 Agent / Catalog / 导入）
        │ materialize（实体复制，源已是中枢则跳过）
        ▼
  ~/.agents/skills/<skill>     ← 唯一真源
        │
        ├─ supports_universal 的 Agent → 跳过私有安装（message: 原生兼容中枢）
        ├─ Cherry Studio / Cowork     → 仍走专用安装（从中枢复制）
        └─ 其它目录型 Agent           → symlink_or_copy_dir(hub_skill → agent/<skill>)
```

- 禁止 Agent→Agent 软链。
- 无中枢时自动创建/检测 Universal Agent（`~/.agents/skills`），必要时 `create_dir_all`。
- `sync_grouped_skill`、`install_catalog_skill`、导入路径共用上述管道。

### [S2.3] 覆盖算法

```text
covered(agent, group):
  group.is_universal && agent.supports_universal  → covered
  agent.id ∈ group.installed_agent_ids            → covered
  否则                                            → missing
```

与现逻辑一致；因 [S2.1] 使 `supports_universal` 可真实为 true，覆盖开始生效。

### [S2.4] 链接与冲突安全

- `symlink_or_copy_dir`：Windows 依次尝试 `symlink_dir` → `mklink /J` Junction → `copy_dir_all`；非 Windows symlink → copy。
- 冲突处理前用 `symlink_metadata` 识别链接：
  - 备份链接时写入 `.symlink-target`（目标路径文本），不 WalkDir 跟随。
  - 删除统一 `remove_dir_or_symlink`。
- 卸载中枢时只删链接节点；已指向中枢的 Agent 软链可能断链，扫描时对 dangling symlink 标 `is_symlink` 并跳过读 manifest 失败路径。

### [S2.5] skill-lock 读写

- 读：整次 scan 加载一次 `~/.agents/.skill-lock.json`。
- 写：materialize 到中枢成功后 upsert 条目（`dir_name` 为 key），保留未知 key；字段含 `source`/`sourceType`/`sourceUrl`/`skillPath`/`installedAt`/`updatedAt`。
- 不强制迁移历史锁文件格式以外字段。

### [S2.6] 合并与持久化

- `list_agents`：saved 优先；若 `is_universal_skills_path(skills_path)` 则强制 `supports_universal = true`。
- `add_agent`：路径为中枢时强制 true；否则尊重传入值。
- 新增可选 command 不必须；前端经现有 `addAgent` upsert。

### [S2.7] UI

- Agents：非中枢 Agent 可切换「原生兼容中枢」；中枢显示「★ 基准中枢」。
- Skills：徽章区分「Universal 中枢 / 软链副本 / 实体副本」；详情展示中枢状态与链接目标。
- 安装对话框：原生兼容 Agent 默认不可选或提示已覆盖（实现为仍可选但标签说明，避免误装）。

## [S3] Out of Scope

- 不实现跨机器中枢同步、ACL、多中枢。
- 不自动探测各 harness 是否原生读 `~/.agents/skills`（由用户标记）。
- 不为 Cherry Studio/Cowork 改造其注册协议。
- 不引入新 crate（junction/winapi）；Junction 经 `cmd /c mklink /J`。
- 不删除用户既有 Agent→Agent 旧软链（仅新安装走新拓扑）。

## Tasks

- [x] T1: 能力模型与中枢路径工具 — acceptance: `is_universal_skills_path`、effective 覆盖、list/add 强制中枢 true；单测通过 (covers: S2.1, S2.6)
- [x] T2: hub-first 安装管道 — acceptance: sync/catalog 先 materialize 到中枢再扇出；原生兼容跳过；无中枢自动创建；禁止 Agent→Agent 软链；单测 (covers: S2.2, S2.3; depends: T1)
- [x] T3: 链接降级与冲突安全 — acceptance: Windows junction 回退；冲突备份/删除识别软链；单测 (covers: S2.4; depends: T2)
- [x] T4: skill-lock 读写与 scan 缓存 — acceptance: 安装到中枢后 lock 出现/更新条目；scan 只加载一次 (covers: S2.5; depends: T2)
- [x] T5: UI 能力开关与副本徽章 — acceptance: Agents 可勾选原生兼容；Skills 显示中枢/软链/实体；TS build 通过 (covers: S2.7; depends: T1)
- [x] T6: 文档同步 AGENT.md — acceptance: 架构段描述 hub-first 与能力模型 (covers: S2; depends: T2)
