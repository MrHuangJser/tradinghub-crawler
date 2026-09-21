//! thc-engine —— SPX 期权驱动的 ES 盘前分析引擎。
//!
//! 纯计算 crate：**禁止依赖 reqwest/tokio 或读取系统时钟**；
//! 时间与数据一律经 `EngineInput` 显式注入（见 `types.rs`）。
//! 算法依据：`docs/SPX期权驱动的ES盘前分析算法-逆向重建.md`（v1.1）。
//! 1:1 移植自 legacy/es_engine（rth_recalibrate 按计划不移植）。

mod direction;
mod levels;
mod narrative;
mod regime;
mod types;

pub use types::*;

use jiff::{Timestamp, tz::TimeZone};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("数据血缘审计拒绝: {0}")]
    LineageRejected(String),
    #[error("Put-Call Parity 离散度超限: {dispersion:.2} > {tolerance:.2}")]
    ParityDispersion { dispersion: f64, tolerance: f64 },
    #[error("输入数据不足: {0}")]
    InsufficientData(String),
}

/// 引擎可调参数（文档要求"必须通过历史样本验证"的量全部配置化，
/// 默认值 = 文档 v1.1 值；见 `config.toml` [engine]）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineConfig {
    /// 价位评分权重（文档 §8）。
    pub weight_options_strength: f64,
    pub weight_technical_confluence: f64,
    pub weight_proximity: f64,
    pub weight_expected_move_alignment: f64,
    pub weight_historical_reaction: f64,
    /// 聚类容差系数：`max(1.0, coef * EM)`（文档 §7.2）。
    pub cluster_tolerance_em_coef: f64,
    /// pivot 缓冲：`max(2.0, coef * EM_total)`（文档 §9.1）。
    pub pivot_buffer_em_coef: f64,
    /// VIX1D 极端低位阈值（文档 §5.5 方法C）。
    pub vix1d_low_threshold: f64,
    /// EM 目标可达性折扣（止损判断不折扣）。
    pub em_reachability_discount: f64,
    /// Flip 失效阈值（SPX 点，文档 §5.4/20.3）。
    pub flip_invalidation_spx: f64,
    /// basis 同步窗口（秒，文档 §6.1）。
    pub basis_sync_max_skew_secs: i64,
    /// 快照时效门禁：同日新鲜阈值（分钟）
    pub freshness_minutes: i64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            weight_options_strength: 0.30,
            weight_technical_confluence: 0.25,
            weight_proximity: 0.20,
            weight_expected_move_alignment: 0.15,
            weight_historical_reaction: 0.10,
            cluster_tolerance_em_coef: 0.02,
            pivot_buffer_em_coef: 0.05,
            vix1d_low_threshold: 10.0,
            em_reachability_discount: 0.87,
            flip_invalidation_spx: 10.0,
            basis_sync_max_skew_secs: 60,
            freshness_minutes: 15,
        }
    }
}

// ---------------------------------------------------------------------------
// 时效/血缘门禁（freshness.py 移植 + 文档 §4.1 四态合并）
// ---------------------------------------------------------------------------

fn et_zoned(ts: i64) -> jiff::Zoned {
    Timestamp::from_second(ts)
        .expect("ts")
        .to_zoned(TimeZone::get("America/New_York").expect("ET tz"))
}

/// 评估快照时效（文档 §4.1 + freshness.py 语义合并）。
/// 返回 (状态, 说明, 会话标记)。
pub fn assess_lineage(
    snapshot_ts: Option<i64>,
    now_ts: i64,
    cfg: &EngineConfig,
) -> (LineageStatus, String, &'static str) {
    let Some(snap) = snapshot_ts else {
        return (
            LineageStatus::Unknown,
            "无 timestamp，无法判断时效".into(),
            "OFF",
        );
    };
    let snap_dt = et_zoned(snap);
    let now_dt = et_zoned(now_ts);
    let staleness_min = (now_ts - snap) as f64 / 60.0;
    let same_day = snap_dt.date() == now_dt.date();
    // 周一~周五（未处理美股假日）
    let is_trading_day = matches!(
        now_dt.date().weekday(),
        jiff::civil::Weekday::Monday
            | jiff::civil::Weekday::Tuesday
            | jiff::civil::Weekday::Wednesday
            | jiff::civil::Weekday::Thursday
            | jiff::civil::Weekday::Friday
    );
    let t = now_dt.time();
    let now_min = t.hour() as i64 * 60 + t.minute() as i64;
    let in_rth = is_trading_day && (570..960).contains(&now_min); // 09:30~16:00 ET
    let pre_market = is_trading_day && now_min < 570;
    let session = if in_rth {
        "RTH"
    } else if pre_market {
        "PRE"
    } else {
        "OFF"
    };

    if same_day && staleness_min <= cfg.freshness_minutes as f64 {
        (
            LineageStatus::Fresh,
            format!("今日数据，约 {staleness_min:.0} 分钟前"),
            session,
        )
    } else if same_day {
        (
            LineageStatus::StaleToday,
            format!("今日数据但已滞后 {staleness_min:.0} 分钟（TradingHub 可能未刷新）"),
            session,
        )
    } else if pre_market {
        (
            LineageStatus::PriorCloseOk,
            format!("昨结数据（{} ET），盘前可用", snap_dt.date()),
            session,
        )
    } else {
        let phase = if in_rth { "盘中" } else { "当前" };
        (
            LineageStatus::StalePriorDay,
            format!(
                "⚠️ 数据为 {} ET（非今日），{phase}慎用——flip 可能偏数十点",
                snap_dt.date()
            ),
            session,
        )
    }
}

