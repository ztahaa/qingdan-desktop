@echo off
cd /d "%~dp0"
start "Qingdan Server" /min powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0server.ps1"

