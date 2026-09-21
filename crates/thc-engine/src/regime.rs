//! 波动状态分类：CHOPPING / COMPRESSION / NORMAL / EXPANSION（文档 §10）。
//! 1:1 移植自 regime.py（含 VIX1D<10 先验，文档 §5.5 方法C）。

use crate::EngineConfig;
use crate::types::Regime;
use std::collections::BTreeMap;

pub struct RegimeOut {
    pub regime: Regime,
    pub prob: BTreeMap<String, f64>,
    pub notes: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn classify_regime(
    net_gex: Option<f64>,
    price: Option<f64>,
    pivot: Option<f64>,
    buffer: f64,
    call_wall: Option<f64>,
    put_wall: Option<f64>,
    em: Option<f64>,
    realized_range: Option<f64>,
    vix1d: Option<f64>,
    cfg: &EngineConfig,
) -> RegimeOut {
    let mut notes: Vec<String> = Vec::new();
    let mut prob = BTreeMap::from([
        ("range_mean_revert".to_string(), 0.5),
        ("trend_expansion".to_string(), 0.5),
    ]);

    // 1) GEX 主导（始终可用）
    let between_walls = match (call_wall, put_wall, price) {
        (Some(cw), Some(pw), Some(p)) => pw < p && p < cw,
        _ => false,
    };
    if net_gex.is_some_and(|g| g > 0.0) && between_walls {
        prob.insert("range_mean_revert".into(), 0.65);
        prob.insert("trend_expansion".into(), 0.35);
        notes.push("净GEX>0 且价格在墙间 → 偏均值回归".into());
    }
    if net_gex.is_some_and(|g| g < 0.0) || (price.is_some() && pivot.is_some() && price < pivot) {
        prob.insert("trend_expansion".into(), 0.6);
        prob.insert("range_mean_revert".into(), 0.4);
        notes.push("净GEX<0 或价格在 Flip 下方 → 偏波动扩张".into());
    }

    // 2) realized / EM（有 em 才启用）
    let mut regime = Regime::Normal;
    if let (Some(em), Some(rr)) = (em, realized_range) {
        if pivot.is_some() && price.is_some() && (price.unwrap() - pivot.unwrap()).abs() <= buffer {
            regime = Regime::Chopping;
            notes.push("价格贴 pivot → CHOPPING".into());
        } else if rr < 0.60 * em {
            regime = Regime::Compression;
            notes.push(format!("实际波幅 {rr:.1} < 0.6×EM({em:.1}) → COMPRESSION"));
        } else if rr > 1.20 * em {
            regime = Regime::Expansion;
            notes.push(format!("实际波幅 {rr:.1} > 1.2×EM({em:.1}) → EXPANSION"));
        }
    } else {
        // 无 EM：用 GEX 符号给粗 regime
        if net_gex.is_some_and(|g| g < 0.0) {
            regime = Regime::Expansion;
        } else if net_gex.is_some_and(|g| g > 0.0) {
            regime = Regime::Chopping;
        }
        notes.push("无 EM/realized → 仅用 GEX 粗判 regime".into());
    }

    // 3) VIX1D 极低位规则（文档 §5.5 方法C）：vix1d<阈值 上调 EXPANSION 先验
    if vix1d.is_some_and(|v| v < cfg.vix1d_low_threshold) {
        let v: f64 = prob["trend_expansion"];
        prob.insert("trend_expansion".into(), (v + 0.1).min(0.75));
        notes.push(format!(
            "VIX1D={}<{} 极低位 → 短端IV无压缩空间，上调扩张先验",
            vix1d.unwrap(),
            cfg.vix1d_low_threshold
        ));
    }

    RegimeOut {
        regime,
        prob,
        notes,
    }
}
