# -*- coding: utf-8 -*-
"""方向状态机：多空转换位 pivot + bias（文档 9.1）。纯确定性。"""

from __future__ import annotations
from typing import Any


def choose_pivot(flip: float | None,
                 vwap: float | None = None,
                 on_mid: float | None = None,
                 poc: float | None = None,
                 prior_pivot: float | None = None) -> tuple[float | None, list[str]]:
    """转换位 = 可用候选的中位数。无任何技术位时退化为 Gamma Flip。
    文档 2026-08-04 实证：真 pivot 更偏 ON 中轴/VWAP，故有技术位时以中位数融合。"""
    cands = {"flip": flip, "on_mid": on_mid, "vwap": vwap,
             "poc": poc, "prior_pivot": prior_pivot}
    present = {k: v for k, v in cands.items() if v is not None}
    if not present:
        return None, []
    vals = sorted(present.values())
    med = vals[len(vals) // 2]
    # 若只有 flip，pivot=flip；否则用中位数（融合技术结构）
    pivot = flip if len(present) == 1 and "flip" in present else med
    return round(pivot, 2), list(present.keys())


def classify_bias(price: float | None, pivot: float | None,
                  em_total: float | None = None) -> tuple[str | None, float]:
    """bias = price 相对 pivot±buffer。buffer = max(2.0, 0.05×EM)（文档 9.1）。"""
    if price is None or pivot is None:
        return None, 0.0
    buffer = max(2.0, 0.05 * em_total) if em_total else 2.0
    if price > pivot + buffer:
        return "BULLISH", round(buffer, 2)
    if price < pivot - buffer:
        return "BEARISH", round(buffer, 2)
    return "NEUTRAL", round(buffer, 2)
