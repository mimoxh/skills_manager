# Skills Manager

[中文文档](README.zh-CN.md)

Skills Manager is a local Windows desktop tool for managing Agent Skills and MCP server configuration across multiple AI clients. It scans local agent directories, compares skill and MCP coverage, installs skills from local files or catalogs, and helps sync configuration without manually copying folders or editing each config file one by one.

## Features

- **Skills coverage:** scan local skill directories, group matching skills by title, compare installed and missing agents, choose a source copy, sync to selected agents, uninstall from selected agents, mark skills that do not need full coverage, and read `SKILL.md` or README content in the detail dialog.
- **Agent management:** detect and manage Codex, Claude, Claude Code, Claude Desktop Cowork, Cursor, Trae, Cherry Studio, OpenCode, and custom skill directories.
- **Skill import:** import a skill folder or `.zip` archive and choose target agents with conflict handling.
- **Catalog:** browse built-in ClawHub, Claude, and Codex catalog sources; search, sort, filter, refresh cached sources, use safety-mode filtering, add custom Git catalog sources, and install catalog skills to selected agents.
- **MCP management:** scan, add, update, enable or disable, sync, and remove MCP servers for Codex, Claude Code, OpenCode, and Trae. Supported transports are `stdio`, `http`, and `sse`.
- **Cherry Studio integration:** register and clean up Cherry Studio skills through its local SQLite database when installing or uninstalling skills.

## Tech Stack

- Frontend: React, TypeScript, Vite, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Data handling: local files, local catalog cache/indexes, and SQLite for Cherry Studio integration

## Development

Install dependencies:

```powershell
npm install
```

Run the Vite web development server:

```powershell
npm run dev
```

Run the Tauri desktop app in development mode:

```powershell
npm run desktop:dev
```

Build the Tauri app:

```powershell
npm run native:build
```

`npm run native:build` currently runs `tauri build`.

Run Rust tests:

```powershell
npm run test:rust
```

## Portable Release

Build a Windows portable package:

```powershell
.\scripts\build-portable.ps1
```

The script runs:

```powershell
npm run native:build -- --no-bundle
```

Then it copies:

```text
src-tauri\target\release\skill-sync-manager.exe
```

to:

```text
dist-native\Skills Manager\Skills Manager.exe
```

and writes a portable-package `README.txt` into the same directory.

To create the versioned zip package for GitHub Releases:

```powershell
Compress-Archive -LiteralPath "dist-native\Skills Manager" -DestinationPath "SkillsManager-v0.1.1-windows-portable.zip" -Force
```

The root-level `SkillsManager.exe` and `SkillsManager-v0.1.1-windows-portable.zip` are release artifacts. The zip package is intentionally ignored by Git and should be uploaded as a GitHub Release asset instead of committed to the repository.

## Repository Notes

Generated files and local caches are ignored, including `dist/`, `dist-native/`, `src-tauri/target/`, `.dev-logs/`, `.npm-cache/`, `*.exe`, and `*.zip`.
