# -*- coding: utf-8 -*-
"""波动状态分类：CHOPPING / COMPRESSION / NORMAL / EXPANSION（文档 10）。纯阈值规则。"""

from __future__ import annotations
from typing import Any


def classify_regime(net_gex: float | None,
                    price: float | None,
                    pivot: float | None,
                    buffer: float,
                    call_wall: float | None,
                    put_wall: float | None,
                    em: float | None = None,
                    realized_range: float | None = None,
                    vix1d: float | None = None) -> dict[str, Any]:
    """分层判定：始终有 GEX 分量；EM/realized、VIX1D 有则细化。"""
    notes: list[str] = []
    prob: dict[str, float] = {"range_mean_revert": 0.5, "trend_expansion": 0.5}

    # 1) GEX 主导（始终可用）
    between_walls = (call_wall is not None and put_wall is not None
                     and put_wall < price < call_wall) if price is not None else False
    if net_gex is not None and net_gex > 0 and between_walls:
        prob["range_mean_revert"] = 0.65
        prob["trend_expansion"] = 0.35
        notes.append("净GEX>0 且价格在墙间 → 偏均值回归")
    if (net_gex is not None and net_gex < 0) or (price is not None and pivot is not None and price < pivot):
        prob["trend_expansion"] = 0.6
        prob["range_mean_revert"] = 0.4
        notes.append("净GEX<0 或价格在 Flip 下方 → 偏波动扩张")

    # 2) realized / EM（有 em 才启用）
    regime = "NORMAL"
    if em and realized_range is not None:
        if pivot is not None and price is not None and abs(price - pivot) <= buffer:
            regime = "CHOPPING"; notes.append("价格贴 pivot → CHOPPING")
        elif realized_range < 0.60 * em:
            regime = "COMPRESSION"; notes.append(f"实际波幅 {realized_range:.1f} < 0.6×EM({em:.1f}) → COMPRESSION")
        elif realized_range > 1.20 * em:
            regime = "EXPANSION"; notes.append(f"实际波幅 {realized_range:.1f} > 1.2×EM({em:.1f}) → EXPANSION")
    else:
        # 无 EM：用 GEX 符号给粗 regime
        if net_gex is not None and net_gex < 0:
            regime = "EXPANSION"
        elif net_gex is not None and net_gex > 0:
            regime = "CHOPPING"
        notes.append("无 EM/realized → 仅用 GEX 粗判 regime")

    # 3) VIX1D 极低位规则（文档 5.5 方法C）：vix1d<10 上调 EXPANSION 先验
    if vix1d is not None and vix1d < 10:
        prob["trend_expansion"] = min(0.75, prob["trend_expansion"] + 0.1)
        notes.append(f"VIX1D={vix1d}<10 极低位 → 短端IV无压缩空间，上调扩张先验")

    return {"regime": regime,
            "prob": prob, "notes": notes,
            "inputs": {"net_gex": net_gex, "em": em, "realized_range": realized_range,
                       "vix1d": vix1d}}
