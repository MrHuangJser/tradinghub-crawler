#!/usr/bin/env bash
# ============================================================================
#  ES 盘前一键报告（macOS / Linux）
#  - 双击运行，或在终端 ./run_report.command
#  - 默认：登录 TradingHub + CBOE → 生成 plan → 输出 report.md
#  - 自定义示例：
#      ./run_report.command --no-cboe              纯结构模式（不联网 CBOE）
#      ./run_report.command --em 45 --vwap 7748 --onh 7760 --onl 7735
#      ./run_report.command -o my_report.md        指定输出路径
#      ./run_report.command --stdout               输出到终端
#      ./run_report.command --save-plan plan.json  同时保存中间 plan JSON
#      ./run_report.command --strict-freshness     时效不达标拒绝输出
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"

# ---- 1) 定位 Python ----
PYTHON=""
for c in python3 python; do
  if command -v "$c" >/dev/null 2>&1; then PYTHON="$c"; break; fi
done
if [ -z "$PYTHON" ]; then
  echo "❌ 未找到 python3，请先安装 Python 3：https://www.python.org/downloads/"
  [ -t 0 ] && read -rn1 -p $'\n按回车关闭...'
  exit 1
fi

# ---- 2) 确保有 requests ----
if ! "$PYTHON" -c "import requests" >/dev/null 2>&1; then
  echo "⚠️  缺少依赖 requests，正在自动安装..."
  "$PYTHON" -m pip install --user -q requests || {
    echo "❌ 自动安装失败，请手动执行：$PYTHON -m pip install requests"
    [ -t 0 ] && read -rn1 -p $'\n按回车关闭...'
    exit 1
  }
fi

# ---- 3) 校验凭据 ----
if [ ! -f config.json ] && [ -z "${TRADINGHUB_EMAIL:-}${TRADINGHUB_PASSWORD:-}" ]; then
  echo "❌ 未找到凭据。请任选一种："
  echo "   1) 复制 config.example.json 为 config.json，填入邮箱和密码；或"
  echo "   2) 设置环境变量：export TRADINGHUB_EMAIL=...; export TRADINGHUB_PASSWORD=..."
  [ -t 0 ] && read -rn1 -p $'\n按回车关闭...'
  exit 1
fi

# ---- 4) 运行 ----
echo "🚀 生成 ES 盘前报告..."
"$PYTHON" es_run.py "$@"
rc=$?

echo
if [ "$rc" = 0 ]; then
  echo "✅ 完成。报告在 ./report.md —— 用 VS Code / Typora / 任意 Markdown 阅读器打开。"
elif [ "$rc" = 5 ]; then
  echo "⚠️  时效不达标，已拒绝输出（--strict-freshness）。"
else
  echo "❌ 运行失败（退出码 $rc）。请检查上面的错误信息。"
fi

# 交互式（双击/终端）才暂停，管道场景不阻塞
if [ -t 0 ]; then
  read -rn1 -p $'\n按回车关闭...'
fi
exit $rc
