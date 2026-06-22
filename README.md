# Skills Manager

[English](README.en.md)

Skills Manager 是一个本地 Windows 桌面工具，用于管理多个 AI 客户端中的 Agent Skills 和 MCP server 配置。它会扫描本机 Agent 目录、对比 Skill 与 MCP 覆盖情况、从本地文件或 skills 仓库安装技能，并帮助你同步配置，减少手动复制文件夹或逐个编辑配置文件的重复操作。

<p align="center">
  <img src="assets/preview-overview.png" alt="Skills Manager 概览预览" width="900" />
</p> 

## 主要功能

- **Skills 覆盖管理**：扫描本地技能目录，按标题分组匹配技能，对比已安装和缺失的 Agent，选择来源副本同步到指定 Agent，从指定 Agent 卸载技能，标记"不需要全覆盖"的技能，并在详情弹窗中读取 `SKILL.md` 或 README 内容。
- **Agent 管理**：检测和管理 Codex、Claude、Claude Code、Claude Desktop Cowork、Cursor、Trae、OpenCode 以及自定义技能目录。Agent 预览会展示已安装和缺失的 Skills，可向该 Agent 添加选中的缺失 Skills，也可从该 Agent 删除选中的已安装 Skills，并可在需要时修复 Claude Desktop Cowork 清单。
- **Skill 导入**：导入技能文件夹或 `.zip` 压缩包，并在处理冲突时选择目标 Agent。
- **Skills 仓库目录**：浏览内置 ClawHub、Claude、Codex 来源；支持搜索、排序、筛选、刷新缓存来源、安全模式筛选、添加自定义 Git 仓库源，并把 skills 仓库中的 skill 安装到指定 Agent。
- **MCP 管理**：为 Codex、Claude Code、OpenCode 和 Trae 扫描、新增、更新、启用或禁用、同步、删除 MCP server。支持的传输类型为 `stdio`、`http` 和 `sse`。
- **主题切换**：支持浅色、深色、跟随系统三种主题模式，并在本机记住界面偏好。
- **可用性改进**：使用更宽、更稳定的响应式 Agent 预览弹窗，提升 skills 仓库卡片可读性，并增强文本选择对比度，方便复制 README 文本、路径和配置片段。

## 技术栈

- 前端：React、TypeScript、Vite、Tailwind CSS
- 桌面框架：Tauri 2
- 后端：Rust
- 数据处理：本地文件、本地 skills 仓库缓存/索引

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

## 便携版发布包

构建 Windows 便携版：

```powershell
.\scripts\build-portable.ps1
```

该脚本会运行：

```powershell
npm run native:build -- --no-bundle
```

然后把：

```text
src-tauri\target\release\skill-sync-manager.exe
```

复制为：

```text
dist-native\Skills Manager\Skills Manager.exe
```

并在同一目录写入便携版 `README.txt`。

生成 GitHub Releases 可上传的版本 zip 包：

```powershell
Compress-Archive -LiteralPath "dist-native\Skills Manager" -DestinationPath "SkillsManager-v0.2.3-windows-portable.zip" -Force
```

根目录中的 `SkillsManager.exe` 和 `SkillsManager-v0.2.3-windows-portable.zip` 是发�
