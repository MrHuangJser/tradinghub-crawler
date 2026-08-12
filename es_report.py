#!/usr/bin/env python3
"""
把 es_plan.py 生成的 plan.json 转成 Markdown 盘前报告。

用法：
  python3 es_plan.py --output plan.json
  python3 es_report.py plan.json                 # 位置参数：输入文件
  python3 es_report.py --input plan.json -o report.md
  python3 es_plan.py | python3 es_report.py      # 管道：stdin → stdout
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path


def _num(x, nd=2):
    if x is None:
        return "—"
    if isinstance(x, (int, float)):
        return f"{x:,.{nd}f}"
    return str(x)


def _band(t):
    """目标价位显示：有非退化 band 显示区间，否则单值。"""
    if not t:
        return "—"
    b = t.get("band")
    if b and b[0] != b[1]:
        return f"{b[0]:,.2f}–{b[1]:,.2f}"
    return _num(t.get("level"))


def _vix_line(vf: dict) -> str:
    names = ["VIX", "VIX1D", "VIX9D", "VVIX", "SKEW"]
    parts = [f"{n}={vf[n]['value']}" for n in names if vf.get(n)]
    return "  ".join(parts) if parts else "—"


def _target_row(targets, idx):
    t = targets[idx] if len(targets) > idx else None
    if not t:
        return "—", "—", "—"
    return _band(t), ", ".join(t.get("types") or []), _num(t.get("score"), 3)


def render_md(p: dict) -> str:
    spot = p.get("spot")
    em = (p.get("em") or {}).get("em_0dte_spx") if isinstance(p.get("em"), dict) else None
    vf = p.get("vix_family") or {}
    bull = p.get("bull_targets") or []
    bear = p.get("bear_targets") or []
    sup = p.get("major_long_support")
    sq = p.get("squeeze_zone")
    net_gex = p.get("net_gex_vol")
    flip = p.get("flip")
    gen_now = datetime.now(timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M %Z")

    L: list[str] = []
    L.append(f"# ES 盘前结构报告 — {p.get('ticker','ES_SPX')}\n")
    L.append(f"- **生成时间**: {gen_now}")
    L.append(f"- **数据时点**: {p.get('as_of') or '—'}")
    fr = p.get("freshness")
    if fr:
        L.append(f"- **时效**: {fr.get('status','—')} — {fr.get('message','')}"
                 f"（当前ET {fr.get('now_et','—')} / 会话 {fr.get('session','—')}）")
    L.append(f"- **数据模式**: `{p.get('data_mode','—')}`"
             + ("（含 CBOE VIX 家族 + EM）" if p.get("data_mode") == "full" else "（纯期权结构，无 EM/VIX）"))
    L.append("")

    # 摘要
    L.append("## 摘要\n")
    L.append("| 项目 | 值 |")
    L.append("|---|---|")
    L.append(f"| ES 现价 | {_num(spot)} |")
    L.append(f"| 嵌入 basis (ES−SPX) | {_num(p.get('embedded_basis'))}（TradingHub 内含，仅审计）|")
    L.append(f"| Gamma Flip | {_num(flip)} |")
    gex_sign = "负 → 偏波动扩张" if (net_gex or 0) < 0 else ("正 → 偏均值回归" if (net_gex or 0) > 0 else "—")
    L.append(f"| 净 GEX 成交量 | {_num(net_gex)}（{gex_sign}）|")
    L.append(f"| VIX 家族 | {_vix_line(vf)} |")
    if em:
        L.append(f"| 0DTE Expected Move | ±{_num(em)} 点（上 {_num((spot or 0)+em)} / 下 {_num((spot or 0)-em)}）|")
    L.append("")

    # 状态判断
    L.append("## 状态判断\n")
    L.append(f"- **Regime（波动状态）**: `{p.get('regime','—')}`")
    L.append(f"- **Bias（方向）**: `{p.get('bias','—')}`")
    L.append(f"- **转换位**: {_num(p.get('pivot'))}（buffer {_num(p.get('buffer'))}；来源 "
             f"{', '.join(p.get('pivot_sources') or []) or '—'}）")
    if p.get("regime_notes"):
        L.append("- 判定依据:")
        for n in p["regime_notes"]:
            L.append(f"    - {n}")
    L.append("")

    # 价位地图
    L.append("## 价位地图（ES 价格空间）\n")

    L.append("### 阻力 / 多头目标（上方，近端→远端）\n")
    L.append("| | T1 | T2 | T3 |")
    L.append("|---|---:|---:|---:|")
    r1 = _target_row(bull, 0); r2 = _target_row(bull, 1); r3 = _target_row(bull, 2)
    L.append(f"| 价位 | {r1[0]} | {r2[0]} | {r3[0]} |")
    L.append(f"| 类型 | {r1[1]} | {r2[1]} | {r3[1]} |")
    L.append(f"| 评分 | {r1[2]} | {r2[2]} | {r3[2]} |")
    L.append("")

    L.append("### 支撑 / 空头目标（下方，近端→远端）\n")
    L.append("| | T1 | T2 | T3 |")
    L.append("|---|---:|---:|---:|")
    r1 = _target_row(bear, 0); r2 = _target_row(bear, 1); r3 = _target_row(bear, 2)
    L.append(f"| 价位 | {r1[0]} | {r2[0]} | {r3[0]} |")
    L.append(f"| 类型 | {r1[1]} | {r2[1]} | {r3[1]} |")
    L.append(f"| 评分 | {r1[2]} | {r2[2]} | {r3[2]} |")
    L.append("")

    L.append("### 关键位\n")
    L.append("| 类型 | 价位 | 说明 |")
    L.append("|---|---:|---|")
    L.append(f"| 核心多头防守 | {_num(sup['level']) if sup else '—'} | "
             f"{(', '.join(sup['types']) if sup else '')}；承接则多头RR好，失守取消逢低做多 |")
    sq_note = "；".join(sq.get("rationale") or []) if sq else ""
    L.append(f"| Squeeze 区 | {_num(sq['level']) if sq else '—'} | {sq_note} |")
    nb = p.get("negative_gamma_band")
    near_n = len(p.get("negative_gamma_strikes_near_spot") or [])
    total_n = p.get("negative_gamma_total")
    L.append(f"| 负 Gamma 带 (±1EM) | {f'{nb[0]:,.2f} – {nb[1]:,.2f}' if nb else '—'} | "
             f"现价附近 {near_n}/{total_n} 档为负 Gamma |")
    L.append("")

    # Gamma 结构分析
    gd = p.get("gamma_detail") or {}
    if gd:
        L.append("## Gamma 结构分析")
        L.append("")
        # Top positive/negative table — data only, legend below
        L.append("| 方向 | 行权价 | GEX | 距现价 (点) | 距现价 (%) |")
        L.append("|---|---:|---:|---:|")
        for r in (gd.get("top_positive") or [])[:5]:
            dist = (r["strike"] - spot) if spot else 0
            dist_pct = (dist / spot * 100) if spot else 0
            L.append(f"| 🟢 正 | {r['strike']:,.2f} | {r['gex']:+,.1f} | "
                     f"{dist:+,.1f} | {dist_pct:+.2f} |")
        for r in (gd.get("top_negative") or [])[:5]:
            dist = (r["strike"] - spot) if spot else 0
            dist_pct = (dist / spot * 100) if spot else 0
            L.append(f"| 🔴 负 | {r['strike']:,.2f} | {r['gex']:+,.1f} | "
                     f"{dist:+,.1f} | {dist_pct:+.2f} |")
        L.append("")
        L.append("> 🟢 正 Gamma = dealer 做多 Gamma，逆势对冲（跌了买、涨了卖 → **缓冲价格**）  \n"
                 "> 🔴 负 Gamma = dealer 做空 Gamma，顺势对冲（跌了卖、涨了买 → **加速行情**）\n")

        # Wall summary
        cw = gd.get("call_walls") or {}
        pw = gd.get("put_walls") or {}
        L.append(f"**Gamma Wall（离散 OI 墙）：**\n")
        L.append(f"- Call Wall: 0DTE **{_num(cw.get('0DTE'))}** / 1DTE+ **{_num(cw.get('1DTE+'))}**")
        L.append(f"- Put Wall: 0DTE **{_num(pw.get('0DTE'))}** / 1DTE+ **{_num(pw.get('1DTE+'))}**")
        mlg = gd.get("major_long_gamma")
        msg = gd.get("major_short_gamma")
        if mlg or msg:
            L.append(f"- 主要多头 Gamma 位: {_num(mlg)}  /  主要空头 Gamma 位: {_num(msg)}")

        # Interpretation
        pos_n = gd.get("pos_gamma_near_spot", 0)
        neg_n = gd.get("neg_gamma_near_spot", 0)
        total_entries = gd.get("total_gamma_ladder_entries", 0)
        net_gex = p.get("net_gex_vol")

        L.append(f"\n**对冲行为解读：**\n")
        L.append(f"- Gamma 档共 {total_entries} 档，现价 ±1EM 内正 Gamma {pos_n} 档 / 负 Gamma {neg_n} 档")
        if net_gex is not None:
            pos_peaks = ", ".join(f"{r['strike']:,.2f}" for r in (gd.get("top_positive") or [])[:3])
            neg_peaks = ", ".join(f"{r['strike']:,.2f}" for r in (gd.get("top_negative") or [])[:3])
            if net_gex > 0:
                L.append(f"- 净 GEX **正** ({net_gex:+,.1f})：dealer 整体做多 Gamma → 跌了会接、涨了会压 → **均值回归**")
                L.append(f"- 正 Gamma 峰值在 {pos_peaks}，"
                         f"是 dealer 对冲买盘最强的位置")
            else:
                L.append(f"- 净 GEX **负** ({net_gex:+,.1f})：dealer 整体做空 Gamma → 跌了加速卖、涨了追买 → **波动扩张**")
                L.append(f"- 负 Gamma 峰值在 {neg_peaks}，"
                         f"突破这些位置会触发 dealer 同向对冲 → 加速行情")
            if pos_n > 0 and neg_n > 0:
                dominant = "正 Gamma 主导 → 价格倾向稳定在 Gamma 密集区" if pos_n >= neg_n else "负 Gamma 主导 → 价格在负 Gamma 区易加速"
                L.append(f"- 正/负 Gamma 档比 {pos_n}:{neg_n}，{dominant}")

        L.append("")

    # 条件式计划
    L.append("## 条件式计划\n")
    for s in (p.get("narrative") or []):
        L.append(f"- {s}")
    L.append("")

    # 限制 / 告警
    if p.get("limitations"):
        L.append("## ⚠️ 模型限制\n")
        for x in p["limitations"]:
            L.append(f"- {x}")
        L.append("")
    if p.get("data_warnings"):
        L.append("## 数据告警\n")
        for x in p["data_warnings"]:
            L.append(f"- {x}")
        L.append("")

    L.append("---")
    L.append("*期权位是地图，价格行为是触发器。本报告由确定性引擎生成（无 LLM），为结构判断，非交易信号。*")
    return "\n".join(L) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="plan.json → Markdown 盘前报告")
    ap.add_argument("input", nargs="?", help="plan.json 路径（缺省读 stdin）")
    ap.add_argument("-i", "--input", dest="input_opt", help="plan.json 路径（同位置参数）")
    ap.add_argument("-o", "--output", help="输出 .md 路径（缺省写 stdout）")
    args = ap.parse_args(argv)

    src = args.input_opt or args.input
    if src:
        data = json.loads(Path(src).read_text(encoding="utf-8"))
    else:
        data = json.load(sys.stdin)

    md = render_md(data)
    if args.output:
        Path(args.output).write_text(md, encoding="utf-8")
        print(f"✅ 报告已写入 {args.output}", file=sys.stderr)
    else:
        sys.stdout.write(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
