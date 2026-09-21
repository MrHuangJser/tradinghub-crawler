//! Expected Move（文档 §5.5）。
//! 优先级：当日真 0DTE ATM straddle > 最近到期 √T 近似 > 调用方注入。
//! VIX 年化换算不得冒充 0DTE 定价（文档 §5.5 方法C 实证）。

use crate::types::{ChainContract, EmEstimate};
use jiff::civil::Date;
use std::collections::BTreeMap;

/// 从期权链直接推导 EM：有当日到期则用真 0DTE straddle，否则最近到期 √T。
pub fn from_chain(chain: &[ChainContract], spot: f64, as_of: Date) -> Option<EmEstimate> {
    let expiry = chain
        .iter()
        .filter(|c| c.expiry >= as_of)
        .map(|c| c.expiry)
        .min()?;
    let dte = as_of
        .until(expiry)
        .map(|s| s.get_days())
        .unwrap_or(0)
        .max(0);

    // 按 strike 收集该到期日的 (call_mid, put_mid, call_iv)
    type StrikeEntry = (Option<f64>, Option<f64>, Option<f64>);
    let mut by_strike: BTreeMap<i64, StrikeEntry> = BTreeMap::new();
    let key = |s: f64| (s * 1000.0).round() as i64;
    for c in chain.iter().filter(|c| c.expiry == expiry) {
        let (Some(b), Some(a)) = (c.bid, c.ask) else {
            continue;
        };
        if b <= 0.0 || a <= 0.0 {
            continue;
        }
        let e = by_strike.entry(key(c.strike)).or_default();
        if c.is_call {
            e.0 = Some((b + a) / 2.0);
            e.2 = c.iv;
        } else {
            e.1 = Some((b + a) / 2.0);
        }
    }
    // ATM：离 spot 最近的双边有效执行价
    let best = by_strike
        .iter()
        .filter(|(_, (c, p, _))| c.is_some() && p.is_some())
        .min_by(|(ka, _), (kb, _)| {
            let da = (**ka as f64 / 1000.0 - spot).abs();
            let db = (**kb as f64 / 1000.0 - spot).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let (_k, (c_mid, p_mid, _iv)) = best;
    let straddle = c_mid.unwrap() + p_mid.unwrap();

    if dte == 0 {
        // 真 0DTE：straddle 即 EM
        Some(EmEstimate {
            em_0dte_spx: (straddle * 100.0).round() / 100.0,
            method: "0DTE ATM straddle".into(),
            source_expiry: Some(expiry.to_string()),
            source_straddle: Some((straddle * 100.0).round() / 100.0),
        })
    } else {
        Some(EmEstimate {
            em_0dte_spx: (straddle / (dte as f64).sqrt() * 100.0).round() / 100.0,
            method: format!("monthly ATM straddle / √DTE (DTE={dte})"),
            source_expiry: Some(expiry.to_string()),
            source_straddle: Some((straddle * 100.0).round() / 100.0),
        })
    }
}
