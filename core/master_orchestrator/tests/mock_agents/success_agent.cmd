@echo off
:: This agent returns a valid JSON response
echo {
echo   "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
echo   "api_version": "v1",
echo   "status": "success",
echo   "code": 0,
echo   "result": {
echo     "output_type": "text",
echo     "data": "Test successful"
echo   }
echo }
exit /b 0