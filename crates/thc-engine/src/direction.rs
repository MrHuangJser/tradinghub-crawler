//! 方向状态机：多空转换位 pivot + bias（文档 §9.1）。1:1 移植自 direction.py。

use crate::EngineConfig;
use crate::types::Bias;

/// 转换位 = 可用候选的中位数。无任何技术位时退化为 Gamma Flip。
/// 文档 2026-08-04 实证：真 pivot 更偏 ON 中轴/VWAP，故有技术位时以中位数融合。
pub fn choose_pivot(
    flip: Option<f64>,
    vwap: Option<f64>,
    on_mid: Option<f64>,
    poc: Option<f64>,
    prior_pivot: Option<f64>,
) -> (Option<f64>, Vec<String>) {
    let cands = [
        ("flip", flip),
        ("on_mid", on_mid),
        ("vwap", vwap),
        ("poc", poc),
        ("prior_pivot", prior_pivot),
    ];
    let present: Vec<(&str, f64)> = cands
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    if present.is_empty() {
        return (None, vec![]);
    }
    let mut vals: Vec<f64> = present.iter().map(|(_, v)| *v).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = vals[vals.len() / 2];
    // 若只有 flip，pivot=flip；否则用中位数（融合技术结构）
    let pivot = if present.len() == 1 && present[0].0 == "flip" {
        present[0].1
    } else {
        med
    };
    (
        Some((pivot * 100.0).round() / 100.0),
        present.into_iter().map(|(k, _)| k.to_string()).collect(),
    )
}

/// bias = price 相对 pivot±buffer。buffer = max(2.0, coef×EM_total)（文档 §9.1）。
/// `em_total` 传入的是 em*2（全幅口径，沿用 Python 行为）。
pub fn classify_bias(
    price: Option<f64>,
    pivot: Option<f64>,
    em_total: Option<f64>,
    cfg: &EngineConfig,
) -> (Option<Bias>, f64) {
    let (Some(price), Some(pivot)) = (price, pivot) else {
        return (None, 0.0);
    };
    let buffer = match em_total {
        Some(t) => (cfg.pivot_buffer_em_coef * t).max(2.0),
        None => 2.0,
    };
    let buffer = (buffer * 100.0).round() / 100.0;
    let bias = if price > pivot + buffer {
        Bias::Bullish
    } else if price < pivot - buffer {
        Bias::Bearish
    } else {
        Bias::Neutral
    };
    (Some(bias), buffer)
}
