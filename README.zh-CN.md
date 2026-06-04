# Skills Manager

[English](README.md)

Skills Manager 是一个本地桌面工具，用于管理多个 AI 客户端中的 Agent Skills。它可以扫描本机技能目录、对比不同客户端的覆盖情况，并帮助你把技能同步到指定 Agent，减少手动复制文件夹的重复操作。

## 主要功能

- 支持管理 Codex、Claude、OpenCode、Trae 和 Cherry Studio 的技能目录。
- 自动扫描本地 Agent 技能，并展示不同客户端之间的覆盖情况。
- 将指定技能同步到一个或多个 Agent。
- 支持标记“不需要全覆盖”的技能，便于排除特殊场景。
- 在详情弹窗中读取并渲染 Markdown 技能说明。
- 针对 Cherry Studio，通过本地 SQLite 数据库注册和清理技能记录。
- 支持管理部分 Agent 类型的 MCP 配置。

## 技术栈

- 前端：React、TypeScript、Vite、Tailwind CSS
- 桌面框架：Tauri 2
- 后端：Rust
- 数据处理：本地文件；Cherry Studio 集成会使用 SQLite

## 开发方式

安装依赖：

```powershell
npm install
```

启动 Web 开发服务：

```powershell
npm run dev
```

以开发模式启动桌面应用：

```powershell
npm run desktop:dev
```

构建 Tauri 应用：

```powershell
npm run native:build
```

运行 Rust 测试：

```powershell
npm run test:rust
```

## 便携版发布包

构建 Windows 便携版：

```powershell
.\scripts\build-portable.ps1
```

生成目录：

```text
dist-native\Skills Manager\
```

生成 GitHub Releases 可上传的 zip 包：

```powershell
Compress-Archive -LiteralPath "dist-native\Skills Manager" -DestinationPath "SkillsManager-v0.1.1-windows-portable.zip" -Force
```

zip 发布包会被 Git 忽略，不应提交到仓库；它适合作为 GitHub Release 附件上传。

## 仓库说明

仓库已忽略生成文件和本地缓存，包括 `dist/`、`dist-native/`、`src-tauri/target/`、`.dev-logs/`、`.npm-cache/`、`*.exe` 和 `*.zip`。

如果需要重新发布，请先运行打包脚本，再把生成的 zip 上传到 GitHub Releases。
