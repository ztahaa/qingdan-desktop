$ErrorActionPreference = "Stop"
$Port = 38421
$Root = $PSScriptRoot
$AppDir = Join-Path $Root "app"
$DataDir = Join-Path $Root "data"
$StateFile = Join-Path $DataDir "state.json"
$PidFile = Join-Path $DataDir "server.pid"
$Utf8 = New-Object System.Text.UTF8Encoding($false)

New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
[System.IO.File]::WriteAllText($PidFile, [string]$PID, $Utf8)

function Send-Response($Stream, [int]$Status, [string]$ContentType, [byte[]]$Body) {
  $StatusText = switch ($Status) {
    200 { "OK" }
    204 { "No Content" }
    400 { "Bad Request" }
    404 { "Not Found" }
    405 { "Method Not Allowed" }
    500 { "Internal Server Error" }
    default { "Error" }
  }
  $Headers = "HTTP/1.1 $Status $StatusText`r`nContent-Type: $ContentType`r`nContent-Length: $($Body.Length)`r`nCache-Control: no-store`r`nConnection: close`r`n`r`n"
  $HeaderBytes = [System.Text.Encoding]::ASCII.GetBytes($Headers)
  $Stream.Write($HeaderBytes, 0, $HeaderBytes.Length)
  if ($Body.Length -gt 0) { $Stream.Write($Body, 0, $Body.Length) }
  $Stream.Flush()
}

function Read-Request($Stream) {
  $Header = New-Object System.Collections.Generic.List[byte]
  $Match = 0
  while ($Header.Count -lt 65536) {
    $Value = $Stream.ReadByte()
    if ($Value -lt 0) { break }
    $Header.Add([byte]$Value)
    if (($Match -eq 0 -or $Match -eq 2) -and $Value -eq 13) { $Match++ }
    elseif (($Match -eq 1 -or $Match -eq 3) -and $Value -eq 10) { $Match++ }
    else { $Match = 0 }
    if ($Match -eq 4) { break }
  }
  $Text = [System.Text.Encoding]::ASCII.GetString($Header.ToArray())
  $Lines = $Text -split "`r`n"
  $First = $Lines[0] -split " "
  if ($First.Length -lt 2) { throw "Invalid request" }
  $Headers = @{}
  foreach ($Line in $Lines[1..($Lines.Length - 1)]) {
    $At = $Line.IndexOf(":")
    if ($At -gt 0) { $Headers[$Line.Substring(0, $At).Trim().ToLowerInvariant()] = $Line.Substring($At + 1).Trim() }
  }
  $Length = 0
  if ($Headers.ContainsKey("content-length")) { $Length = [int]$Headers["content-length"] }
  if ($Length -lt 0 -or $Length -gt 67108864) { throw "Request too large" }
  $Body = New-Object byte[] $Length
  $Offset = 0
  while ($Offset -lt $Length) {
    $Read = $Stream.Read($Body, $Offset, $Length - $Offset)
    if ($Read -le 0) { break }
    $Offset += $Read
  }
  if ($Offset -ne $Length) { throw "Incomplete request" }
  return [PSCustomObject]@{ Method = $First[0]; Path = $First[1].Split("?")[0]; Body = $Body; Headers = $Headers }
}

function Get-MimeType([string]$Path) {
  switch ([System.IO.Path]::GetExtension($Path).ToLowerInvariant()) {
    ".html" { return "text/html; charset=utf-8" }
    ".js" { return "text/javascript; charset=utf-8" }
    ".css" { return "text/css; charset=utf-8" }
    ".json" { return "application/json; charset=utf-8" }
    ".svg" { return "image/svg+xml" }
    ".png" { return "image/png" }
    ".ico" { return "image/x-icon" }
    default { return "application/octet-stream" }
  }
}

$Listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
try {
  $Listener.Start()
} catch {
  Start-Process "http://127.0.0.1:$Port/"
  exit 0
}

