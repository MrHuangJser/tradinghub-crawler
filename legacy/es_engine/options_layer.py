# -*- coding: utf-8 -*-
"""期权层：从 ES_SPX 快照提取 flip/墙/节点/正负 Gamma 区（ES 价格空间，TradingHub 已算好）。"""

from __future__ import annotations
from typing import Any


def extract_options(snap_es: dict) -> dict:
    """把 ES_SPX 的 extract_ticker 输出整理成引擎用的期权结构。"""
    lv = snap_es.get("levels_summary") or {}
    of = snap_es.get("orderflow") or {}
    gl = snap_es.get("gamma_ladder") or {}
    ex = snap_es.get("exposure") or {}
    metrics = (ex.get("metrics") or {})

    # 逐行权价 GEX（net/zero/one）→ 正负 Gamma 区
    gex_pts = metrics.get("gex") or []
    neg_gamma_strikes = sorted({p["strike"] for p in gex_pts if (p.get("net") or 0) < 0})
    pos_gamma_strikes = sorted({p["strike"] for p in gex_pts if (p.get("net") or 0) > 0})

    ladder = gl.get("ladder") or []

    return {
        "spot": lv.get("spot"),
        "flip": lv.get("zero_gamma"),                       # Gamma Flip（ES 空间）
        "net_gex_vol": lv.get("net_gex_vol"),
        "net_gex_oi": lv.get("net_gex_oi"),
        # 墙（离散节点）
        "call_walls": {"0DTE": of.get("zero_mcall"), "1DTE+": of.get("one_mcall")},
        "put_walls":  {"0DTE": of.get("zero_mput"),  "1DTE+": of.get("one_mput")},
        # 状态线
        "major_long_gamma":  of.get("z_mlgamma"),           # 主要多 Gamma
        "major_short_gamma": of.get("z_msgamma"),           # 主要空 Gamma
        # 极值行权价
        "max_pos_oi_strike":  lv.get("mpos_oi"),
        "max_neg_oi_strike":  lv.get("mneg_oi"),
        "max_pos_vol_strike": lv.get("mpos_vol"),
        "max_neg_vol_strike": lv.get("mneg_vol"),
        # 逐档
        "gamma_ladder": ladder,
        "neg_gamma_strikes": neg_gamma_strikes,
        "pos_gamma_strikes": pos_gamma_strikes,
        # 0DTE 资金流（RTH 重校准用）
        "flow": {
            "zcvr": of.get("zcvr"), "ocvr": of.get("ocvr"),
            "zgr": of.get("zgr"), "ogr": of.get("ogr"),
            "net_gex_vol": lv.get("net_gex_vol"),
        },
    }


def embedded_basis(snap_es: dict, snap_spx: dict) -> float | None:
    """TradingHub 内含的 ES−SPX basis（透明审计用，不做计算依据）。"""
    a = (snap_es.get("levels_summary") or {}).get("spot")
    b = (snap_spx.get("levels_summary") or {}).get("spot")
    if a is None or b is None:
        return None
    return round(a - b, 4)


def top_gamma_strikes(opt: dict, n: int = 8, side: str | None = None) -> list[dict]:
    """按 |current_value| 取前 N 个 Gamma 行权价；side 可选 positive/negative。"""
    rows = opt.get("gamma_ladder") or []
    if side:
        rows = [r for r in rows if r.get("side") == side]
    rows = sorted(rows, key=lambda r: abs(r.get("current_value") or 0), reverse=True)
    return rows[:n]
