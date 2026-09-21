//! raw payload / CBOE → `thc_engine::EngineInput` 的映射层。
//! engine 不感知 TradingHub/CBOE 字段命名；数据源特有逻辑止步于此。

use crate::crawl::cboe::{CboeOption, MarketData};
use crate::crawl::payload::TickerView;
use jiff::civil::Date;
use thc_engine::*;

/// TradingHub 标的视图 → OptionStructure（对应 legacy options_layer.extract_options）。
pub fn options_structure(view: &TickerView) -> OptionStructure {
    let mut s = OptionStructure::default();
    let lv = view.levels_summary.as_ref();
    let of = view.orderflow.as_ref();
    s.spot = lv.map(|l| l.spot).or(view.spot);
    s.flip = lv.and_then(|l| l.zero_gamma);
    s.net_gex_vol = lv.and_then(|l| l.net_gex_vol);
    s.net_gex_oi = lv.and_then(|l| l.net_gex_oi);
    if let Some(of) = of {
        s.call_wall_0dte = of.zero_mcall;
        s.call_wall_1dte = of.one_mcall;
        s.put_wall_0dte = of.zero_mput;
        s.put_wall_1dte = of.one_mput;
        s.major_long_gamma = of.z_mlgamma;
        s.major_short_gamma = of.z_msgamma;
        s.flow = FlowState {
            zcvr: of.zcvr,
            ocvr: of.ocvr,
            zgr: of.zgr,
            ogr: of.ogr,
            net_gex_vol: s.net_gex_vol,
        };
        s.orderflow = OrderflowMetrics {
            dex_0dte: of.agg_dex,
            call_dex_0dte: of.agg_call_dex,
            put_dex_0dte: of.agg_put_dex,
            cvr_0dte: of.zcvr,
            gex_ratio_0dte: of.zgr,
            vanna_0dte: of.zvanna,
            charm_0dte: of.zcharm,
            dex_1dte: of.one_agg_dex,
            call_dex_1dte: of.one_agg_call_dex,
            put_dex_1dte: of.one_agg_put_dex,
            cvr_1dte: of.ocvr,
            gex_ratio_1dte: of.ogr,
            vanna_1dte: of.ovanna,
            charm_1dte: of.ocharm,
            net_dex: of.net_dex,
            net_call_dex: of.net_call_dex,
            net_put_dex: of.net_put_dex,
            dexoflow: of.dexoflow,
            gexoflow: of.gexoflow,
            cvroflow: of.cvroflow,
        };
    }
    s.max_pos_oi = lv.and_then(|l| l.mpos_oi);
    s.max_neg_oi = lv.and_then(|l| l.mneg_oi);
    s.max_pos_vol = lv.and_then(|l| l.mpos_vol);
    s.max_neg_vol = lv.and_then(|l| l.mneg_vol);

    if let Some(gl) = &view.gamma_ladder {
        s.gamma_ladder = gl
            .ladder
            .iter()
            .map(|r| LadderPoint {
                strike: r.strike,
                current_value: r.current_value,
                side: r.side.clone(),
            })
            .collect();
    }
    if let Some(ex) = &view.exposure {
        for row in &ex.metrics.gex {
            let net = row.extra.get("net").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if net < 0.0 {
                s.neg_gamma_strikes.push(row.strike);
            } else if net > 0.0 {
                s.pos_gamma_strikes.push(row.strike);
            }
        }
        s.neg_gamma_strikes
            .sort_by(|a, b| a.partial_cmp(b).unwrap());
        s.pos_gamma_strikes
            .sort_by(|a, b| a.partial_cmp(b).unwrap());
        // 去重
        s.neg_gamma_strikes.dedup();
        s.pos_gamma_strikes.dedup();
    }
    s
}

/// CBOE MarketData → VolatilityData（VIX 家族值透传）。
pub fn volatility(md: &MarketData) -> VolatilityData {
    VolatilityData {
        vix_family: md
            .vix_family
            .iter()
            .map(|(k, v)| (k.clone(), v.value))
            .collect(),
        events: vec![],
    }
}

/// CBOE EM 信息 → engine 的 EmEstimate。
pub fn em_estimate(em: &crate::crawl::cboe::EmInfo) -> EmEstimate {
    EmEstimate {
        em_0dte_spx: em.em_0dte_spx,
        method: em.method.clone(),
        source_expiry: Some(em.source_expiry.clone()),
        source_straddle: Some(em.source_straddle),
    }
}

/// CBOE 链 → engine 的 ChainContract。
pub fn chain_contracts(opts: &[CboeOption]) -> Vec<ChainContract> {
    opts.iter()
        .map(|o| ChainContract {
            expiry: o.expiry,
            is_call: o.is_call,
            strike: o.strike,
            bid: o.bid,
            ask: o.ask,
            iv: o.iv,
            open_interest: o.open_interest,
            volume: o.volume,
        })
        .collect()
}

/// 技术位覆盖 → Technicals。
#[allow(clippy::too_many_arguments)]
pub fn technicals(
    vwap: Option<f64>,
    poc: Option<f64>,
    pdh: Option<f64>,
    pdl: Option<f64>,
    onh: Option<f64>,
    onl: Option<f64>,
    prior_pivot: Option<f64>,
    realized_range: Option<f64>,
) -> Technicals {
    Technicals {
        vwap,
        poc,
        pdh,
        pdl,
        onh,
        onl,
        prior_pivot,
        realized_range,
    }
}

/// TradingHub 内嵌 basis = ES_SPX.spot − SPX.spot（透明审计用）。
pub fn embedded_basis(es: &TickerView, spx: Option<&TickerView>) -> Option<f64> {
    let a = es.spot;
    let b = spx.and_then(|v| v.spot);
    match (a, b) {
        (Some(a), Some(b)) => Some(((a - b) * 10000.0).round() / 10000.0),
        _ => None,
    }
}

/// 当前 ET 日期。
pub fn et_today() -> Date {
    jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::get("America/New_York").expect("ET tz"))
        .date()
}