// ---------------------------------------------------------------------------
// 引擎编排（文档 §18 build_premarket_plan 的 1:1 移植）
// ---------------------------------------------------------------------------

/// 引擎入口：纯函数 `EngineInput → Plan`。
pub fn build_plan(input: &EngineInput, cfg: &EngineConfig) -> Result<Plan, EngineError> {
    let opt = &input.options;
    let price = opt
        .spot
        .ok_or_else(|| EngineError::InsufficientData("无现价".into()))?;

    let em = input.em.as_ref().map(|e| e.em_0dte_spx);
    let vix = input
        .vix_override
        .or_else(|| input.volatility.vix_family.get("VIX").copied().flatten());
    let vix1d = input
        .vix1d_override
        .or_else(|| input.volatility.vix_family.get("VIX1D").copied().flatten());

    let tech = &input.technicals;
    let on_mid = match (tech.onh, tech.onl) {
        (Some(h), Some(l)) => Some(((h + l) / 2.0 * 100.0).round() / 100.0),
        _ => None,
    };

    // 转换位 + bias
    let (pivot_level, pivot_sources) =
        direction::choose_pivot(opt.flip, tech.vwap, on_mid, tech.poc, tech.prior_pivot);
    let (bias, buffer) =
        direction::classify_bias(Some(price), pivot_level, em.map(|e| e * 2.0), cfg);
    let pivot = pivot_level.map(|level| Pivot {
        level,
        buffer,
        sources: pivot_sources,
    });

    // regime
    let regime_out = regime::classify_regime(
        opt.net_gex_vol,
        Some(price),
        pivot_level,
        buffer,
        opt.call_wall_0dte,
        opt.put_wall_0dte,
        em,
        tech.realized_range,
        vix1d,
        cfg,
    );

    // 价位
    let lv = levels::build_levels(opt, price, em, tech, cfg);

    // 血缘/时效
    let (lineage, lineage_msg, session) = assess_lineage(input.captured_ts, input.now_ts, cfg);
    let freshness = input.captured_ts.map(|snap| {
        let staleness_min = (input.now_ts - snap) as f64 / 60.0;
        serde_json::json!({
            "status": lineage,
            "staleness_min": (staleness_min * 10.0).round() / 10.0,
            "snapshot_et": et_zoned(snap).strftime("%Y-%m-%d %H:%M %Z").to_string(),
            "now_et": et_zoned(input.now_ts).strftime("%Y-%m-%d %H:%M %Z").to_string(),
            "session": session,
            "message": lineage_msg,
        })
    });

    let mut limitations: Vec<String> = Vec::new();
    if em.is_none() {
        limitations
            .push("未提供 EM：buffer 固定 2 点、无目标可达性/止损尺度、regime 仅 GEX 粗判".into());
    }
    if ![tech.vwap, tech.poc, tech.pdh, tech.pdl, tech.onh, tech.onl]
        .iter()
        .any(|v| v.is_some())
    {
        limitations
            .push("未提供技术位(VWAP/POC/PDH/ON)：pivot 退化为 Gamma Flip、无技术共振评分".into());
    }
    if on_mid.is_none() {
        limitations.push("未提供 ONH/ONL：转换位未纳入隔夜中轴(文档实证的重要候选)".into());
    }
    if matches!(
        lineage,
        LineageStatus::StalePriorDay | LineageStatus::StaleToday
    ) {
        limitations.push(format!("数据时效：{lineage_msg}"));
    }

    // Gamma / Delta 结构详情（报告渲染用）
    let pos_near: Vec<f64> = opt
        .pos_gamma_strikes
        .iter()
        .cloned()
        .filter(|s| price - em.unwrap_or(25.0) <= *s && *s <= price + em.unwrap_or(25.0))
        .collect();
    let top_pos: Vec<serde_json::Value> = levels::top_gamma_strikes(opt, 5, Some("positive"))
        .iter()
        .map(|r| serde_json::json!({"strike": r.strike, "gex": r1(r.current_value)}))
        .collect();
    let top_neg: Vec<serde_json::Value> = levels::top_gamma_strikes(opt, 5, Some("negative"))
        .iter()
        .map(|r| serde_json::json!({"strike": r.strike, "gex": r1(r.current_value)}))
        .collect();
    let of = &opt.orderflow;
    let gamma_detail = serde_json::json!({
        "top_positive": top_pos,
        "top_negative": top_neg,
        "call_walls": {"0DTE": opt.call_wall_0dte, "1DTE+": opt.call_wall_1dte},
        "put_walls": {"0DTE": opt.put_wall_0dte, "1DTE+": opt.put_wall_1dte},
        "major_long_gamma": opt.major_long_gamma,
        "major_short_gamma": opt.major_short_gamma,
        "pos_gamma_near_spot": pos_near.len(),
        "neg_gamma_near_spot": lv.negative_gamma_strikes_near_spot.len(),
        "total_gamma_ladder_entries": opt.gamma_ladder.len(),
    });
    let delta_detail = serde_json::json!({
        "dex_0dte": r1(of.dex_0dte), "call_dex_0dte": r1(of.call_dex_0dte),
        "put_dex_0dte": r1(of.put_dex_0dte), "cvr_0dte": r1(of.cvr_0dte),
        "gex_ratio_0dte": r1(of.gex_ratio_0dte), "vanna_0dte": r1(of.vanna_0dte),
        "charm_0dte": r1(of.charm_0dte),
        "dex_1dte": r1(of.dex_1dte), "call_dex_1dte": r1(of.call_dex_1dte),
        "put_dex_1dte": r1(of.put_dex_1dte), "cvr_1dte": r1(of.cvr_1dte),
        "gex_ratio_1dte": r1(of.gex_ratio_1dte), "vanna_1dte": r1(of.vanna_1dte),
        "charm_1dte": r1(of.charm_1dte),
        "net_dex": r1(of.net_dex), "net_call_dex": r1(of.net_call_dex),
        "net_put_dex": r1(of.net_put_dex),
        "dexoflow": r1(of.dexoflow), "gexoflow": r1(of.gexoflow), "cvroflow": r1(of.cvroflow),
        "max_pos_oi_strike": opt.max_pos_oi, "max_neg_oi_strike": opt.max_neg_oi,
        "max_pos_vol_strike": opt.max_pos_vol, "max_neg_vol_strike": opt.max_neg_vol,
    });

    let mut plan = Plan {
        ticker: input.ticker.clone(),
        spot: price,
        as_of: input.as_of.clone(),
        analysis_date: input.analysis_date,
        captured_ts: input.captured_ts,
        embedded_basis: input.embedded_basis,
        data_mode: if em.is_some() {
            "full"
        } else {
            "structure_only"
        }
        .into(),
        vix,
        vix1d,
        vix_family: input.volatility.vix_family.clone(),
        em: input.em.clone(),
        flip: opt.flip,
        net_gex_vol: opt.net_gex_vol,
        regime: regime_out.regime,
        regime_notes: regime_out.notes,
        regime_prob: regime_out.prob,
        bias,
        pivot,
        bull_targets: lv.bull_targets,
        bear_targets: lv.bear_targets,
        major_long_support: lv.major_long_support,
        squeeze_zone: lv.squeeze_zone,
        negative_gamma_band: lv.negative_gamma_band,
        negative_gamma_strikes_near_spot: lv.negative_gamma_strikes_near_spot,
        negative_gamma_total: lv.negative_gamma_total,
        levels_tolerance: lv.tolerance,
        all_clusters_n: lv.all_clusters_n,
        gamma_detail,
        delta_detail,
        narrative: vec![],
        limitations,
        lineage,
        lineage_notes: vec![lineage_msg],
        // v1.1 字段在后续提交填充（parity/dual-EM/动态静态分离）
        synthetic_forward: None,
        parity: None,
        session_em: em,
        remaining_em: em,
        freshness,
        data_warnings: vec![],
        source_flow: opt.flow.clone(),
    };
    plan.narrative = narrative::gen_narrative(&plan);
    Ok(plan)
}

fn r1(x: Option<f64>) -> Option<f64> {
    x.map(|v| (v * 10.0).round() / 10.0)
}
