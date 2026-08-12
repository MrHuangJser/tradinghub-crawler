#!/usr/bin/env python3
"""
ES 盘前确定性计划生成器（无 LLM）。

把 TradingHub(ES_SPX/SPX 期权结构) + CBOE(VIX 家族 + EM) + 可选技术位拼成输入，
调用 es_engine 生成盘前计划 JSON（regime/bias/pivot/三级目标/核心防守/squeeze/条件文案）。

用法：
  python3 es_plan.py                         # 全自动：登录拉 TradingHub + CBOE，ES_SPX 计划
  python3 es_plan.py --no-cboe               # 跳过 CBOE（纯结构模式，无需联网 CBOE）
  python3 es_plan.py --em 45 --vwap 7748 --onh 7760 --onl 7735   # 注入技术位/精确 EM
  python3 es_plan.py --ticker ES_SPX --output plan.json
  python3 es_plan.py --recalibrate pre_plan.json   # RTH 重跑快照后，对盘前计划做重校准

离线（用已抓好的单文件）：python3 es_plan.py --es-file out_es.json --spx-file out_spx.json --no-cboe
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


def build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="生成 ES 盘前确定性计划（无 LLM）。")
    p.add_argument("--ticker", default="ES_SPX", help="分析标的（默认 ES_SPX，ES 价格空间）")
    p.add_argument("--basis-ticker", default="SPX", help="用于算嵌入 basis 的标的（默认 SPX）")
    p.add_argument("--no-cboe", action="store_true", help="跳过 CBOE（VIX 家族/EM），纯结构模式")
    # 人工/技术位覆盖
    p.add_argument("--em", type=float, default=None, help="手动指定 0DTE EM（ES/SPX 点数），覆盖 CBOE 估算")
    p.add_argument("--vix", type=float, default=None)
    p.add_argument("--vix1d", type=float, default=None)
    p.add_argument("--realized-range", type=float, default=None, help="已实现波幅(点)，用于 regime")
    for name in ("vwap", "poc", "pdh", "pdl", "onh", "onl", "prior-pivot"):
        p.add_argument(f"--{name}", type=float, default=None)
    # 离线/输出
    p.add_argument("--es-file", default=None, help="离线：读取 ES_SPX 的 extract_ticker JSON")
    p.add_argument("--spx-file", default=None, help="离线：读取 SPX 的 extract_ticker JSON")
    p.add_argument("--output", "-o", default=None)
    p.add_argument("--strict-freshness", action="store_true",
                   help="时效不达标（非今日/滞后）直接拒绝输出，退出码 5")
    p.add_argument("--recalibrate", default=None, help="RTH 重校准：传入盘前计划 JSON 路径")
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)

    # 1) 快照
    if args.es_file:
        snap_es = json.loads(Path(args.es_file).read_text(encoding="utf-8"))
        snap_spx = json.loads(Path(args.spx_file).read_text(encoding="utf-8")) if args.spx_file else {}
    else:
        try:
            snap_es, snap_spx = _live_snapshots(args.ticker, args.basis_ticker)
        except S.AuthError as e:
            print(f"❌ {e}", file=sys.stderr); return 2
        except requests.RequestException as e:
            print(f"❌ TradingHub 请求失败：{e}", file=sys.stderr); return 3

    # 2) RTH 重校准分支
    if args.recalibrate:
        pre_plan = json.loads(Path(args.recalibrate).read_text(encoding="utf-8"))
        result = rth_recalibrate(pre_plan, snap_es, snap_spx)
        text = json.dumps(result, ensure_ascii=False, indent=2)
        (Path(args.output).write_text(text + "\n", encoding="utf-8") if args.output
         else sys.stdout.write(text + "\n"))
        return 0

    # 3) 市场数据（CBOE）
    market = {"vix_family": {}, "warnings": []}
    if not args.no_cboe and args.em is None:
        try:
            market = M.get_market_data(want_em=True)
        except Exception as e:  # noqa: BLE001
            market["warnings"].append(f"CBOE 抓取失败：{e}")

    # 4) 覆盖
    overrides = {k: getattr(args, k.replace("-", "_")) for k in
                 ("em", "vix", "vix1d", "realized-range", "vwap", "poc",
                  "pdh", "pdl", "onh", "onl", "prior-pivot")}
    overrides = {k: v for k, v in overrides.items() if v is not None}

    # 5) 生成计划
    plan = build_plan(snap_es, snap_spx, market=market, overrides=overrides)
    plan["data_warnings"] = market.get("warnings", [])

    # 6) 时效门禁（美东时区感知）
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

    text = json.dumps(plan, ensure_ascii=False, indent=2)
    if args.output:
        Path(args.output).write_text(text + "\n", encoding="utf-8")
        print(f"✅ 计划已写入 {args.output}", file=sys.stderr)
        for line in plan["narrative"]:
            print("  " + line, file=sys.stderr)
    else:
        sys.stdout.write(text + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
