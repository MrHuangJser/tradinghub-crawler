#!/usr/bin/env python3
"""
ES 盘前一键报告生成器：登录 TradingHub + CBOE → 生成计划 → 输出 report.md

整合 es_plan.py（数据抓取+引擎）和 es_report.py（Markdown 渲染），
一键从零到可读盘前报告。

用法：
  python3 es_run.py                           # 全自动：生成 report.md
  python3 es_run.py -o my_report.md           # 指定输出路径
  python3 es_run.py --stdout                  # 输出到 stdout（管道友好）
  python3 es_run.py --no-cboe                  # 跳过 CBOE（纯结构模式，无需联网 CBOE）
  python3 es_run.py --em 45 --vwap 7748 --onh 7760 --onl 7735  # 注入技术位/精确 EM
  python3 es_run.py --es-file out_es.json --spx-file out_spx.json --no-cboe  # 离线
  python3 es_run.py --recalibrate plan.json    # RTH 重校准后输出报告
  python3 es_run.py --save-plan plan.json      # 同时保存中间 plan JSON
  python3 es_run.py --strict-freshness         # 时效不达标拒绝输出（退出码 5）
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import requests

import spx_options as S          # 复用登录/抓取/抽取
import market_data as M
from es_engine import build_plan, rth_recalibrate
from es_engine import freshness as FR
from es_report import render_md   # plan dict → Markdown 字符串


# ---------------------------------------------------------------------------
# 快照获取（同 es_plan.py）
# ---------------------------------------------------------------------------
def _live_snapshots(ticker: str, basis_ticker: str) -> tuple[dict, dict]:
    """登录 TradingHub，抓两接口，抽取指定标的。"""
    email, password = S.load_credentials(None, None)
    sess = requests.Session()
    S.login(sess, email, password)
    gex = S.fetch_json(sess, S.GEX_URL)
    exp = S.fetch_json(sess, S.EXPOSURE_URL)
    merged = {"gex": gex, "exposure": exp}
    snap = S.extract_ticker(merged, ticker, [])
    basis = S.extract_ticker(merged, basis_ticker, []) if basis_ticker else {}
    return snap, basis


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="ES 盘前一键报告：登录 TradingHub + CBOE → 生成计划 → 输出 report.md",
    )
    # 标的
    p.add_argument("--ticker", default="ES_SPX", help="分析标的（默认 ES_SPX，ES 价格空间）")
    p.add_argument("--basis-ticker", default="SPX", help="用于算嵌入 basis 的标的（默认 SPX）")
    # 数据源
    p.add_argument("--no-cboe", action="store_true", help="跳过 CBOE（VIX 家族/EM），纯结构模式")
    # 人工/技术位覆盖
    p.add_argument("--em", type=float, default=None, help="手动指定 0DTE EM（ES/SPX 点数），覆盖 CBOE 估算")
    p.add_argument("--vix", type=float, default=None)
    p.add_argument("--vix1d", type=float, default=None)
    p.add_argument("--realized-range", type=float, default=None, help="已实现波幅(点)，用于 regime")
    for name in ("vwap", "poc", "pdh", "pdl", "onh", "onl", "prior-pivot"):
        p.add_argument(f"--{name}", type=float, default=None)
    # 离线
    p.add_argument("--es-file", default=None, help="离线：读取 ES_SPX 的 extract_ticker JSON")
    p.add_argument("--spx-file", default=None, help="离线：读取 SPX 的 extract_ticker JSON")
    # 输出
    p.add_argument("--output", "-o", default="report.md",
                   help="输出 Markdown 报告路径（默认 report.md；传 - 输出到 stdout）")
    p.add_argument("--stdout", action="store_true",
                   help="强制输出到 stdout（等价于 -o -）")
    p.add_argument("--save-plan", default=None, metavar="PATH",
                   help="同时保存中间 plan JSON 到指定路径")
    # 时效
    p.add_argument("--strict-freshness", action="store_true",
                   help="时效不达标（非今日/滞后）直接拒绝输出，退出码 5")
    # RTH 重校准
    p.add_argument("--recalibrate", default=None, metavar="PLAN_JSON",
                   help="RTH 重校准：传入盘前 plan.json，抓最新快照后输出重校准报告")
    return p


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------
def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)

    # ---- 1) 快照 ----
    if args.es_file:
        snap_es = json.loads(Path(args.es_file).read_text(encoding="utf-8"))
        snap_spx = json.loads(Path(args.spx_file).read_text(encoding="utf-8")) if args.spx_file else {}
    else:
        try:
            snap_es, snap_spx = _live_snapshots(args.ticker, args.basis_ticker)
        except S.AuthError as e:
            print(f"❌ {e}", file=sys.stderr)
            return 2
        except requests.RequestException as e:
            print(f"❌ TradingHub 请求失败：{e}", file=sys.stderr)
            return 3

    # ---- 2) RTH 重校准分支 ----
    if args.recalibrate:
        pre_plan = json.loads(Path(args.recalibrate).read_text(encoding="utf-8"))
        plan = rth_recalibrate(pre_plan, snap_es, snap_spx)
        # 重校准结果本身也是 plan dict，直接渲染
        # 补充 ticker / spot 信息方便报告渲染
        plan.setdefault("ticker", args.ticker)
        plan.setdefault("spot", (snap_es.get("levels_summary") or {}).get("spot"))
        plan.setdefault("as_of", snap_es.get("captured_at") or snap_es.get("generated_at"))
        plan.setdefault("data_mode", "rth_recalibrate")
        plan.setdefault("vix_family", {})
        plan.setdefault("bull_targets", [])
        plan.setdefault("bear_targets", [])
        plan.setdefault("major_long_support", None)
        plan.setdefault("squeeze_zone", None)
        plan.setdefault("narrative", plan.get("notes") or [])
        plan.setdefault("limitations", [])
        plan.setdefault("data_warnings", [])
    else:
        # ---- 3) 市场数据（CBOE） ----
        market: dict = {"vix_family": {}, "warnings": []}
        if not args.no_cboe and args.em is None:
            try:
                market = M.get_market_data(want_em=True)
            except Exception as e:  # noqa: BLE001
                market["warnings"].append(f"CBOE 抓取失败：{e}")

        # ---- 4) 覆盖 ----
        overrides = {k: getattr(args, k.replace("-", "_")) for k in
                     ("em", "vix", "vix1d", "realized-range", "vwap", "poc",
                      "pdh", "pdl", "onh", "onl", "prior-pivot")}
        overrides = {k: v for k, v in overrides.items() if v is not None}

        # ---- 5) 生成计划 ----
        plan = build_plan(snap_es, snap_spx, market=market, overrides=overrides)
        plan["data_warnings"] = market.get("warnings", [])

        # ---- 6) 时效门禁 ----
        snap_ts = (snap_es.get("levels_summary") or {}).get("timestamp")
        plan["freshness"] = FR.assess(snap_ts)
        fr = plan["freshness"]
        if fr["status"] not in FR.USABLE:
            print(f"{FR.emoji(fr['status'])} 时效告警: {fr['message']} "
                  f"(数据 {fr.get('snapshot_et')} | 当前ET {fr.get('now_et')} | 会话 {fr.get('session')})",
                  file=sys.stderr)
            if args.strict_freshness:
                print("❌ --strict-freshness：时效不达标，拒绝输出。", file=sys.stderr)
                return 5

    # ---- 7) 可选：保存中间 plan JSON ----
    if args.save_plan:
        text = json.dumps(plan, ensure_ascii=False, indent=2)
        Path(args.save_plan).write_text(text + "\n", encoding="utf-8")
        print(f"📋 中间 plan 已保存到 {args.save_plan}", file=sys.stderr)

    # ---- 8) 渲染 Markdown 报告 ----
    md = render_md(plan)

    # ---- 9) 输出 ----
    output_dest = args.output
    if args.stdout:
        output_dest = "-"

    if output_dest == "-":
        sys.stdout.write(md)
    else:
        out_path = Path(output_dest)
        out_path.write_text(md, encoding="utf-8")
        print(f"✅ 报告已写入 {out_path.resolve()}", file=sys.stderr)
        # 摘要预览
        for line in (plan.get("narrative") or [])[:6]:
            print("  " + line, file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
