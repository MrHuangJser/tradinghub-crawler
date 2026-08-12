# -*- coding: utf-8 -*-
"""引擎编排：build_plan（文档 18 伪代码）+ rth_recalibrate（文档 20.5）。确定性，无 LLM。"""

from __future__ import annotations
from typing import Any

from .options_layer import extract_options, embedded_basis
from .direction import choose_pivot, classify_bias
from .regime import classify_regime
from .levels import build_levels
from .narrative import gen_narrative


def _vix(market: dict, overrides: dict) -> tuple[float | None, float | None, dict]:
    vf = (market or {}).get("vix_family") or {}
    vix = overrides.get("vix") or (vf.get("VIX") or {}).get("value")
    vix1d = overrides.get("vix1d") or (vf.get("VIX1D") or {}).get("value")
    return vix, vix1d, vf


def _em(market: dict, overrides: dict) -> tuple[float | None, dict | None]:
    if overrides.get("em") is not None:
        em = float(overrides["em"])
        return em, {"em_0dte_spx": em, "method": "manual override"}
    em_obj = (market or {}).get("em") or {}
    em = em_obj.get("em_0dte_spx")
    return em, em_obj


def build_plan(snap_es: dict, snap_spx: dict | None,
               market: dict | None = None, overrides: dict | None = None) -> dict[str, Any]:
    """生成盘前计划。snap_es=ES_SPX 快照, snap_spx=SPX 快照(算 basis), market=CBOE, overrides=人工/技术位。"""
    overrides = overrides or {}
    market = market or {}
    opt = extract_options(snap_es)
    price = opt["spot"]

    em, em_obj = _em(market, overrides)
    vix, vix1d, vf = _vix(market, overrides)

    tech = {k: overrides.get(k) for k in
            ("vwap", "poc", "pdh", "pdl", "onh", "onl", "prior_pivot")}
    on_mid = None
    if tech.get("onh") is not None and tech.get("onl") is not None:
        on_mid = round((tech["onh"] + tech["onl"]) / 2, 2)

    # 转换位 + bias
    pivot, pivot_sources = choose_pivot(opt["flip"], vwap=tech.get("vwap"),
                                        on_mid=on_mid, poc=tech.get("poc"),
                                        prior_pivot=tech.get("prior_pivot"))
    bias, buffer = classify_bias(price, pivot, em_total=(em * 2) if em else None)  # em*2 ≈ 全幅

    # regime
    regime = classify_regime(
        net_gex=opt["net_gex_vol"], price=price, pivot=pivot, buffer=buffer,
        call_wall=opt["call_walls"].get("0DTE"), put_wall=opt["put_walls"].get("0DTE"),
        em=em, realized_range=overrides.get("realized_range"), vix1d=vix1d)

    # 价位
    levels = build_levels(opt, price, em, tech)

    limitations: list[str] = []
    data_mode = "full" if em else "structure_only"
    if not em:
        limitations.append("未提供 EM：buffer 固定 2 点、无目标可达性/止损尺度、regime 仅 GEX 粗判")
    if not any(tech.values()):
        limitations.append("未提供技术位(VWAP/POC/PDH/ON)：pivot 退化为 Gamma Flip、无技术共振评分")
    if on_mid is None:
        limitations.append("未提供 ONH/ONL：转换位未纳入隔夜中轴(文档实证的重要候选)")

    # Gamma / Delta 结构详情（用于报告渲染）
    from .options_layer import top_gamma_strikes as _top_g
    of_raw = snap_es.get("orderflow") or {}
    pos_top = _top_g(opt, n=5, side="positive")
    neg_top = _top_g(opt, n=5, side="negative")
    pos_near = sorted([s for s in (opt.get("pos_gamma_strikes") or [])
                       if price - (em or 25) <= s <= price + (em or 25)])
    neg_near = levels.get("negative_gamma_strikes_near_spot") or []
    gamma_detail = {
        "top_positive": [{"strike": r["strike"], "gex": round(r.get("current_value") or 0, 1)}
                         for r in pos_top],
        "top_negative": [{"strike": r["strike"], "gex": round(r.get("current_value") or 0, 1)}
                         for r in neg_top],
        "call_walls": opt.get("call_walls", {}),
        "put_walls": opt.get("put_walls", {}),
        "major_long_gamma": opt.get("major_long_gamma"),
        "major_short_gamma": opt.get("major_short_gamma"),
        "pos_gamma_near_spot": len(pos_near),
        "neg_gamma_near_spot": len(neg_near),
        "total_gamma_ladder_entries": len(opt.get("gamma_ladder") or []),
    }

    plan = {
        "ticker": snap_es.get("ticker", "ES_SPX"),
        "spot": price,
        "as_of": snap_es.get("captured_at") or snap_es.get("generated_at"),
        "embedded_basis": embedded_basis(snap_es, snap_spx or {}),
        "data_mode": data_mode,
        "vix": vix, "vix1d": vix1d, "vix_family": vf,
        "em": em_obj,
        "flip": opt["flip"],
        "net_gex_vol": opt["net_gex_vol"],
        "regime": regime["regime"], "regime_notes": regime["notes"],
        "bias": bias,
        "pivot": pivot, "pivot_sources": pivot_sources, "buffer": buffer,
        "levels": levels,
        # 平铺一份方便读取（narrative 用）
        "bull_targets": levels["bull_targets"],
        "bear_targets": levels["bear_targets"],
        "major_long_support": levels["major_long_support"],
        "squeeze_zone": levels["squeeze_zone"],
        "negative_gamma_band": levels["negative_gamma_band"],
        "negative_gamma_strikes_near_spot": levels["negative_gamma_strikes_near_spot"],
        "negative_gamma_total": levels["negative_gamma_total"],
        "gamma_detail": gamma_detail,
        "delta_detail": {
            # 0DTE (intraday flow)
            "dex_0dte": round(of_raw.get("agg_dex") or 0, 1),
            "call_dex_0dte": round(of_raw.get("agg_call_dex") or 0, 1),
            "put_dex_0dte": round(of_raw.get("agg_put_dex") or 0, 1),
            "cvr_0dte": round(of_raw.get("zcvr") or 0, 1),       # Call Volume Ratio >0 = call heavy
            "gex_ratio_0dte": round(of_raw.get("zgr") or 0, 1),   # GEX ratio
            "vanna_0dte": round(of_raw.get("zvanna") or 0, 1),
            "charm_0dte": round(of_raw.get("zcharm") or 0, 1),
            # 1DTE+ (structural / longer-term)
            "dex_1dte": round(of_raw.get("one_agg_dex") or 0, 1),
            "call_dex_1dte": round(of_raw.get("one_agg_call_dex") or 0, 1),
            "put_dex_1dte": round(of_raw.get("one_agg_put_dex") or 0, 1),
            "cvr_1dte": round(of_raw.get("ocvr") or 0, 1),
            "gex_ratio_1dte": round(of_raw.get("ogr") or 0, 1),
            "vanna_1dte": round(of_raw.get("ovanna") or 0, 1),
            "charm_1dte": round(of_raw.get("ocharm") or 0, 1),
            # Net totals
            "net_dex": round(of_raw.get("net_dex") or 0, 1),
            "net_call_dex": round(of_raw.get("net_call_dex") or 0, 1),
            "net_put_dex": round(of_raw.get("net_put_dex") or 0, 1),
            # Flow direction
            "dexoflow": round(of_raw.get("dexoflow") or 0, 1),
            "gexoflow": round(of_raw.get("gexoflow") or 0, 1),
            "cvroflow": round(of_raw.get("cvroflow") or 0, 1),
            # Key strikes
            "max_pos_oi_strike": opt.get("max_pos_oi_strike"),
            "max_neg_oi_strike": opt.get("max_neg_oi_strike"),
            "max_pos_vol_strike": opt.get("max_pos_vol_strike"),
            "max_neg_vol_strike": opt.get("max_neg_vol_strike"),
        },
        "limitations": limitations,
        "source_flow": opt["flow"],
    }
    plan["narrative"] = gen_narrative(plan)
    return plan


