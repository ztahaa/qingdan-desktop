param(
  [string]$InstallDir = "D:\Qingdan"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Stop-WithMessage([string]$Message) {
  Write-Host ""
  Write-Host $Message -ForegroundColor Red
  Write-Host ""
  Read-Host "Press Enter to exit"
  exit 1
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Qingdan - Portable D Drive Deployment" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

if (-not (Test-Path "D:\")) {
  Stop-WithMessage "Drive D was not found. Connect or create drive D, then run this script again."
}

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
  Stop-WithMessage "Node.js is missing. Install it first: winget install OpenJS.NodeJS.LTS"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Stop-WithMessage "Rust is missing. Install it first: winget install Rustlang.Rustup"
}

Write-Host "[1/5] Installing project dependencies..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
  npm ci
  if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }

  Write-Host "[2/5] Building the frontend..." -ForegroundColor Yellow
  npm run build
  if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }

  Write-Host "[3/5] Building the Windows desktop app..." -ForegroundColor Yellow
  npm run tauri -- build --no-bundle
  if ($LASTEXITCODE -ne 0) { throw "Desktop build failed" }
} catch {
  Stop-WithMessage $_.Exception.Message
} finally {
  Pop-Location
}

$BuiltExe = Join-Path $ProjectRoot "src-tauri\target\release\qingdan-desktop.exe"
if (-not (Test-Path $BuiltExe)) {
  Stop-WithMessage "Built program not found: $BuiltExe"
}

Write-Host "[4/5] Copying files to $InstallDir ..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "data") | Out-Null

$TargetExe = Join-Path $InstallDir "Qingdan.exe"
try {
  Copy-Item $BuiltExe $TargetExe -Force
} catch {
  Stop-WithMessage "Cannot update the program. Close Qingdan if it is running, then try again."
}

Set-Content -Path (Join-Path $InstallDir "portable.mode") -Value "Qingdan portable data mode" -Encoding ASCII

Write-Host "[5/5] Creating the desktop shortcut..." -ForegroundColor Yellow
$Desktop = [Environment]::GetFolderPath("Desktop")
$Shell = New-Object -ComObject WScript.Shell
$Shortcut = $Shell.CreateShortcut((Join-Path $Desktop "Qingdan.lnk"))
$Shortcut.TargetPath = $TargetExe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.IconLocation = "$TargetExe,0"
$Shortcut.Description = "Qingdan local todo app"
$Shortcut.Save()

Write-Host ""
Write-Host "Deployment completed successfully." -ForegroundColor Green
Write-Host "Program: $TargetExe"
Write-Host "Data: $(Join-Path $InstallDir 'data')"
Write-Host "Shortcut: $(Join-Path $Desktop 'Qingdan.lnk')"
Write-Host ""

Start-Process $TargetExe
Read-Host "The app has started. Press Enter to close this window"