Start-Process "http://127.0.0.1:$Port/"
Write-Host "Qingdan is running at http://127.0.0.1:$Port/"
Write-Host "Keep this window open. Close it to stop Qingdan."

try {
  while ($true) {
    $Client = $Listener.AcceptTcpClient()
    try {
      $Stream = $Client.GetStream()
      $Request = Read-Request $Stream

      if ($Request.Headers["host"] -ne "127.0.0.1:$Port") {
        Send-Response $Stream 400 "text/plain" ($Utf8.GetBytes("Invalid host"))
        continue
      }
      if ($Request.Method -ne "GET" -and $Request.Headers["origin"] -ne "http://127.0.0.1:$Port") {
        Send-Response $Stream 400 "text/plain" ($Utf8.GetBytes("Invalid origin"))
        continue
      }
      if ($Request.Path -eq "/api/open-directory" -and $Request.Method -eq "POST") {
        $InputData = $Utf8.GetString($Request.Body) | ConvertFrom-Json
        $Folder = [string]$InputData.path
        if (-not [System.IO.Path]::IsPathRooted($Folder) -or $Folder.Contains('"') -or -not (Test-Path -LiteralPath $Folder -PathType Container)) {
          Send-Response $Stream 400 "text/plain" ($Utf8.GetBytes("Invalid directory"))
          continue
        }
        $Folder = (Resolve-Path -LiteralPath $Folder).Path
        Start-Process -FilePath "explorer.exe" -ArgumentList ('"' + $Folder + '"')
        Send-Response $Stream 204 "text/plain" ([byte[]]@())
        continue
      }

      if ($Request.Path -eq "/api/state") {
        if ($Request.Method -eq "GET") {
          $Json = if (Test-Path $StateFile) { [System.IO.File]::ReadAllText($StateFile, $Utf8) } else { "{}" }
          Send-Response $Stream 200 "application/json; charset=utf-8" ($Utf8.GetBytes($Json))
        } elseif ($Request.Method -eq "PUT") {
          $Json = $Utf8.GetString($Request.Body)
          try { $null = $Json | ConvertFrom-Json } catch { throw "Invalid JSON" }
          $Temp = "$StateFile.tmp"
          [System.IO.File]::WriteAllText($Temp, $Json, $Utf8)
          Move-Item -Force $Temp $StateFile
          Send-Response $Stream 204 "text/plain" ([byte[]]@())
        } else {
          Send-Response $Stream 405 "text/plain" ($Utf8.GetBytes("Method not allowed"))
        }
        continue
      }

      if ($Request.Method -ne "GET") {
        Send-Response $Stream 405 "text/plain" ($Utf8.GetBytes("Method not allowed"))
        continue
      }

      $Relative = [System.Uri]::UnescapeDataString($Request.Path.TrimStart([char]'/'))
      if ([string]::IsNullOrWhiteSpace($Relative)) { $Relative = "index.html" }
      $AppRoot = [System.IO.Path]::GetFullPath($AppDir) + [System.IO.Path]::DirectorySeparatorChar
      $FilePath = [System.IO.Path]::GetFullPath((Join-Path $AppDir $Relative))
      if (-not $FilePath.StartsWith($AppRoot, [System.StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path $FilePath -PathType Leaf)) {
        Send-Response $Stream 404 "text/plain" ($Utf8.GetBytes("Not found"))
        continue
      }
      $Bytes = [System.IO.File]::ReadAllBytes($FilePath)
      Send-Response $Stream 200 (Get-MimeType $FilePath) $Bytes
    } catch {
      if ($Stream) { Send-Response $Stream 500 "text/plain" ($Utf8.GetBytes($_.Exception.Message)) }
    } finally {
      $Client.Close()
    }
  }
} finally {
  $Listener.Stop()
  if (Test-Path $PidFile) { Remove-Item $PidFile -Force }
}


