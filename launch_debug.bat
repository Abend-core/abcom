@echo off
REM Lancer Abcom avec debug logging visible dans PowerShell
powershell -NoExit -Command "cd '$env:LOCALAPPDATA\abcom'; Write-Host 'Lancement d''Abcom avec logging...' -ForegroundColor Green; Write-Host 'Clique sur le bouton + et regarde les logs' -ForegroundColor Yellow; & '.\abcom_debug.exe' 'ra' 2>&1 | ForEach-Object { Write-Host $_ }"




