@echo off
REM ===========================================================================
REM  SPX 期权数据一键抓取（Windows）
REM  - 双击 fetch.bat 运行
REM  - 默认抓 SPX 并按板块拆分到 out\（每个文件配 .schema.json）
REM  - 自定义示例（在 cmd 里）：
REM      fetch.bat --ticker NDX            换标的
REM      fetch.bat --output spx.json       改成单文件模式
REM      fetch.bat --sections levels_summary,orderflow
REM      set TICKER=SPY && fetch.bat       用环境变量指定标的
REM ===========================================================================
chcp 65001 >nul
cd /d "%~dp0"
setlocal enableextensions

REM ---- 1) 定位 Python ----
set "PY="
py -3 -c "import sys" >nul 2>&1 && set "PY=py -3"
if not defined PY python -c "import sys" >nul 2>&1 && set "PY=python"
if not defined PY python3 -c "import sys" >nul 2>&1 && set "PY=python3"
if not defined PY (
  echo [ERROR] 未找到 Python 3，请先安装：https://www.python.org/downloads/
  echo （安装时勾选 "Add Python to PATH"）
  pause
  exit /b 1
)

REM ---- 2) 确保有 requests ----
%PY% -c "import requests" >nul 2>&1
if errorlevel 1 (
  echo [WARN] 缺少依赖 requests，正在自动安装...
  %PY% -m pip install --user -q requests
  if errorlevel 1 (
    echo [ERROR] 自动安装失败，请手动执行：%PY% -m pip install requests
    pause
    exit /b 1
  )
)

REM ---- 3) 校验凭据 ----
if not exist config.json (
  if "%TRADINGHUB_EMAIL%"=="" (
    echo [ERROR] 未找到凭据。请任选一种：
    echo    1^) 复制 config.example.json 为 config.json，填入邮箱和密码；或
    echo    2^) 设置环境变量：set TRADINGHUB_EMAIL=... ^&^& set TRADINGHUB_PASSWORD=...
    pause
    exit /b 1
  )
)

REM ---- 4) 决定是否默认拆分（用户传了单文件类参数就不强制拆分）----
set "FORCE_SPLIT=1"
for %%A in (%*) do (
  if "%%~A"=="--output" set "FORCE_SPLIT=0"
  if "%%~A"=="--raw" set "FORCE_SPLIT=0"
  if "%%~A"=="--list-tickers" set "FORCE_SPLIT=0"
  if "%%~A"=="--compact" set "FORCE_SPLIT=0"
  if "%%~A"=="--sections" set "FORCE_SPLIT=0"
  if "%%~A"=="--split" set "FORCE_SPLIT=0"
  if "%%~A"=="--split-flat" set "FORCE_SPLIT=0"
)
if "%TICKER%"=="" set "TICKER=SPX"

REM ---- 5) 运行 ----
if "%FORCE_SPLIT%"=="1" (
  echo [INFO] 抓取 %TICKER% 并拆分到 out\ ...
  %PY% spx_options.py --ticker %TICKER% --split out %*
) else (
  echo [INFO] 抓取 %TICKER% ...
  %PY% spx_options.py --ticker %TICKER% %*
)
set "RC=%errorlevel%"

echo.
if "%RC%"=="0" (
  if "%FORCE_SPLIT%"=="1" (
    echo [OK] 完成。输出在 .\out\ —— 先看 out\meta.json 找板块，再读对应 .schema.json 和数据。
  ) else (
    echo [OK] 完成（见上方输出）。
  )
) else (
  echo [ERROR] 运行失败（退出码 %RC%）。请检查上面的错误信息。
)

pause
exit /b %RC%
