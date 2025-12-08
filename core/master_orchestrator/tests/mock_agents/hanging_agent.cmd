@echo off
:: This agent sleeps for a long time to trigger timeout
timeout /t 30 >nul
exit /b 0