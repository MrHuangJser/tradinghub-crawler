//! TradingHub 上游 payload 强类型模型（docs/ANALYSIS.md + 实测 2026-09-18 payload）。
//!
//! 漂移策略（Q14=A）：结构字段必需、类型不符 → 反序列化报错；
//! 指标值一律 `Option<f64>`（null/缺失由引擎语义层判定降级，不在这里崩）；
//! 位置数组（strikes/raw_row/mini_contracts）按 legacy/schemas.py 逐位释义定型。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type TickerMap<T> = BTreeMap<String, T>;

// ---------------------------------------------------------------------------
// 接口响应外壳
// ---------------------------------------------------------------------------

/// `GET /beta-test/api/gex/live-data`
#[derive(Debug, Deserialize, Serialize)]
pub struct GexResponse {
    pub ok: bool,
    pub generated_at: Option<String>,
    pub last_updated_at: Option<String>,
    pub stale: bool,
    pub primary: GexPrimary,
    /// 数据源元信息（形状未公开，保留原值）
    #[serde(default)]
    pub sources: Option<Value>,
}

/// `GET /beta-test/api/options-data/exposure`
#[derive(Debug, Deserialize, Serialize)]
pub struct ExposureResponse {
    pub ok: bool,
    pub generated_at: Option<String>,
    pub last_updated_at: Option<String>,
    pub stale: bool,
    pub primary: ExposurePrimary,
    #[serde(default)]
    pub sources: Option<Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GexPrimary {
    #[serde(default)]
    pub tickers: Vec<String>,
    #[serde(default)]
    pub levels: TickerMap<Levels>,
    #[serde(default)]
    pub classic_chain: TickerMap<ChainBlock>,
    #[serde(default)]
    pub state_greeks: TickerMap<StateBlock>,
    /// 按 DTE 模式聚合的 GEX 链（zero=0DTE, one=1DTE+, net=90天）
    #[serde(default)]
    pub gex_zero: TickerMap<ChainBlock>,
    #[serde(default)]
    pub gex_one: TickerMap<ChainBlock>,
    #[serde(default)]
    pub gex_net: TickerMap<ChainBlock>,
    /// 按 DTE 模式聚合的状态希腊值（dex/vex/chex 同构 StateBlock）
    #[serde(default)]
    pub dex_zero: TickerMap<StateBlock>,
    #[serde(default)]
    pub dex_one: TickerMap<StateBlock>,
    #[serde(default)]
    pub dex_net: TickerMap<StateBlock>,
    #[serde(default)]
    pub vex_zero: TickerMap<StateBlock>,
    #[serde(default)]
    pub vex_one: TickerMap<StateBlock>,
    #[serde(default)]
    pub vex_net: TickerMap<StateBlock>,
    #[serde(default)]
    pub chex_zero: TickerMap<StateBlock>,
    #[serde(default)]
    pub chex_one: TickerMap<StateBlock>,
    #[serde(default)]
    pub chex_net: TickerMap<StateBlock>,
    #[serde(default)]
    pub iv_zero: TickerMap<StateBlock>,
    #[serde(default)]
    pub iv_one: TickerMap<StateBlock>,
    #[serde(default)]
    pub state_volume_zero: TickerMap<StateBlock>,
    #[serde(default)]
    pub state_volume_one: TickerMap<StateBlock>,
    #[serde(default)]
    pub orderflow: TickerMap<Orderflow>,
    #[serde(default)]
    pub gex_proxy: TickerMap<GexProxy>,
    #[serde(default)]
    pub exposure: TickerMap<Exposure>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ExposurePrimary {
    #[serde(default)]
    pub exposure: TickerMap<Exposure>,
    #[serde(default)]
    pub levels: TickerMap<Levels>,
    #[serde(default)]
    pub gex_proxy: TickerMap<GexProxy>,
    #[serde(default)]
    pub orderflow: TickerMap<Orderflow>,
}

// ---------------------------------------------------------------------------
// 板块类型
// ---------------------------------------------------------------------------

/// `levels.<T>`：关键价位概览。timestamp = Unix 秒（ET 捕获时间）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Levels {
    pub ticker: String,
    pub timestamp: i64,
    pub spot: f64,
    pub zero_gamma: Option<f64>,
    pub mpos_vol: Option<f64>,
    pub mpos_oi: Option<f64>,
    pub mneg_vol: Option<f64>,
    pub mneg_oi: Option<f64>,
    pub net_gex_vol: Option<f64>,
    pub net_gex_oi: Option<f64>,
}

/// classic_chain 与 gex_{zero,one,net} 同构。
/// `strikes` 位置数组：[strike, gex_vol, gex_oi(保留位常为0), lookback[5]]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainBlock {
    pub ticker: String,
    pub timestamp: i64,
    pub spot: f64,
    pub min_dte: Option<i64>,
    pub sec_min_dte: Option<i64>,
    pub zero_gamma: Option<f64>,
    pub major_pos_vol: Option<f64>,
    pub major_pos_oi: Option<f64>,
    pub major_neg_vol: Option<f64>,
    pub major_neg_oi: Option<f64>,
    #[serde(default)]
    pub strikes: Vec<ChainStrike>,
    pub sum_gex_vol: Option<f64>,
    pub sum_gex_oi: Option<f64>,
    pub delta_risk_reversal: Option<f64>,
    /// 历史极值记录 [行权价, 极值]
    #[serde(default)]
    pub max_priors: Vec<(f64, f64)>,
}

