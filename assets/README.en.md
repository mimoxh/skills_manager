# Skills Manager

[中文文档](README.md)

Skills Manager is a local Windows desktop tool for managing Agent Skills and MCP server configuration across multiple AI clients. It scans local agent directories, compares skill and MCP coverage, installs skills from local files or skills repositories, and helps sync configuration without manually copying folders or editing each config file one by one.

<p align="center">
  <img src="assets/preview-overview.png" alt="Skills Manager overview preview" width="900" />
</p>

## Features

- **Skills coverage:** scan local skill directories, group matching skills by title, compare installed and missing agents, choose a source copy, narrow sync targets by Agent tag, sync to selected agents, uninstall from selected agents, mark skills that do not need full coverage, and read `SKILL.md` or README content in the detail dialog.
- **Agent management:** detect and manage Codex, Claude, Claude Code, Claude Desktop Cowork, Cursor, Trae, OpenCode, and custom skill directories. The agent preview shows installed and missing skills, can add selected missing skills, can delete selected installed skills from that agent, and can repair Claude Desktop Cowork manifests when needed. Agents can have custom user tags, and those tags appear in the list, preview, editor, and sync-target picker for filtering. Supports custom agents configuration file path.
- **Skill import:** import a skill folder or `.zip` archive and choose target agents with conflict handling.
- **Skills repository:** browse built-in ClawHub, Claude, and Codex sources; search, sort, filter, refresh cached sources, use safety-mode filtering, add custom Git sources, and install skills to selected agents.
- **MCP management:** scan, add, update, enable or disable, sync, and remove MCP servers for Codex, Claude Code, OpenCode, and Trae. Supported transports are `stdio`, `http`, and `sse`. Supports custom MCP configuration file path.
- **No-tag filtering:** filter skills and agents lists by "no tag" to quickly locate uncategorized items.
- **Independent view scrolling:** Skills, MCP, and Agents view lists scroll independently, the page no longer scrolls as a whole.
- **Theme switching:** choose light mode, dark mode, or follow the system theme. The app remembers the local theme preference on each machine.
- **Streamlined UI:** removed title bar icon and text, removed bottom status bar, sidebar theme switcher uses icon mode for a cleaner interface with more content area.

## Tech Stack

- Frontend: React, TypeScript, Vite, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Data handling: local files and local skills repository caches/indexes

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
Compress-Archive -LiteralPath "dist-native\Skills Manager" -DestinationPath "SkillsManager-v0.2.4-windows-portable.zip" -Force
```

The root-level `SkillsManager.exe` and `SkillsManager-v0.2.4-windows-portable.zip` are release artifacts. The zip package is suitable for GitHub Releases, while the standalone exe is useful for quick local verification.
