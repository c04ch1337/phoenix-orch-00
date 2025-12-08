@echo off
:: This agent always fails with a non-zero exit code
echo Some error text that is not valid JSON
exit /b 1