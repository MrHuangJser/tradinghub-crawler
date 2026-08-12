# -*- coding: utf-8 -*-
"""价位生成：候选 → 聚类 → 评分 → 多空转换位/三级目标/核心防守/Squeeze（文档 7/8/9）。"""

from __future__ import annotations
from typing import Any

from .options_layer import top_gamma_strikes


def _gex_abs_at(opt: dict, strike: float | None) -> float:
    """某行权价的 |GEX|（从 gamma_ladder 最近档取，用于评分强度）。"""
    if strike is None:
        return 0.0
    ladder = opt.get("gamma_ladder") or []
    if not ladder:
        return 0.0
    nearest = min(ladder, key=lambda r: abs((r.get("strike") or 0) - strike))
    return abs(nearest.get("current_value") or 0.0)


def build_candidates(opt: dict, em: float | None,
                     tech: dict[str, float | None]) -> list[dict]:
    """收集所有候选价位（ES 空间）。每个带 level/type/gex_abs。"""
    spot = opt.get("spot")
    cands: list[dict] = []

    def add(level, typ, boost=1.0):
        if level is None:
            return
        cands.append({"level": float(level), "type": typ,
                      "gex_abs": _gex_abs_at(opt, level) * boost})

    # 期权节点
    add(opt.get("flip"), "gamma_flip")
    for k, v in (opt.get("call_walls") or {}).items():
        add(v, f"call_wall({k})", boost=1.5)
    for k, v in (opt.get("put_walls") or {}).items():
        add(v, f"put_wall({k})", boost=1.5)
    add(opt.get("major_long_gamma"), "major_long_gamma")
    add(opt.get("major_short_gamma"), "major_short_gamma")
    add(opt.get("max_pos_oi_strike"), "max_pos_oi", boost=1.3)
    add(opt.get("max_neg_oi_strike"), "max_neg_oi", boost=1.3)
    add(opt.get("max_pos_vol_strike"), "max_pos_vol")
    add(opt.get("max_neg_vol_strike"), "max_neg_vol")
    for r in top_gamma_strikes(opt, n=6):
        add(r.get("strike"), "top_gamma_strike")
    # EM 边界
    if em and spot:
        add(spot + em, "upper_em")
        add(spot - em, "lower_em")
    # 技术位（可选）
    tech_names = {"vwap": "VWAP", "poc": "POC", "pdh": "PDH", "pdl": "PDL",
                  "onh": "ONH", "onl": "ONL", "prior_pivot": "PRIOR_PIVOT"}
    for key, label in tech_names.items():
        add(tech.get(key), label)
    # 去重 None
    return [c for c in cands if c["level"] is not None and c["gex_abs"] is not None]


def cluster_levels(cands: list[dict], tol: float) -> list[dict]:
    """相近候选合并为一个反应区（文档 7.2）。"""
    rows = sorted(cands, key=lambda c: c["level"])
    clusters: list[dict] = []
    for c in rows:
        if clusters and abs(c["level"] - clusters[-1]["mean"]) <= tol:
            cl = clusters[-1]
            cl["members"].append(c)
            cl["levels"].append(c["level"])
            cl["mean"] = sum(cl["levels"]) / len(cl["levels"])
            cl["gex_abs"] = max(cl["gex_abs"], c["gex_abs"])
        else:
            clusters.append({"members": [c], "levels": [c["level"]],
                             "mean": c["level"], "gex_abs": c["gex_abs"]})
    for cl in clusters:
        cl["types"] = sorted({m["type"] for m in cl["members"]})
        cl["level"] = round(cl["mean"], 2)
        lo, hi = min(cl["levels"]), max(cl["levels"])
        cl["band"] = [round(lo, 2), round(hi, 2)] if len(cl["levels"]) > 1 else None
    return clusters


def _score(cl: dict, price: float, em: float | None,
           max_gex: float, has_tech: bool) -> dict:
    """评分（文档 8）。无可用项的权重重分配到期权强度/邻近度。"""
    # 邻近度：理想目标距现价 0.3~1.0×EM；无 EM 用 5~25 点窗口
    dist = abs(cl["level"] - price)
    if em:
        prox = 1.0 if 0.3 * em <= dist <= 1.0 * em else max(0.0, 1.0 - abs(dist - 0.65 * em) / em)
        em_align = 1.0 if dist <= 1.2 * em else max(0.0, 1.0 - (dist - 1.2 * em) / em)
    else:
        prox = 1.0 if 5 <= dist <= 25 else max(0.0, 1.0 - abs(dist - 15) / 30)
        em_align = None
    opt_strength = (cl["gex_abs"] / max_gex) if max_gex else 0.0
    tech_conf = 1.0 if (has_tech and any(
        t in ("VWAP", "POC", "PDH", "PDL", "ONH", "ONL", "PRIOR_PIVOT") for t in cl["types"])) else None

    # 权重：原文档 0.30/0.25/0.20/0.15/0.10；缺项重分配
    items = [("options", 0.30, opt_strength), ("proximity", 0.20, prox)]
    if em_align is not None:
        items.append(("em_align", 0.15, em_align))
    if tech_conf is not None:
        items.append(("tech", 0.25, tech_conf))
    total_w = sum(w for _, w, _ in items)
    score = sum(v * (w / total_w) for _, w, v in items)
    return {"score": round(score, 3), "detail": {k: round(v, 3) for k, _, v in items}}


