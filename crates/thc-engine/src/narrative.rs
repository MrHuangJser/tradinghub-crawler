//! 条件式盘前文案（文档 §11）。纯模板拼接，不调 LLM。
//! 只输出条件式结构判断；不生成"先多后空"类路径剧本（文档 §20.4 证实不可靠）。
//! 1:1 移植自 narrative.py。

use crate::types::{Bias, Plan, Regime};

pub fn gen_narrative(plan: &Plan) -> Vec<String> {
    let mut txt: Vec<String> = Vec::new();
    let (price, pivot) = (plan.spot, plan.pivot.as_ref().map(|p| p.level));

    // 方向 × 波动 组合（文档 §10 表）
    match (plan.bias, plan.regime) {
        (Some(Bias::Bullish), Regime::Chopping | Regime::Compression) => {
            txt.push("短线偏多，但更偏震荡/压缩，不宜在近端目标前追价，等回落更优。".into())
        }
        (Some(Bias::Bearish), Regime::Chopping | Regime::Compression) => {
            txt.push("短线偏空，但更偏震荡/压缩，优先等反弹确认后再动作。".into())
        }
        (Some(Bias::Bullish), Regime::Expansion) => {
            txt.push("偏多且波动扩张，突破回踩获接受后顺势做多，VWAP 上方只多不空。".into())
        }
        (Some(Bias::Bearish), Regime::Expansion) => {
            txt.push("偏空且波动扩张，跌破确认后顺势做空，不左侧摸顶。".into())
        }
        (Some(Bias::Neutral), _) => txt.push("方向中性，区间边缘交易、中部不交易。".into()),
        _ => {}
    }

    if let Some(p) = pivot {
        if price > p
            && let Some(t1) = plan.bull_targets.first()
        {
            txt.push(format!(
                "价格在转换位 {p} 上方，上方先看 {}（{}）。",
                t1.level,
                t1.types.join(", ")
            ));
        } else if price < p
            && let Some(t1) = plan.bear_targets.first()
        {
            txt.push(format!(
                "价格在转换位 {p} 下方，下方先看 {}（{}）。",
                t1.level,
                t1.types.join(", ")
            ));
        }
    }

    if let Some(sup) = &plan.major_long_support {
        txt.push(format!(
            "若回撤至核心防守 {}（{}）出现承接，该区域多头风险收益比较好；有效失守则取消逢低做多。",
            sup.level,
            sup.types.join(", ")
        ));
    }
    if let Some(sq) = &plan.squeeze_zone {
        let rationale = if sq.rationale.is_empty() {
            "波动加速区".to_string()
        } else {
            sq.rationale.join("、")
        };
        txt.push(format!(
            "若核心防守有效失守，下方 {} 是潜在{rationale}。",
            sq.level
        ));
    }

    if let Some(em) = &plan.em {
        txt.push(format!(
            "0DTE Expected Move ≈ ±{:?} 点（用于目标可达性与止损距离的尺度，非保证）。",
            em.em_0dte_spx
        ));
    } else {
        txt.push("未提供 EM，目标可达性与止损距离未评估（纯结构模式）。".into());
    }

    txt.push("期权位是地图，价格行为是触发器；以上为结构判断，非交易信号。".into());
    txt
}
