param([string]$InstallDir = "")
$ErrorActionPreference = "Stop"
try {
  if (-not $InstallDir) { $InstallDir = Read-Host "Existing Qingdan folder (example D:\Qingdan)" }
  if (-not [System.IO.Path]::IsPathRooted($InstallDir)) { throw "Enter a full path." }
  $InstallDir = (Resolve-Path -LiteralPath $InstallDir).Path
  $SourceExe = Join-Path $PSScriptRoot "Qingdan.exe"
  $TargetExe = Join-Path $InstallDir "Qingdan.exe"
  if (-not (Test-Path -LiteralPath $SourceExe -PathType Leaf)) { throw "Run this script from the newly extracted portable package." }
  if (-not (Test-Path -LiteralPath (Join-Path $InstallDir "portable.mode") -PathType Leaf)) { throw "Select the existing portable application folder (portable.mode must exist)." }
  if ([System.IO.Path]::GetFullPath($SourceExe) -eq [System.IO.Path]::GetFullPath($TargetExe)) { throw "Extract the NEW package to a different folder before updating." }
  $Running = Get-Process -Name Qingdan,qingdan-desktop -ErrorAction SilentlyContinue
  if ($Running) { throw "Close Qingdan before updating. Your data has not been changed." }
  if (Test-Path -LiteralPath $TargetExe) {
    Copy-Item -LiteralPath $TargetExe -Destination "$TargetExe.previous" -Force
  }
  # Never copy or remove data, backups, or portable.mode from a downloaded package.
  Copy-Item -LiteralPath $SourceExe -Destination "$TargetExe.new" -Force
  Move-Item -LiteralPath "$TargetExe.new" -Destination $TargetExe -Force
  Write-Host "Updated. Existing data and backups are preserved." -ForegroundColor Green
  Start-Process -FilePath $TargetExe -WorkingDirectory $InstallDir
} catch {
  Write-Host $_.Exception.Message -ForegroundColor Red
  exit 1
}
