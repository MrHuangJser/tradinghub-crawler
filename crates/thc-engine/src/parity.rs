//! Put-Call Parity 合成远期（文档 §4.2）。
//!
//! 对同执行价、同到期日的 Call/Put 用中间价反推合成远期：
//!   F_syn,i ≈ K_i + C_mid,i − P_mid,i   （0DTE 近似）
//! 生产口径：ATM 附近多组配对 → 中位数 + 稳健离散度（MAD）。
//! 离散超限 → 链的 EM/Skew/GEX 衍生计算不可信（标记 rejected）。

use crate::types::{ChainContract, ParityInfo};
use jiff::civil::Date;
use std::collections::BTreeMap;

/// 计算合成远期。
/// - `expiry`：使用最近到期日（0DTE 近似最合理的期限）
/// - `atm_window`：只取 |strike − spot| ≤ window 的配对
/// - `max_spread_frac`：配对有效门槛 ask−bid ≤ frac × mid（过滤错配报价）
/// - `dispersion_tol`：F_syn 的 MAD 容忍度（SPX 点），超限 → rejected
pub fn synthetic_forward(
    chain: &[ChainContract],
    spot: f64,
    as_of: Date,
    atm_window: f64,
    max_spread_frac: f64,
    dispersion_tol: f64,
) -> Option<ParityInfo> {
    // 最近到期日（≥as_of）
    let expiry = chain
        .iter()
        .filter(|c| c.expiry >= as_of)
        .map(|c| c.expiry)
        .min()?;

    // 按 strike 配对 call/put
    let mut by_strike: BTreeMap<i64, (Option<&ChainContract>, Option<&ChainContract>)> =
        BTreeMap::new();
    let key = |s: f64| (s * 1000.0).round() as i64;
    for c in chain.iter().filter(|c| c.expiry == expiry) {
        let e = by_strike.entry(key(c.strike)).or_default();
        if c.is_call {
            e.0 = Some(c);
        } else {
            e.1 = Some(c);
        }
    }

    let mut fs: Vec<f64> = Vec::new();
    for (k, (call, put)) in &by_strike {
        let (Some(c), Some(p)) = (call, put) else {
            continue;
        };
        let strike = *k as f64 / 1000.0;
        if (strike - spot).abs() > atm_window {
            continue;
        }
        let (Some(cb), Some(ca), Some(pb), Some(pa)) = (c.bid, c.ask, p.bid, p.ask) else {
            continue;
        };
        if cb <= 0.0 || ca <= 0.0 || pb <= 0.0 || pa <= 0.0 {
            continue;
        }
        let c_mid = (cb + ca) / 2.0;
        let p_mid = (pb + pa) / 2.0;
        // 报价交叉或价差过大 → 该配对不可信
        if cb > ca || pb > pa {
            continue;
        }
        let mid = (c_mid + p_mid) / 2.0;
        if mid > 0.0 && (ca - cb + pa - pb) / (2.0 * mid) > max_spread_frac {
            continue;
        }
        fs.push(strike + c_mid - p_mid);
    }
    if fs.is_empty() {
        return None;
    }
    fs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = fs[fs.len() / 2];
    // 离散度用 p90−p10：MAD/IQR 在小样本"多数一致+个别错配"时会塌缩漏检；
    // p90−p10 排除单点极端尾部，又能捕获系统性离散（文档 §4.2 要求拒绝离散链）。
    let q = |p: f64| {
        let i = ((fs.len() - 1) as f64 * p).round() as usize;
        fs[i.min(fs.len() - 1)]
    };
    let dispersion = q(0.9) - q(0.1);
    Some(ParityInfo {
        synthetic_forward: median,
        pairs: fs.len(),
        dispersion,
        rejected: dispersion > dispersion_tol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(strike: f64, is_call: bool, bid: f64, ask: f64) -> ChainContract {
        ChainContract {
            expiry: Date::new(2026, 9, 21).unwrap(),
            is_call,
            strike,
            bid: Some(bid),
            ask: Some(ask),
            iv: None,
            open_interest: None,
            volume: None,
        }
    }

    #[test]
    fn parity_recovers_forward() {
        // F=7700：K + C − P ⇒ 7700 执行价 C−P≈0；每档都该给回 ~7700
        let day = Date::new(2026, 9, 21).unwrap();
        let chain = vec![
            mk(7650.0, true, 52.0, 53.0),
            mk(7650.0, false, 2.0, 3.0),
            mk(7700.0, true, 25.0, 26.0),
            mk(7700.0, false, 25.0, 26.0),
            mk(7750.0, true, 2.0, 3.0),
            mk(7750.0, false, 52.0, 53.0),
        ];
        let info = synthetic_forward(&chain, 7700.0, day, 60.0, 0.5, 2.0).unwrap();
        assert!((info.synthetic_forward - 7700.0).abs() < 1.0);
        assert_eq!(info.pairs, 3);
        assert!(!info.rejected);
    }

    #[test]
    fn parity_rejects_dispersed() {
        let day = Date::new(2026, 9, 21).unwrap();
        let chain = vec![
            mk(7650.0, true, 52.0, 53.0),
            mk(7650.0, false, 2.0, 3.0),
            // 错配：7700 put 报价离谱
            mk(7700.0, true, 25.0, 26.0),
            mk(7700.0, false, 60.0, 62.0),
            mk(7750.0, true, 2.0, 3.0),
            mk(7750.0, false, 52.0, 53.0),
        ];
        let info = synthetic_forward(&chain, 7700.0, day, 60.0, 0.9, 2.0).unwrap();
        assert!(
            info.rejected,
            "dispersion {} should reject",
            info.dispersion
        );
    }
}