def build_levels(opt: dict, price: float, em: float | None,
                 tech: dict[str, float | None]) -> dict[str, Any]:
    cands = build_candidates(opt, em, tech)
    tol = max(1.0, 0.02 * em) if em else 1.0
    clusters = cluster_levels(cands, tol)
    has_tech = any(tech.get(k) is not None for k in ("vwap", "poc", "pdh", "pdl", "onh", "onl"))
    max_gex = max((c["gex_abs"] for c in cands), default=0.0) or 0.0
    for cl in clusters:
        cl["score_info"] = _score(cl, price, em, max_gex, has_tech)
        cl["score"] = cl["score_info"]["score"]

    above = sorted([c for c in clusters if c["level"] > price], key=lambda c: c["level"])
    below = sorted([c for c in clusters if c["level"] < price], key=lambda c: -c["level"])

    def top_n(rows, n=3, nearest_first=True):
        # 先按评分取前 n，再按距现价由近到远排（文档：T1=最近的高权重节点，T3=远端/波动边界）
        picked = sorted(rows, key=lambda c: c["score"], reverse=True)[:n]
        picked.sort(key=lambda c: abs(c["level"] - price))
        return [_brief(c) for c in picked]

    bull_targets = top_n(above)   # 上方目标（近端优先）
    bear_targets = top_n(below)   # 下方目标（近端优先）

    # 核心多头防守：下方有 put_wall/max_neg_oi、且距离足够远（>0.5EM 或 >8 点）的强簇
    def is_put_support(c):
        return any(t.startswith("put_wall") or "neg_oi" in t or t == "lower_em"
                   or t == "max_neg_vol" for t in c["types"])
    dist_thresh = (0.5 * em) if em else 8.0
    support_pool = [c for c in below if is_put_support(c)
                    and abs(c["level"] - price) >= dist_thresh]
    major_long_support = _brief(max(support_pool, key=lambda c: c["score"])) if support_pool else None

    # Squeeze：核心防守下方、落在负 Gamma 区或 lower_em 的最近强簇
    neg_lo = min(opt["neg_gamma_strikes"]) if opt.get("neg_gamma_strikes") else None
    lower_em = (price - em) if em and price else None
    squeeze_anchor = None
    if major_long_support:
        below_support = [c for c in below if c["level"] < major_long_support["level"]]
        if below_support:
            squeeze_anchor = _brief(max(below_support, key=lambda c: c["score"]))
    if not squeeze_anchor and lower_em is not None:
        squeeze_anchor = {"level": round(lower_em, 2), "types": ["lower_em"], "rationale": ["下侧 EM 边界"]}

    squeeze = squeeze_anchor
    if squeeze:
        rationale = list(squeeze.get("rationale") or [])
        if neg_lo is not None and squeeze["level"] <= neg_lo:
            rationale.append("位于负 Gamma 区")
        rationale.append("支撑失守后的波动/流动性加速区（非 dealer 必卖）")
        squeeze["rationale"] = rationale

    # 负 Gamma 带：只取现价 ±1EM 内的负 Gamma 行权价（可操作区），避免整条链的最值无意义
    neg_all = opt.get("neg_gamma_strikes") or []
    lo, hi = (price - em, price + em) if em else (price - 25, price + 25)
    neg_near = sorted(s for s in neg_all if lo <= s <= hi)
    neg_band = ([round(neg_near[0], 2), round(neg_near[-1], 2)]
                if neg_near else None)

    return {
        "tol": round(tol, 2),
        "all_clusters_n": len(clusters),
        "bull_targets": bull_targets,
        "bear_targets": bear_targets,
        "major_long_support": major_long_support,
        "squeeze_zone": squeeze,
        "negative_gamma_band": neg_band,
        "negative_gamma_strikes_near_spot": [round(s, 2) for s in neg_near],
        "negative_gamma_total": len(neg_all),
    }


def _brief(cl: dict) -> dict:
    return {"level": cl["level"], "types": cl["types"], "score": cl["score"],
            "band": cl.get("band")}
