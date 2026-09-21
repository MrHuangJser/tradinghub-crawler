//! 核心领域类型：`EngineInput`（引擎输入契约）与 `Plan`（输出契约）。
//!
//! 字段清单对齐算法文档 §3（输入数据）与 §18 `PremarketPlan`（输出）。
//! engine 不感知任何具体数据源——上游 payload 到本类型的映射由 bin crate 的
//! `adapt` 模块负责。

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 输入
// ---------------------------------------------------------------------------

/// 单条期权合约（逐 strike/逐到期）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    pub strike: f64,
    pub expiry: Date,
    pub dte: u16,
    pub call_oi: f64,
    pub put_oi: f64,
    pub call_volume: f64,
    pub put_volume: f64,
    pub call_bid: Option<f64>,
    pub call_ask: Option<f64>,
    pub put_bid: Option<f64>,
    pub put_ask: Option<f64>,
    pub call_iv: Option<f64>,
    pub put_iv: Option<f64>,
}

/// 期权链快照。`effective_timestamp` 是报价有效时间（非下载时间），
/// 血缘审计与 Gamma 重定价一律使用它（文档 §4.1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionChain {
    pub symbol: String,
    pub effective_timestamp: String,
    pub trade_date: Date,
    pub contracts: Vec<OptionContract>,
}

/// 波动率与事件数据（文档 §3.4）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VolatilityData {
    pub vix: Option<f64>,
    pub vix1d: Option<f64>,
    pub vix9d: Option<f64>,
    pub vvix: Option<f64>,
    pub skew: Option<f64>,
    /// 当日事件标记（FOMC/CPI/NFP/OPEX…），盘前人工或数据源注入。
    pub events: Vec<String>,
}

/// ES 技术结构（文档 §3.3）。全部由调用方注入（CLI 覆盖或数据源）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TechnicalStructure {
    pub overnight_high: Option<f64>,
    pub overnight_low: Option<f64>,
    pub prior_day_high: Option<f64>,
    pub prior_day_low: Option<f64>,
    pub prior_close: Option<f64>,
    pub vwap: Option<f64>,
    pub poc: Option<f64>,
    pub realized_range: Option<f64>,
}

/// 同步报价对：basis 计算的唯一合法输入（文档 §6.1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotePair {
    pub es_price: f64,
    pub es_timestamp: String,
    pub spx_price: f64,
    pub spx_timestamp: String,
}

/// 引擎输入契约：一次盘前分析所需的全部数据。
/// `as_of` 为分析基准时点（注入而非读取系统时钟，保证可测）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInput {
    pub analysis_date: Date,
    pub as_of: String,
    pub ticker: String,
    pub spot: f64,
    pub chain: OptionChain,
    pub quotes: Option<QuotePair>,
    pub volatility: VolatilityData,
    pub technicals: TechnicalStructure,
}

// ---------------------------------------------------------------------------
// 输出
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bias {
    Bullish,
    Neutral,
    Bearish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regime {
    Chopping,
    Compression,
    Normal,
    Expansion,
}

/// 血缘审计状态（文档 §4.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineageStatus {
    Current,
    DelayedUsable,
    StaleContextOnly,
    Rejected,
}

/// 一个聚类后的候选价位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    /// ES 空间价位（已映射、取整到 tick）。
    pub level: f64,
    /// SPX 空间源价位（未映射，供残差比对）。
    pub source_spx: Option<f64>,
    /// 聚类成员标签（如 "gamma_flip", "pdh", "put_node"）。
    pub members: Vec<String>,
    pub score: f64,
    /// 距现价的 EM 倍数（标准化距离，文档 §16.4）。
    pub distance_em: Option<f64>,
}

/// 多空转换位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pivot {
    pub level: f64,
    pub buffer: f64,
    pub components: Vec<String>,
}

/// `build_plan` 的输出契约（文档 §18 `PremarketPlan`）。
/// `session_em`/`remaining_em` 双轨字段保留（v1.1），盘中更新语义不在本阶段实现。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub analysis_date: Date,
    pub ticker: String,
    pub spot: f64,
    pub bias: Bias,
    pub regime: Regime,
    pub pivot: Pivot,
    pub bull_targets: Vec<Level>,
    pub bear_targets: Vec<Level>,
    pub major_long_support: Option<Level>,
    pub squeeze_zone: Option<Level>,
    pub narrative: Vec<String>,
    // --- 审计与中间量 ---
    pub lineage: LineageStatus,
    pub synchronized_basis: Option<f64>,
    pub synthetic_forward: Option<f64>,
    pub session_em: Option<f64>,
    pub remaining_em: Option<f64>,
    pub gamma_flip_spx: Option<f64>,
    pub limitations: Vec<String>,
}
