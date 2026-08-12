@echo off
setlocal enableextensions
cd /d "%~dp0"

set "PY="
py -3 -c "import sys" >nul 2>&1 && set "PY=py -3"
if not defined PY python -c "import sys" >nul 2>&1 && set "PY=python"
if not defined PY python3 -c "import sys" >nul 2>&1 && set "PY=python3"
if not defined PY (
  echo [ERROR] Python 3 not found.
  pause
  exit /b 1
)

%PY% -c "import requests" >nul 2>&1
if errorlevel 1 (
  echo [WARN] Missing requests, installing...
  %PY% -m pip install --user -q requests
  if errorlevel 1 (
    echo [ERROR] pip install failed.
    pause
    exit /b 1
  )
)

if not exist config.json (
  if "%TRADINGHUB_EMAIL%"=="" (
    echo [ERROR] No credentials found.
    pause
    exit /b 1
  )
)

echo [INFO] Generating ES pre-market report...
%PY% es_run.py %*
set RC=%errorlevel%
echo.
if %RC%==0 (
  echo [OK] Done. Report saved to .\report.md
) else if %RC%==5 (
  echo [WARN] Stale data rejected.
) else (
  echo [ERROR] Failed with exit code %RC%.
)
pause
exit /b %RC%
