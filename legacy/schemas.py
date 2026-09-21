# -*- coding: utf-8 -*-
"""
每个拆分数据文件对应的字段 schema（中文释义），供 AI 感知每个字段含义。
被 spx_options.py 在 --split 时引用，与数据文件同目录、同名 .schema.json。

schema 文档结构：
  schema_version : 文档格式版本
  title          : 文件中文标题
  page_section   : 对应页面上的板块位置
  data_source    : 原始 API 数据来源
  fields         : 对象型字段的逐字段说明 {字段名: {type, unit?, desc}}
  tuple_items    : 当某字段是"位置数组"时，逐位置说明
"""

import copy
import json

SCHEMA_VERSION = "tradinghub-options-v1"

# ---------- 通用释义片段（复用） ----------
_DTE_MODE_DESC = (
    "DTE 模式：zero=0DTE（当日到期），one=1DTE+（次日及以后到期），"
    "net=90 天（约 90 天累计）。对应页面上图表的 0DTE / 1DTE+ / 90天 切换。"
)

_GEX_UNIT = "Gamma 暴露值（已按合约乘数和符号方向归一化，可正可负）"
_GAMMA_LADDER_TUPLE = {
    "desc": "每个行权价一档；raw_row 是后端原始 7 元组（与命名字段重复，保留以无损）",
    "positions": {
        "0": {"name": "strike", "type": "number", "unit": "指数点", "desc": "行权价"},
        "1": {"name": "call_component", "type": "number", "desc": "call 侧希腊值分量（推断；远 OTM 行权价常为 0）"},
        "2": {"name": "put_component", "type": "number", "desc": "put 侧希腊值分量（推断；远 OTM 行权价常为 0）"},
        "3": {"name": "value", "type": "number", "desc": "该行权价的指标主值（净 GEX）"},
        "4": {"name": "lookback", "type": "array<number>", "desc": "近若干次刷新的历史值（用于看变化趋势）"},
        "5": {"name": "reserved_0", "type": "number", "desc": "保留位，常为 0"},
        "6": {"name": "reserved_null", "type": "null", "desc": "保留位，常为 null"},
    },
}

