# Skills Manager

[中文文档](README.md)

Skills Manager is a local Windows desktop tool for managing Agent Skills and MCP server configuration across multiple AI clients. It scans local agent directories, compares skill and MCP coverage, installs skills from local files or skills repositories, and helps sync configuration without manually copying folders or editing each config file one by one.

<p align="center">
  <img src="assets/preview-overview.png" alt="Skills Manager overview preview" width="900" />
</p>

## Features

- **Skills coverage:** scan local skill directories, group matching skills by title, compare installed and missing agents, choose a source copy, narrow sync targets by Agent tag, sync to selected agents, uninstall from selected agents, mark skills that do not need full coverage, and read `SKILL.md` or README content in the detail dialog.
- **Agent management:** detect and manage Codex, Claude, Claude Code, Claude Desktop Cowork, Cursor, Trae, OpenCode, Cherry Studio, and custom skill directories. The agent preview shows installed and missing skills, can add selected missing skills, can delete selected installed skills from that agent, and can repair Claude Desktop Cowork manifests when needed. Agents can have custom user tags, and those tags appear in the list, preview, editor, and sync-target picker for filtering. Supports custom agents configuration file path.
- **Skill import:** import a skill folder or `.zip` archive and choose target agents with conflict handling.
- **Skills repository:** browse built-in ClawHub, Claude, and Codex sources; search, sort, filter, refresh cached sources, use safety-mode filtering, add custom Git sources, and install skills to selected agents.
- **MCP management:** scan, add, update, enable or disable, sync, and remove MCP servers for Codex, Claude Code, OpenCode, and Trae. Supported transports are `stdio`, `http`, and `sse`. Supports custom MCP configuration file path.
- **Multi-device Skills sync:** sync Universal Hub skills across machines via an S3-compatible gateway or a local shared folder. Supports client-side encryption, conflict resolution (keep local / take remote / keep both with remote rename), on-demand sync and background polling, connection test, and GC. Install can target the hub (participates in sync) or local agents only.
- **No-tag filtering:** filter skills and agents lists by "no tag" to quickly locate uncategorized items.
- **Independent view scrolling:** Skills, MCP, and Agents view lists scroll independently, the page no longer scrolls as a whole.
- **Dual Styles & Theme Switching:** provides "Clean Minimal (Modern Blue)" and "Classic Warm (Amber Gold)" visual styles, supporting light mode, dark mode, and system preference with local persistence.
- **Streamlined UI:** removed title bar icon and text, removed bottom status bar, theme switcher moved to the Settings view for a cleaner interface with more content area.

## Tech Stack

- Frontend: React, TypeScript, Vite, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Data handling: local files and local skills repository caches/indexes; sync transport for local directories and S3-compatible endpoints

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

### Skills sync (Settings)

Configure under **Settings → Skills Sync**:

| Field | Notes |
|-------|--------|
| Endpoint | S3: `http://host:port`; local folder for single-machine tests: `local://D:/path/to/bucket` |
| Bucket | Required for S3; optional for local folders (subdirectory under the endpoint path) |
| Access Key / Secret | S3 only; secret key and encryption password live in the OS keyring, not in `state.json` |
| Encryption password | Must match on every device when encryption is enabled |
| Auto sync | Enables background polling and hub directory watching |

Single-machine verification (no second PC or real S3 required):

```powershell
# Sync-related tests
cargo test --manifest-path src-tauri/Cargo.toml sync_

# Dual-device demo: A publishes → B pulls → conflict → resolve
cargo run --manifest-path src-tauri/Cargo.toml --bin sync_local_demo
```

When running a second desktop instance on the same machine, give it its own data directory (otherwise both processes fight over one `state.json`):

```powershell
$env:SKILLS_MANAGER_DATA_DIR="D:\tmp\skills-manager-device-b"
npm run desktop:dev
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

The portable build outputs `src-tauri\target\release\skill-sync-manager.exe`. The `release/` folder's `SkillsManager-v0.4.1-windows-portable.zip` is the release artifact, suitable for uploading to GitHub Releases; the root-level `SkillsManager.exe` is useful for quick local verification.
