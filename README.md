# Skills Manager

[English](README_EN.md)

Skills Manager 是一个本地 Windows 桌面工具，用于管理多个 AI 客户端中的 Agent Skills 和 MCP server 配置。它会扫描本机 Agent 目录、对比 Skill 与 MCP 覆盖情况、从本地文件或 skills 仓库安装技能，并帮助你同步配置，减少手动复制文件夹或逐个编辑配置文件的重复操作。

<p align="center">
  <img src="assets/preview-overview.png" alt="Skills Manager 概览预览" width="900" />
</p> 

## 主要功能

- **Skills 覆盖管理**：扫描本地技能目录，按标题分组匹配技能，对比已安装和缺失的 Agent，选择来源副本同步到指定 Agent，按 Agent 标签缩小同步目标范围，从指定 Agent 卸载技能，标记"不需要全覆盖"的技能，并在详情弹窗中读取 `SKILL.md` 或 README 内容。
- **Agent 管理**：检测和管理 Codex、Claude、Claude Code、Claude Desktop Cowork、Cursor、Trae、OpenCode、Cherry Studio 以及自定义技能目录。Agent 预览会展示已安装和缺失的 Skills，可向该 Agent 添加选中的缺失 Skills，也可从该 Agent 删除选中的已安装 Skills。支持为 Agent 添加自定义标签，并在列表、预览、编辑和同步目标选择中按标签展示或筛选；需要时可修复 Claude Desktop Cowork 清单。支持自定义 agents 配置文件路径。
- **Skill 导入**：导入技能文件夹或 `.zip` 压缩包，并在处理冲突时选择目标 Agent。
- **Skills 仓库目录**：浏览内置 ClawHub、Claude、Codex 来源；支持搜索、排序、筛选、刷新缓存来源、安全模式筛选、添加自定义 Git 仓库源，并把 skills 仓库中的 skill 安装到指定 Agent。
- **MCP 管理**：为 Codex、Claude Code、OpenCode 和 Trae 扫描、新增、更新、启用或禁用、同步、删除 MCP server。支持的传输类型为 `stdio`、`http` 和 `sse`。支持自定义 MCP 配置文件路径。
- **Skills 多设备同步**：通过 S3 兼容网关或本地共享目录，在多台设备间同步 Universal 中枢 skills。支持客户端加密、冲突处理（保留本地 / 采用远端 / 远端改名保留）、立即同步与后台轮询、连接测试与 GC。安装技能时可选择「同步到中枢」或「仅本机」。
- **无标签筛选**：Skills 和 Agents 列表支持筛选"无标签"项目，便于快速定位未分类的技能和 Agent。
- **视图独立滚动**：Skills、MCP、Agents 视图列表独立滚动，页面整体不跟随滚动，多列表场景下操作更流畅。
- **双风格与主题切换**：支持「清爽极简（现代蓝白）」与「经典暖色（琥珀暖金）」两种风格，兼容浅色、深色与跟随系统模式，并在本机持久化偏好。
- **精简界面**：移除标题栏图标和标题文字，移除底部状态栏，主题切换移入设置页，界面更简洁、内容区域更大。

## 技术栈

- 前端：React、TypeScript、Vite、Tailwind CSS
- 桌面框架：Tauri 2
- 后端：Rust
- 数据处理：本地文件、本地 skills 仓库缓存/索引；同步层支持本地目录与 S3 兼容传输

## 开发方式

安装依赖：

```powershell
npm install
```

启动 Vite Web 开发服务：

```powershell
npm run dev
```

以开发模式启动 Tauri 桌面应用：

```powershell
npm run desktop:dev
```

构建 Tauri 应用：

```powershell
npm run native:build
```

`npm run native:build` 当前会运行 `tauri build`。

运行 Rust 测试：

```powershell
npm run test:rust
```

### Skills 同步（设置页）

在设置页「Skills 同步」中配置：

| 字段 | 说明 |
|------|------|
| Endpoint | S3：`http://主机:端口`；单机测试本地目录：`local://D:/path/to/bucket` |
| Bucket | S3 必填；本地目录可留空，或填作 endpoint 下的子目录名 |
| Access Key / Secret | 仅 S3 需要；Secret 与加密口令保存在系统钥匙串，不写入配置文件 |
| 加密口令 | 启用加密时两端必须相同 |
| 自动同步 | 启用后台轮询与中枢目录监听 |

单机验证（无需第二台电脑或真实 S3）：

```powershell
# 同步相关测试
cargo test --manifest-path src-tauri/Cargo.toml sync_

# 双设备演示：A 发布 → B 拉取 → 制造冲突 → 解决
cargo run --manifest-path src-tauri/Cargo.toml --bin sync_local_demo
```

同机双开第二个桌面实例时，为第二个进程设置独立数据目录（否则会争用同一 `state.json`）：

```powershell
$env:SKILLS_MANAGER_DATA_DIR="D:\tmp\skills-manager-device-b"
npm run desktop:dev
```

## 便携版发布包

构建 Windows 便携版：

```powershell
.\scripts\build-portable.ps1
```

该脚本会运行：

```powershell
npm run native:build -- --no-bundle
```

便携版构建产物为 `src-tauri\target\release\skill-sync-manager.exe`。`release/` 目录中的 `SkillsManager-v0.4.1-windows-portable.zip` 是发布产物，适合上传到 GitHub Releases；根目录的 `SkillsManager.exe` 可用于本地快速验证。