# ---------- 各文件 schema ----------
SCHEMAS = {
    # 1) meta -------------------------------------------------------------
    "meta": {
        "schema_version": SCHEMA_VERSION,
        "title": "元信息 / 索引",
        "page_section": "页面顶部（标的、捕获时间）+ 本次输出的文件清单",
        "data_source": "两接口顶层字段 + 拆分清单",
        "fields": {
            "ticker": {"type": "string", "desc": "标的代码，如 SPX"},
            "spot": {"type": "number", "unit": "指数点", "desc": "标的现价"},
            "captured_at": {"type": "string", "desc": "数据捕获时间（美东 EST/EDT），由 timestamp 换算"},
            "generated_at": {"type": "string", "format": "ISO 8601 UTC", "desc": "接口生成本次响应的时间"},
            "last_updated_at": {"type": "string", "format": "ISO 8601 UTC", "desc": "后端缓存最后一次成功刷新的时间"},
            "stale": {"type": "boolean", "desc": "数据是否过期（true=后端长时间未刷新，需谨慎）"},
            "files": {
                "type": "object",
                "desc": "本次拆分输出的文件清单：键=文件相对路径，值={bytes=字节数, title=中文标题, source=数据来源}",
            },
        },
    },

    # 2) levels_summary ---------------------------------------------------
    "levels_summary": {
        "schema_version": SCHEMA_VERSION,
        "title": "关键价位概览",
        "page_section": "数据看板 → 关键价位概览",
        "data_source": "primary.levels.<ticker>",
        "fields": {
            "ticker": {"type": "string", "desc": "标的代码"},
            "spot": {"type": "number", "unit": "指数点", "desc": "现价"},
            "zero_gamma": {"type": "number", "unit": "指数点", "desc": "零 Gamma 价位：Gamma 暴露由正转负的翻转点，价格上穿/下穿时波动率放大特征会反转"},
            "mpos_vol": {"type": "number", "unit": "指数点", "desc": "按成交量口径的最大正 Gamma 行权价"},
            "mpos_oi": {"type": "number", "unit": "指数点", "desc": "按未平仓量(OI)口径的最大正 Gamma 行权价"},
            "mneg_vol": {"type": "number", "unit": "指数点", "desc": "按成交量口径的最大负 Gamma 行权价"},
            "mneg_oi": {"type": "number", "unit": "指数点", "desc": "按未平仓量(OI)口径的最大负 Gamma 行权价"},
            "net_gex_vol": {"type": "number", "unit": "Gamma 暴露", "desc": "按成交量口径汇总的净 GEX（正=做市商卖gamma/抑制波动，负=买gamma/放大波动）"},
            "net_gex_oi": {"type": "number", "unit": "Gamma 暴露", "desc": "按未平仓量口径汇总的净 GEX"},
            "timestamp": {"type": "integer", "unit": "Unix 秒", "desc": "数据捕获时间戳（EST/ET）"},
        },
    },

    # 3) key_levels/_meta -------------------------------------------------
    "key_levels_meta": {
        "schema_version": SCHEMA_VERSION,
        "title": "关键价位图 / 希腊值分布 — 公共元信息",
        "page_section": "数据看板→关键价位图 + 希腊值分布页（OI/GEX/DEX/VEX/CHEX 按钮那张图）",
        "data_source": "primary.exposure.<ticker>（去掉 metrics 后的包装字段）",
        "fields": {
            "symbol": {"type": "string", "desc": "标的代码"},
            "underlyingPrice": {"type": "number", "unit": "指数点", "desc": "标的现价"},
            "updatedAt": {"type": "string", "format": "ISO 8601", "desc": "本组 exposure 数据的更新时间"},
            "levels": {"type": "object", "desc": "关键价位（如 maxPain 最大痛点）；可能为空对象"},
            "rawCapabilities": {
                "type": "object",
                "desc": "本标的实际支持哪些指标/拆分（hasOi/hasGex/hasDex/hasVex/hasChex/hasMaxPain 等），用于判断某些字段是否可用",
            },
        },
    },

    # 4) key_levels/oi ----------------------------------------------------
    "key_levels_oi": {
        "schema_version": SCHEMA_VERSION,
        "title": "关键价位图 — OI（未平仓量）按行权价",
        "page_section": "数据看板→关键价位图 选 OI；希腊值分布页 OI",
        "data_source": "primary.exposure.<ticker>.metrics.oi",
        "fields": {},  # 根是数组，见 items
        "items": {
            "type": "object",
            "desc": "每个行权价一个点，对应图上一个柱",
            "fields": {
                "strike": {"type": "number", "unit": "指数点", "desc": "行权价"},
                "total": {"type": "number", "unit": "合约数(归一化)", "desc": "该行权价的净未平仓量（正=看涨方持仓多，负=看跌方持仓多）"},
            },
        },
    },

    # 5) key_levels/gex|dex|vex|chex （同构）------------------------------
    "key_levels_exposure_metric": {
        "schema_version": SCHEMA_VERSION,
        "title": "关键价位图 — {METRIC} 按行权价（带 0DTE/1DTE+/90天 三档）",
        "page_section": "数据看板→关键价位图 选 {METRIC}；希腊值分布页 {METRIC}（可切 DTE 模式）",
        "data_source": "primary.exposure.<ticker>.metrics.{metric}",
        "notes": _DTE_MODE_DESC,
        "metric_meaning": {
            "gex": "GEX = Gamma Exposure 伽马暴露",
            "dex": "DEX = Delta Exposure 德尔塔暴露",
            "vex": "VEX = Vanna Exposure 瓦纳暴露",
            "chex": "CHEX = Charm Exposure 查姆暴露",
        },
        "fields": {},
        "items": {
            "type": "object",
            "desc": "每个行权价一个点；zero/one/net 对应三种 DTE 模式，图上切按钮就是切这三条序列",
            "fields": {
                "strike": {"type": "number", "unit": "指数点", "desc": "行权价"},
                "zero": {"type": "number", "desc": "0DTE（当日到期）的 {METRIC} 值"},
                "one": {"type": "number", "desc": "1DTE+（次日及以后到期）的 {METRIC} 值"},
                "net": {"type": "number", "desc": "约 90 天累计的 {METRIC} 值"},
            },
        },
    },

    # 6) gamma_ladder -----------------------------------------------------
    "gamma_ladder": {
        "schema_version": SCHEMA_VERSION,
        "title": "前列 Gamma 行权价（Gamma 阶梯）",
        "page_section": "数据看板 → 前列 Gamma 行权价（正/负/绝对 三张表）",
        "data_source": "primary.gex_proxy.<ticker>",
        "fields": {
            "metrics": {
                "type": "object",
                "desc": "全标的 Gamma 汇总指标",
                "fields": {
                    "levels_count": {"type": "integer", "desc": "纳入计算的行权价数量"},
                    "positive_gamma": {"type": "number", "desc": "所有正 Gamma 行权价之和"},
                    "negative_gamma": {"type": "number", "desc": "所有负 Gamma 行权价之和（负数）"},
                    "net_gamma": {"type": "number", "desc": "净 Gamma = positive + negative"},
                    "absolute_gamma": {"type": "number", "desc": "绝对值之和 = |positive| + |negative|"},
                    "zero_gamma_proxy": {"type": "number", "unit": "指数点", "desc": "零 Gamma 价位（由阶梯估算）"},
                    "largest_positive_strike": {"type": "number", "unit": "指数点", "desc": "最大正 Gamma 行权价"},
                    "largest_negative_strike": {"type": "number", "unit": "指数点", "desc": "最大负 Gamma 行权价"},
                },
            },
        },
        "ladder_items": {
            "type": "object",
            "desc": "每个行权价一档，按行权价升序；页面'前列正/负/绝对 Gamma 行权价'表即由此排序取前 N",
            "fields": {
                "strike": {"type": "number", "unit": "指数点", "desc": "行权价"},
                "current_value": {"type": "number", "desc": "当前 Gamma 暴露值（净 GEX）"},
                "gamma": {"type": "number", "desc": "同 current_value（Gamma 主值）"},
                "abs_value": {"type": "number", "desc": "绝对值，用于'前列绝对 Gamma'排序"},
                "abs_gamma": {"type": "number", "desc": "同 abs_value"},
                "side": {"type": "string", "enum": ["positive", "negative", "neutral"], "desc": "方向：positive=正Gamma，negative=负Gamma"},
                "distance_from_spot": {"type": "number", "unit": "指数点", "desc": "距现价的点数（负=低于现价）"},
                "distance_percent": {"type": "number", "unit": "%", "desc": "距现价的百分比"},
                "lookback_values": {"type": "array<number>", "desc": "近若干次刷新的历史 Gamma 值"},
                "dte_values": {"type": "array<number>", "desc": "按 DTE 拆分的历史值（与 lookback 类似）"},
                "raw_row": {"type": "array", **_GAMMA_LADDER_TUPLE},
            },
        },
    },

    # 7) orderflow --------------------------------------------------------
    "orderflow": {
        "schema_version": SCHEMA_VERSION,
        "title": "订单流看板",
        "page_section": "数据看板 → 订单流看板（GAMMA价位 / 看涨看跌墙 / 订单流指标 / DEX分解）",
        "data_source": "primary.orderflow.<ticker>",
        "notes": "字段名前缀 z=0DTE，o=1DTE+；mlgamma/msgamma=主要多/空 Gamma；mcall/mput=主要看涨/看跌。CVR/GEX/Vanna/Charm/DEX 均为对应希腊值/订单流指标在该 DTE 下的汇总值。",
        "fields": {
            "ticker": {"type": "string", "desc": "标的代码"},
            "spot": {"type": "number", "unit": "指数点", "desc": "现价"},
            "timestamp": {"type": "integer", "unit": "Unix 秒", "desc": "捕获时间戳"},
            "z_mlgamma": {"type": "number", "unit": "指数点", "desc": "0DTE 主要多 Gamma 价位"},
            "z_msgamma": {"type": "number", "unit": "指数点", "desc": "0DTE 主要空 Gamma 价位"},
            "o_mlgamma": {"type": "number", "unit": "指数点", "desc": "1DTE+ 主要多 Gamma 价位"},
            "o_msgamma": {"type": "number", "unit": "指数点", "desc": "1DTE+ 主要空 Gamma 价位"},
            "zero_mcall": {"type": "number", "unit": "指数点", "desc": "0DTE 主要看涨墙价位"},
            "zero_mput": {"type": "number", "unit": "指数点", "desc": "0DTE 主要看跌墙价位"},
            "one_mcall": {"type": "number", "unit": "指数点", "desc": "1DTE+ 主要看涨墙价位"},
            "one_mput": {"type": "number", "unit": "指数点", "desc": "1DTE+ 主要看跌墙价位"},
            "zcvr": {"type": "number", "desc": "0DTE CVR 指标（看涨/看跌成交量比类指标）汇总值"},
            "ocvr": {"type": "number", "desc": "1DTE+ CVR 汇总值"},
            "zgr": {"type": "number", "desc": "0DTE GEX（Gamma 暴露）汇总值"},
            "ogr": {"type": "number", "desc": "1DTE+ GEX 汇总值"},
            "zvanna": {"type": "number", "desc": "0DTE Vanna 暴露汇总值"},
            "ovanna": {"type": "number", "desc": "1DTE+ Vanna 暴露汇总值"},
            "zcharm": {"type": "number", "desc": "0DTE Charm 暴露汇总值"},
            "ocharm": {"type": "number", "desc": "1DTE+ Charm 暴露汇总值"},
            "agg_dex": {"type": "number", "desc": "聚合 DEX（Delta 暴露，全部到期）"},
            "one_agg_dex": {"type": "number", "desc": "1DTE+ 聚合 DEX"},
            "agg_call_dex": {"type": "number", "desc": "聚合看涨 DEX"},
            "one_agg_call_dex": {"type": "number", "desc": "1DTE+ 聚合看涨 DEX"},
            "agg_put_dex": {"type": "number", "desc": "聚合看跌 DEX"},
            "one_agg_put_dex": {"type": "number", "desc": "1DTE+ 聚合看跌 DEX"},
            "net_dex": {"type": "number", "desc": "净 DEX（看涨-看跌）"},
            "one_net_dex": {"type": "number", "desc": "1DTE+ 净 DEX"},
            "net_call_dex": {"type": "number", "desc": "净看涨 DEX"},
            "one_net_call_dex": {"type": "number", "desc": "1DTE+ 净看涨 DEX"},
            "net_put_dex": {"type": "number", "desc": "净看跌 DEX"},
            "one_net_put_dex": {"type": "number", "desc": "1DTE+ 净看跌 DEX"},
            "dexoflow": {"type": "number", "desc": "DEX 的订单流加权值（推断）"},
            "gexoflow": {"type": "number", "desc": "GEX 的订单流加权值（推断）"},
            "cvroflow": {"type": "number", "desc": "CVR 的订单流加权值（推断）"},
            "one_dexoflow": {"type": "number", "desc": "1DTE+ DEX 订单流加权值（推断）"},
            "one_gexoflow": {"type": "number", "desc": "1DTE+ GEX 订单流加权值（推断）"},
            "one_cvroflow": {"type": "number", "desc": "1DTE+ CVR 订单流加权值（推断）"},
        },
    },

    # 8) classic_chain ----------------------------------------------------
    "classic_chain": {
        "schema_version": SCHEMA_VERSION,
        "title": "经典期权链摘要（GEX 视角）",
        "page_section": "支撑数据（关键价位/前列Gamma 的底层逐档数据）",
        "data_source": "primary.classic_chain.<ticker>",
        "fields": {
            "ticker": {"type": "string", "desc": "标的代码"},
            "timestamp": {"type": "integer", "unit": "Unix 秒", "desc": "捕获时间戳"},
            "min_dte": {"type": "integer", "unit": "天", "desc": "最近到期合约的 DTE"},
            "sec_min_dte": {"type": "integer", "unit": "天", "desc": "次近到期合约的 DTE"},
            "spot": {"type": "number", "unit": "指数点", "desc": "现价"},
            "zero_gamma": {"type": "number", "unit": "指数点", "desc": "零 Gamma 价位"},
            "major_pos_vol": {"type": "number", "unit": "指数点", "desc": "成交量口径最大正 Gamma 行权价"},
            "major_pos_oi": {"type": "number", "unit": "指数点", "desc": "OI 口径最大正 Gamma 行权价"},
            "major_neg_vol": {"type": "number", "unit": "指数点", "desc": "成交量口径最大负 Gamma 行权价"},
            "major_neg_oi": {"type": "number", "unit": "指数点", "desc": "OI 口径最大负 Gamma 行权价"},
            "sum_gex_vol": {"type": "number", "desc": "成交量口径 GEX 总和"},
            "sum_gex_oi": {"type": "number", "desc": "OI 口径 GEX 总和"},
            "delta_risk_reversal": {"type": "number", "desc": "Delta 风险逆转（call-put 偏度指标）"},
            "strikes": {"type": "array", "tuple_items": {
                "desc": "每个行权价一行（位置数组）",
                "positions": {
                    "0": {"name": "strike", "type": "number", "unit": "指数点", "desc": "行权价"},
                    "1": {"name": "gex_vol", "type": "number", "desc": "成交量口径 GEX"},
                    "2": {"name": "gex_oi", "type": "number", "desc": "未平仓量口径 GEX"},
                    "3": {"name": "lookback", "type": "array<number>", "desc": "近若干次刷新的历史值（5 点）"},
                },
            }},
            "max_priors": {"type": "array<array>", "desc": "历史极值记录 [行权价, 极值]，用于标注前期高/低 Gamma 位"},
        },
    },

    # 9) state_greeks -----------------------------------------------------
    "state_greeks": {
        "schema_version": SCHEMA_VERSION,
        "title": "状态希腊值",
        "page_section": "支撑数据（订单流看板'状态线'/主要价位的底层逐档数据）",
        "data_source": "primary.state_greeks.<ticker>",
        "fields": {
            "ticker": {"type": "string", "desc": "标的代码"},
            "timestamp": {"type": "integer", "unit": "Unix 秒", "desc": "捕获时间戳"},
            "spot": {"type": "number", "unit": "指数点", "desc": "现价"},
            "min_dte": {"type": "integer", "unit": "天", "desc": "最近到期 DTE"},
            "sec_min_dte": {"type": "integer", "unit": "天", "desc": "次近到期 DTE"},
            "major_positive": {"type": "number", "unit": "指数点", "desc": "主要正值价位"},
            "major_negative": {"type": "number", "unit": "指数点", "desc": "主要负值价位"},
            "major_long_gamma": {"type": "number", "unit": "指数点", "desc": "主要多 Gamma 价位"},
            "major_short_gamma": {"type": "number", "unit": "指数点", "desc": "主要空 Gamma 价位"},
            "mini_contracts": {"type": "array", "tuple_items": {
                "desc": "每个行权价一行（位置数组）",
                "positions": {
                    "0": {"name": "strike", "type": "number", "unit": "指数点", "desc": "行权价"},
                    "1": {"name": "call_component", "type": "number", "desc": "call 侧希腊值分量（推断）"},
                    "2": {"name": "put_component", "type": "number", "desc": "put 侧希腊值分量（推断）"},
                    "3": {"name": "value", "type": "number", "desc": "该行权价的希腊值主值"},
                    "4": {"name": "lookback", "type": "array<number>", "desc": "近若干次刷新的历史值（3 点）"},
                    "5": {"name": "reserved_0", "type": "number", "desc": "保留位，常为 0"},
                    "6": {"name": "reserved_null", "type": "null", "desc": "保留位，常为 null"},
                },
            }},
        },
    },

    # 10) dte_exposure/gex.* （classic_chain 形状，每 DTE 一个）-----------
    "dte_exposure_gex": {
        "schema_version": SCHEMA_VERSION,
        "title": "按 DTE 拆分的 GEX 期权链（每个 DTE 模式一份完整 classic_chain）",
        "page_section": "图表切 0DTE/1DTE+/90天 时，逐行权价的 GEX 底层数据",
        "data_source": "primary.gex_{mode}.<ticker>（mode = zero/one/net）",
        "notes": _DTE_MODE_DESC + " 每种模式结构与 classic_chain 一致，但 strikes 只含成交量口径（OI 位常为 0）。",
        "fields": {
            # 与 classic_chain 同构；此处描述 strikes 的差别
        },
        "inherit": "classic_chain",
        "strikes_override": {
            "type": "array", "tuple_items": {
                "desc": "每个行权价一行（位置数组；仅成交量口径）",
                "positions": {
                    "0": {"name": "strike", "type": "number", "unit": "指数点", "desc": "行权价"},
                    "1": {"name": "gex_vol", "type": "number", "desc": "该 DTE 模式下的 GEX（成交量口径）"},
                    "2": {"name": "gex_oi", "type": "number", "desc": "保留位，常为 0"},
                    "3": {"name": "lookback", "type": "array<number>", "desc": "近若干次刷新的历史值（5 点）"},
                },
            }
        },
    },

    # 11) dte_exposure/dex|vex|chex.* （state_greeks 形状，每 DTE 一个）--
    "dte_exposure_state": {
        "schema_version": SCHEMA_VERSION,
        "title": "按 DTE 拆分的 {METRIC} 状态希腊值（每个 DTE 模式一份完整 state_greeks）",
        "page_section": "图表切 0DTE/1DTE+/90天 时，逐行权价的 {METRIC} 底层数据",
        "data_source": "primary.{metric}_{mode}.<ticker>（metric=dex/vex/chex，mode=zero/one/net）",
        "notes": _DTE_MODE_DESC,
        "metric_meaning": {
            "dex": "DEX = Delta Exposure",
            "vex": "VEX = Vanna Exposure",
            "chex": "CHEX = Charm Exposure",
        },
        "inherit": "state_greeks",
        "value_meaning": "mini_contracts[][3] 即该 {METRIC} 在该 DTE 模式下、该行权价的主值",
    },
}


def render_schema(schema_id: str, metric: str | None = None) -> dict:
    """取一份 schema 的可序列化副本，必要时把 {METRIC}/{metric} 占位符替换。"""
    s = copy.deepcopy(SCHEMAS[schema_id])
    if metric:
        blob = json.dumps(s, ensure_ascii=False)
        blob = blob.replace("{METRIC}", metric.upper()).replace("{metric}", metric)
        s = json.loads(blob)
    return s