/// [strike, gex_vol, gex_oi, lookback]
pub type ChainStrike = (f64, f64, f64, Vec<Option<f64>>);

/// state_greeks / iv_* / state_volume_* / dex|vex|chex_{zero,one,net} 同构。
/// `mini_contracts` 位置数组：[strike, call分量, put分量, 主值, lookback[3], 保留0, 保留null]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateBlock {
    pub ticker: String,
    pub timestamp: i64,
    pub spot: f64,
    pub min_dte: Option<i64>,
    pub sec_min_dte: Option<i64>,
    pub major_positive: Option<f64>,
    pub major_negative: Option<f64>,
    pub major_long_gamma: Option<f64>,
    pub major_short_gamma: Option<f64>,
    #[serde(default)]
    pub mini_contracts: Vec<MiniRow>,
}

/// [strike, call分量, put分量, 主值, lookback, 保留0, 保留位]
/// 注意（实测 2026-09-18）：state_volume_* 块的位置 6 不是 null 而是三元数组，
/// 故该位用 Value 兜底。
pub type MiniRow = (f64, f64, f64, f64, Vec<Option<f64>>, f64, Value);

/// `gex_proxy.<T>`：前列 Gamma 行权价。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GexProxy {
    pub ticker: String,
    pub timestamp: i64,
    pub spot: f64,
    pub min_dte: Option<i64>,
    pub sec_min_dte: Option<i64>,
    pub major_positive: Option<f64>,
    pub major_negative: Option<f64>,
    pub major_long_gamma: Option<f64>,
    pub major_short_gamma: Option<f64>,
    pub metrics: Option<GexProxyMetrics>,
    #[serde(default)]
    pub ladder: Vec<LadderRow>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GexProxyMetrics {
    pub levels_count: Option<i64>,
    pub positive_gamma: Option<f64>,
    pub negative_gamma: Option<f64>,
    pub net_gamma: Option<f64>,
    pub absolute_gamma: Option<f64>,
    pub zero_gamma_proxy: Option<f64>,
    pub largest_positive_strike: Option<f64>,
    pub largest_negative_strike: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LadderRow {
    pub strike: f64,
    pub current_value: Option<f64>,
    pub gamma: Option<f64>,
    pub abs_value: Option<f64>,
    pub abs_gamma: Option<f64>,
    pub side: Option<String>,
    pub distance_from_spot: Option<f64>,
    pub distance_percent: Option<f64>,
    #[serde(default)]
    pub lookback_values: Vec<Option<f64>>,
    #[serde(default)]
    pub dte_values: Vec<Option<f64>>,
    /// 后端原始 7 元组（与命名字段重复，保留以无损）
    #[serde(default)]
    pub raw_row: Vec<Value>,
}

/// `orderflow.<T>`：订单流看板。z*=0DTE, o*=1DTE+。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Orderflow {
    pub ticker: String,
    pub timestamp: i64,
    pub spot: f64,
    pub z_mlgamma: Option<f64>,
    pub z_msgamma: Option<f64>,
    pub o_mlgamma: Option<f64>,
    pub o_msgamma: Option<f64>,
    pub zero_mcall: Option<f64>,
    pub zero_mput: Option<f64>,
    pub one_mcall: Option<f64>,
    pub one_mput: Option<f64>,
    pub zcvr: Option<f64>,
    pub ocvr: Option<f64>,
    pub zgr: Option<f64>,
    pub ogr: Option<f64>,
    pub zvanna: Option<f64>,
    pub ovanna: Option<f64>,
    pub zcharm: Option<f64>,
    pub ocharm: Option<f64>,
    pub agg_dex: Option<f64>,
    pub one_agg_dex: Option<f64>,
    pub agg_call_dex: Option<f64>,
    pub one_agg_call_dex: Option<f64>,
    pub agg_put_dex: Option<f64>,
    pub one_agg_put_dex: Option<f64>,
    pub net_dex: Option<f64>,
    pub one_net_dex: Option<f64>,
    pub net_call_dex: Option<f64>,
    pub one_net_call_dex: Option<f64>,
    pub net_put_dex: Option<f64>,
    pub one_net_put_dex: Option<f64>,
    pub dexoflow: Option<f64>,
    pub gexoflow: Option<f64>,
    pub cvroflow: Option<f64>,
    pub one_dexoflow: Option<f64>,
    pub one_gexoflow: Option<f64>,
    pub one_cvroflow: Option<f64>,
}

