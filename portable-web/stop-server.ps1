$PidFile = Join-Path $PSScriptRoot "data\server.pid"
if (Test-Path $PidFile) {
  $ServerPid = [int](Get-Content $PidFile -Raw)
  Stop-Process -Id $ServerPid -ErrorAction SilentlyContinue
  Remove-Item $PidFile -Force -ErrorAction SilentlyContinue
}