def rth_recalibrate(pre_plan: dict, rth_snap_es: dict,
                    snap_spx: dict | None = None) -> dict[str, Any]:
    """RTH 重校准（文档 20.5）：对比盘前与 RTH 快照，按规则判定作废。"""
    rth_opt = extract_options(rth_snap_es)
    pre_flip = pre_plan.get("flip")
    new_flip = rth_opt.get("flip")
    flip_shift = abs(new_flip - pre_flip) if (pre_flip and new_flip) else None

    result = {
        "pre_flip": pre_flip, "rth_flip": new_flip,
        "flip_shift": round(flip_shift, 2) if flip_shift is not None else None,
        "dynamic_gamma_invalidated": flip_shift is not None and flip_shift > 10,
    }

    # 墙位变化（排名/水平）
    pre_walls = {k: v for k, v in (pre_plan.get("source_flow") or {}).items()}
    def walls(o):
        return (o.get("call_walls", {}).get("0DTE"), o.get("put_walls", {}).get("0DTE"))
    pre_w, rth_w = walls(extract_options_from_plan(pre_plan)), walls(rth_opt)
    wall_shift = max(
        abs((pre_w[0] or 0) - (rth_w[0] or 0)) if pre_w[0] and rth_w[0] else 0,
        abs((pre_w[1] or 0) - (rth_w[1] or 0)) if pre_w[1] and rth_w[1] else 0,
    )
    result["wall_shift"] = round(wall_shift, 2)
    result["oi_walls_invalidated"] = wall_shift > 20

    # 资金流符号翻转 → 仅作废路径叙事
    pre_flow = pre_plan.get("source_flow") or {}
    rth_flow = rth_opt.get("flow") or {}
    def sign(x):
        return None if x is None else (1 if x > 0 else -1 if x < 0 else 0)
    flow_flipped = (sign(pre_flow.get("zcvr")) != sign(rth_flow.get("zcvr"))) or \
                   (sign(pre_flow.get("net_gex_vol")) != sign(rth_flow.get("net_gex_vol")))
    result["narrative_path_invalidated"] = bool(flow_flipped)
    result["rth_flow"] = rth_flow

    notes = []
    if result["dynamic_gamma_invalidated"]:
        notes.append(f"Flip 偏移 {result['flip_shift']} > 10 点：盘前动态 Gamma 状态作废，以 RTH 新结构重算。")
    if result["oi_walls_invalidated"]:
        notes.append("墙位迁移 > 20 点：离散 OI 墙重算。")
    if result["narrative_path_invalidated"]:
        notes.append("0DTE CVR 或净 GEX 成交量符号翻转：路径叙事作废（价位可保留）。")
    result["notes"] = notes
    return result


def extract_options_from_plan(plan: dict) -> dict:
    """从已存盘前计划里复原期权结构供重校准用（轻量）。"""
    return {
        "flip": plan.get("flip"),
        "call_walls": {"0DTE": None, "1DTE+": None},
        "put_walls": {"0DTE": None, "1DTE+": None},
    }
