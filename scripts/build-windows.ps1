$ErrorActionPreference = "Stop"

Write-Host "[1/4] 检查环境"
node --version
npm --version
rustc --version
cargo --version

Write-Host "[2/4] 安装前端依赖"
npm ci

Write-Host "[3/4] 构建前端与 Windows 安装包"
npm run build
npm run desktop:build

Write-Host "[4/4] 收集安装包"
$releaseDir = Join-Path $PSScriptRoot "..\release"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$bundleDir = Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle"
Get-ChildItem -Path $bundleDir -Recurse -File | Where-Object { $_.Extension -in ".exe", ".msi" } | ForEach-Object {
  Copy-Item $_.FullName -Destination $releaseDir -Force
  Write-Host "已生成: $($_.Name)"
}

Write-Host "构建完成，文件位于: $releaseDir"

