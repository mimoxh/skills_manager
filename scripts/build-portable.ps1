$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$ExeSource = Join-Path $Root "src-tauri\target\release\skill-sync-manager.exe"
$ReleaseRoot = Join-Path $Root "release"
$PackageDir = Join-Path $ReleaseRoot "Skills Manager"
$Version = (Get-Content (Join-Path $Root "package.json") -Raw | ConvertFrom-Json).version
$ZipPath = Join-Path $ReleaseRoot "SkillsManager-v${Version}-windows-portable.zip"
$RootExe = Join-Path $Root "SkillsManager.exe"

Push-Location $Root
try {
  npm run native:build -- --no-bundle
  if ($LASTEXITCODE -ne 0) {
    throw "Tauri build failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

# Copy the exe; if the target is locked (app running), write a .updated.exe sibling and warn.
function Copy-ExeWithFallback([string]$Source, [string]$Target) {
  try {
    Copy-Item -LiteralPath $Source -Destination $Target -Force
  } catch {
    $fallback = [System.IO.Path]::ChangeExtension($Target, ".updated.exe")
    Copy-Item -LiteralPath $Source -Destination $fallback -Force
    Write-Warning "$Target is currently running, so the updated executable was written to $fallback."
  }
}

# Stage the portable package under release\Skills Manager
New-Item -ItemType Directory -Force -Path $PackageDir | Out-Null
Copy-ExeWithFallback $ExeSource (Join-Path $PackageDir "Skills Manager.exe")

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
$Readme | Set-Content -LiteralPath (Join-Path $PackageDir "README.txt") -Encoding UTF8

# Create the versioned zip under release/ and clean up the staging dir.
# Also refresh the root-level SkillsManager.exe for quick local verification.
if (Test-Path $ZipPath) { Remove-Item -LiteralPath $ZipPath -Force }
Compress-Archive -Path $PackageDir -DestinationPath $ZipPath -Force
Remove-Item -LiteralPath $PackageDir -Recurse -Force
Copy-ExeWithFallback $ExeSource $RootExe

Write-Output "Portable zip created at: $ZipPath"
Write-Output "Local exe updated at: $RootExe"
