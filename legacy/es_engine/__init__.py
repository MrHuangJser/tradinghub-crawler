# -*- coding: utf-8 -*-
"""ES 盘前分析确定性引擎 —— 把《SPX期权驱动的ES盘前分析算法》文档去 AI 化实现。

输入（全部确定性，无 LLM）：
  snap_es  : spx_options.extract_ticker() 对 ES_SPX 的输出（ES 价格空间）
  snap_spx : 对 SPX 的输出（用于计算嵌入 basis）
  market   : market_data.get_market_data()（CBOE：VIX 家族 + EM）
  overrides: 可选人工/技术位 {em, vix, vix1d, vwap, onh, onl, pdh, pdl, poc, realized_range}

输出：盘前计划 dict（regime/bias/pivot/三级目标/核心防守/squeeze/条件文案/limitations）。
"""
from .engine import build_plan, rth_recalibrate  # noqa: F401
