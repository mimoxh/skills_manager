$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $Root "src-tauri\Cargo.toml"
$ReleaseDir = Join-Path $Root "dist-native\Skills Manager"
$ExeSource = Join-Path $Root "src-tauri\target\release\skill-sync-manager.exe"
$ExeTarget = Join-Path $ReleaseDir "Skills Manager.exe"

cargo build --release --manifest-path $Manifest

New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null
try {
  Copy-Item -LiteralPath $ExeSource -Destination $ExeTarget -Force
} catch {
  $FallbackExe = Join-Path $ReleaseDir "Skills Manager.updated.exe"
  Copy-Item -LiteralPath $ExeSource -Destination $FallbackExe -Force
  Write-Warning "Skills Manager.exe is currently running, so the updated executable was written to Skills Manager.updated.exe."
}

$Readme = @(
  "Skills Manager",
  "==============",
  "",
  "Windows portable build.",
  "",
  "Run:",
  "1. Double-click Skills Manager.exe.",
  "2. The app creates a default repository and reuses existing local state.",
  "",
  "Notes:",
  "- Tauri 2 desktop app with a Rust backend.",
  "- This portable package does not require a browser or localhost preview service.",
  "- Source development uses React, TypeScript, Vite, and Rust."
)
$Readme | Set-Content -LiteralPath (Join-Path $ReleaseDir "README.txt") -Encoding UTF8

Write-Output "Portable build created at: $ReleaseDir"
