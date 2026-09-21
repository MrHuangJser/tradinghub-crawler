#!/usr/bin/env bash
# ============================================================================
#  SPX 期权数据一键抓取（macOS / Linux）
#  - 双击运行，或在终端 ./fetch.command
#  - 默认抓 SPX 并按板块拆分到 out/（每个文件配 .schema.json）
#  - 自定义示例：
#      ./fetch.command --ticker NDX            换标的
#      ./fetch.command --output spx.json       改成单文件模式
#      ./fetch.command --sections levels_summary,orderflow
#      TICKER=SPY ./fetch.command              用环境变量指定标的
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

# ---- 4) 决定是否默认拆分（用户传了单文件类参数就不强制拆分）----
FORCE_SPLIT=1
for a in "$@"; do
  case "$a" in
    --output|--raw|--list-tickers|--compact|--sections|--split|--split-flat)
      FORCE_SPLIT=0 ;;
  esac
done
TICKER="${TICKER:-SPX}"
if [ "$FORCE_SPLIT" = 1 ]; then
  ARGS=(--split out); echo "🚀 抓取 $TICKER 并拆分到 out/ ..."
else
  ARGS=(); echo "🚀 抓取 $TICKER ..."
fi

# ---- 5) 运行 ----
"$PYTHON" spx_options.py --ticker "$TICKER" ${ARGS[@]+"${ARGS[@]}"} "$@"
rc=$?

echo
if [ "$rc" = 0 ]; then
  if [ "$FORCE_SPLIT" = 1 ]; then
    echo "✅ 完成。输出在 ./out/ —— 先看 out/meta.json 找板块，再读对应 .schema.json 和数据。"
  else
    echo "✅ 完成（见上方输出）。"
  fi
else
  echo "❌ 运行失败（退出码 $rc）。请检查上面的错误信息。"
fi

# 交互式（双击/终端）才暂停，管道场景不阻塞
if [ -t 0 ]; then
  read -rn1 -p $'\n按回车关闭...'
fi
exit $rc
