@echo off
REM Launch Abcom with logs redirected to a file
cd /d "%LOCALAPPDATA%\abcom"
echo Launching Abcom with logging...
abcom_new.exe %USERNAME% > logs.txt 2>&1
pause