/// `exposure.<T>`：希腊值分布图表数据（权威源是 exposure 接口）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Exposure {
    pub symbol: String,
    #[serde(rename = "underlyingPrice")]
    pub underlying_price: Option<f64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub metrics: ExposureMetrics,
    /// 形状未公开，保留原值
    #[serde(default)]
    pub levels: Option<Value>,
    #[serde(rename = "rawCapabilities", default)]
    pub raw_capabilities: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExposureMetrics {
    /// {strike, onePut, ...} —— iv 行字段不固定，flatten 兜底
    #[serde(default)]
    pub iv: Vec<MetricRow>,
    #[serde(default)]
    pub oi: Vec<MetricRow>,
    #[serde(default)]
    pub volume: Vec<MetricRow>,
    #[serde(default)]
    pub gex: Vec<MetricRow>,
    #[serde(default)]
    pub dex: Vec<MetricRow>,
    #[serde(default)]
    pub vex: Vec<MetricRow>,
    #[serde(default)]
    pub chex: Vec<MetricRow>,
}

/// 按行权价的指标行：`strike` 必需，其余键（zero/one/net/total/zeroCall/…）进 extra。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricRow {
    pub strike: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
// 合并视图（对应 legacy extract_ticker 的输出）
// ---------------------------------------------------------------------------

/// `fetch` 输出的标的视图。
#[derive(Debug, Serialize)]
pub struct TickerView {
    pub ticker: String,
    pub generated_at: Option<String>,
    pub last_updated_at: Option<String>,
    pub stale: bool,
    pub spot: Option<f64>,
    /// ET 时区格式化捕获时间（沿用旧工具的展示口径）
    pub captured_at: Option<String>,
    pub captured_ts: Option<i64>,
    pub levels_summary: Option<Levels>,
    pub gamma_ladder: Option<GexProxy>,
    pub classic_chain: Option<ChainBlock>,
    pub state_greeks: Option<StateBlock>,
    pub orderflow: Option<Orderflow>,
    pub exposure: Option<Exposure>,
    pub dte_exposure: DteExposure,
}

/// 按 DTE 聚合的 exposure（zero/one/net 三键）。
#[derive(Debug, Default, Serialize)]
pub struct DteExposure {
    pub gex: DteSet<ChainBlock>,
    pub dex: DteSet<StateBlock>,
    pub vex: DteSet<StateBlock>,
    pub chex: DteSet<StateBlock>,
}

#[derive(Debug, Serialize)]
pub struct DteSet<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<T>,
}

impl<T> Default for DteSet<T> {
    fn default() -> Self {
        Self {
            zero: None,
            one: None,
            net: None,
        }
    }
}
