# 将本地 release/ 下的 portable zip 上传到 GitHub Release 资产。
# 用法（在项目根目录）:
#   powershell -File scripts/upload-release-asset.ps1
#   powershell -File scripts/upload-release-asset.ps1 -Tag v0.4.1
#   powershell -File scripts/upload-release-asset.ps1 -ZipPath path\to\file.zip
#
# Token 解析顺序: $env:GITHUB_TOKEN > $env:GH_TOKEN > git remote github URL 内嵌凭据。
param(
  [string]$Tag = "",
  [string]$ZipPath = "",
  [string]$Owner = "mimoxh",
  [string]$Repo = "skills_manager"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
try {
  if (-not $Tag) {
    $Version = (Get-Content (Join-Path $Root "package.json") -Raw | ConvertFrom-Json).version
    $Tag = "v$Version"
  }

  if (-not $ZipPath) {
    $Version = $Tag.TrimStart("v")
    $candidate = Join-Path $Root "release\SkillsManager-v${Version}-windows-portable.zip"
    if (-not (Test-Path -LiteralPath $candidate)) {
      $found = Get-ChildItem -Path (Join-Path $Root "release") -Filter "*.zip" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
      if (-not $found) {
        throw "未找到 zip。先运行 scripts\build-portable.ps1，或用 -ZipPath 指定。"
      }
      $ZipPath = $found.FullName
      Write-Warning "使用最近的 zip: $ZipPath"
    } else {
      $ZipPath = $candidate
    }
  }

  if (-not (Test-Path -LiteralPath $ZipPath)) {
    throw "zip 不存在: $ZipPath"
  }

  $Token = $env:GITHUB_TOKEN
  if (-not $Token) { $Token = $env:GH_TOKEN }

  $remote = git remote get-url github 2>$null
  if ($LASTEXITCODE -eq 0 -and $remote) {
    if (-not $Token -and $remote -match '^(?<scheme>https?://)(?<auth>[^@]+)@(?<host>.+)$') {
      $auth = $Matches['auth']
      if ($auth -match ':') {
        $Token = ($auth -split ':', 2)[1]
      } else {
        $Token = $auth
      }
    }
    if ($remote -match 'github\.com[:/](?<owner>[^/]+)/(?<repo>[^/]+?)(?:\.git)?$') {
      $Owner = $Matches['owner']
      $Repo = $Matches['repo']
    }
  }

  if (-not $Token) {
    throw "缺少 token。设置 `$env:GITHUB_TOKEN 或 `$env:GH_TOKEN（需要 repo 写权限）。"
  }

  $Api = "https://api.github.com/repos/$Owner/$Repo"

  Write-Host "Release tag : $Tag"
  Write-Host "API         : $Api"
  Write-Host "Asset       : $ZipPath"

  $headers = @{
    Authorization = "Bearer $Token"
    Accept        = "application/vnd.github+json"
    "User-Agent"  = "skills-manager-release-script"
  }

  $release = Invoke-RestMethod -Uri "$Api/releases/tags/$Tag" -Headers $headers -Method Get
  $releaseId = $release.id
  Write-Host "Release id  : $releaseId ($($release.name))"

  $fileName = Split-Path -Leaf $ZipPath
  $existing = @($release.assets) | Where-Object { $_.name -eq $fileName }
  foreach ($a in $existing) {
    Write-Host "删除同名资产 id=$($a.id) name=$($a.name)"
    Invoke-RestMethod -Uri "$Api/releases/$releaseId/assets/$($a.id)" -Headers $headers -Method Delete | Out-Null
  }

  $escapedName = [uri]::EscapeDataString($fileName)
  $uploadUrl = "https://uploads.github.com/repos/$Owner/$Repo/releases/$releaseId/assets?name=$escapedName"
  Write-Host "上传中..."
  $resp = Invoke-RestMethod -Uri $uploadUrl -Headers $headers -Method Post -InFile $ZipPath -ContentType "application/octet-stream"
  Write-Host "已上传: $($resp.name) size=$($resp.size) url=$($resp.browser_download_url)"
  Write-Output $resp.browser_download_url
}
finally {
  Pop-Location
}
