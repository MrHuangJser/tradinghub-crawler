@echo off
REM ===========================================================================
REM  ES 盘前一键报告（Windows）
REM  - 双击 run_report.bat 运行
REM  - 默认：登录 TradingHub + CBOE → 生成 plan → 输出 report.md
REM  - 自定义示例（在 cmd 里）：
REM      run_report.bat --no-cboe              纯结构模式（不联网 CBOE）
REM      run_report.bat --em 45 --vwap 7748 --onh 7760 --onl 7735   注入技术位
REM      run_report.bat -o my_report.md        指定输出路径
REM      run_report.bat --stdout               输出到终端
REM      run_report.bat --save-plan plan.json  同时保存中间 plan JSON
REM      run_report.bat --strict-freshness     时效不达标拒绝输出
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

REM ---- 4) 运行 ----
echo 🚀 生成 ES 盘前报告...
%PY% es_run.py %*
set "RC=%errorlevel%"

echo.
if "%RC%"=="0" (
  echo [OK] 完成。报告在 .\report.md —— 用 VS Code / Typora / 任意 Markdown 阅读器打开。
) else if "%RC%"=="5" (
  echo [WARN] 时效不达标，已拒绝输出（--strict-freshness）。
) else (
  echo [ERROR] 运行失败（退出码 %RC%）。请检查上面的错误信息。
)

pause
exit /b %RC%
