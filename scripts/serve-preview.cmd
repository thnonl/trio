@echo off
setlocal
cd /d E:\3touch
echo LAUNCHED %DATE% %TIME% > "%TEMP%\trio-preview.log"
node "E:\3touch\scripts\serve.js" < NUL >> "%TEMP%\trio-preview.log" 2>&1
echo EXITED %DATE% %TIME% >> "%TEMP%\trio-preview.log"
