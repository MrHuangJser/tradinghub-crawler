# -*- coding: utf-8 -*-
"""条件式盘前文案（文档 11）。纯模板拼接，不调 LLM。
只输出条件式结构判断；不生成"先多后空"类路径剧本（文档 20.4 证实不可靠）。"""

from __future__ import annotations
from typing import Any


def gen_narrative(plan: dict) -> list[str]:
    bias, regime = plan.get("bias"), plan.get("regime")
    price, pivot = plan.get("spot"), plan.get("pivot")
    bull = plan.get("bull_targets") or []
    bear = plan.get("bear_targets") or []
    sup = plan.get("major_long_support")
    sq = plan.get("squeeze_zone")
    em = (plan.get("em") or {}).get("em_0dte_spx") if isinstance(plan.get("em"), dict) else plan.get("em")

    txt: list[str] = []

    # 方向 × 波动 组合（文档 10 表）
    if bias == "BULLISH" and regime in ("CHOPPING", "COMPRESSION"):
        txt.append("短线偏多，但更偏震荡/压缩，不宜在近端目标前追价，等回落更优。")
    elif bias == "BEARISH" and regime in ("CHOPPING", "COMPRESSION"):
        txt.append("短线偏空，但更偏震荡/压缩，优先等反弹确认后再动作。")
    elif bias == "BULLISH" and regime == "EXPANSION":
        txt.append("偏多且波动扩张，突破回踩获接受后顺势做多，VWAP 上方只多不空。")
    elif bias == "BEARISH" and regime == "EXPANSION":
        txt.append("偏空且波动扩张，跌破确认后顺势做空，不左侧摸顶。")
    elif bias == "NEUTRAL":
        txt.append("方向中性，区间边缘交易、中部不交易。")

    if price and pivot:
        if price > pivot and bull:
            txt.append(f"价格在转换位 {pivot} 上方，上方先看 {bull[0]['level']}（{', '.join(bull[0]['types'])}）。")
        elif price < pivot and bear:
            txt.append(f"价格在转换位 {pivot} 下方，下方先看 {bear[0]['level']}（{', '.join(bear[0]['types'])}）。")

    if sup:
        txt.append(f"若回撤至核心防守 {sup['level']}（{', '.join(sup['types'])}）出现承接，"
                   "该区域多头风险收益比较好；有效失守则取消逢低做多。")
    if sq:
        rationale = "、".join(sq.get("rationale") or []) or "波动加速区"
        txt.append(f"若核心防守有效失守，下方 {sq['level']} 是潜在{rationale}。")

    if em:
        txt.append(f"0DTE Expected Move ≈ ±{em} 点（用于目标可达性与止损距离的尺度，非保证）。")
    else:
        txt.append("未提供 EM，目标可达性与止损距离未评估（纯结构模式）。")

    txt.append("期权位是地图，价格行为是触发器；以上为结构判断，非交易信号。")
    return txt
