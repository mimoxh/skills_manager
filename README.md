# Skills Manager

Skills Manager is a local desktop tool for managing agent skills across multiple AI clients. It scans skill directories, shows coverage across configured agents, and helps sync or import skills without manually copying folders one by one.

## Features

- Manage skills for Codex, Claude, OpenCode, Trae, and Cherry Studio.
- Scan local agent skill directories and compare coverage.
- Sync skills to selected agents.
- Mark skills that do not need full coverage.
- Read Markdown skill descriptions in the detail view.
- Register Cherry Studio skills through its local SQLite database.
- Manage MCP configuration for supported agent types.

## Tech Stack

- Frontend: React, TypeScript, Vite, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Data handling: local files plus SQLite for Cherry Studio integration

## Development

Install dependencies:

```powershell
npm install
```

Run the web development server:

```powershell
npm run dev
```

Run the desktop app in development mode:

```powershell
npm run desktop:dev
```

Build the Tauri app:

```powershell
npm run native:build
```

Run Rust tests:

```powershell
npm run test:rust
```

## Portable Release

Build a Windows portable package:

```powershell
.\scripts\build-portable.ps1
```

The portable app is written to:

```text
dist-native\Skills Manager\
```

To create a zip package for GitHub Releases:

```powershell
Compress-Archive -LiteralPath "dist-native\Skills Manager" -DestinationPath "SkillsManager-v0.1.0-windows-portable.zip" -Force
```

The zip package is intentionally ignored by Git and should be uploaded as a GitHub Release asset instead of committed to the repository.

## Repository Notes

Generated files and local caches are ignored, including `dist/`, `dist-native/`, `src-tauri/target/`, `.dev-logs/`, `.npm-cache/`, `*.exe`, and `*.zip`.
