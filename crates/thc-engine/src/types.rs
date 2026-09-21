//! 核心领域类型：`EngineInput`（引擎输入契约）与 `Plan`（输出契约）。
//!
//! engine 不感知 TradingHub/CBOE 的字段命名——上游 payload 到本类型的映射
//! 由 bin crate 的 `adapt` 模块负责。`OptionStructure` 是已提取的期权结构
//! （ES 价格空间，basis 已由 ES_SPX 内嵌）。

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// 输入
// ---------------------------------------------------------------------------

/// gamma_ladder 单行（TradingHub 已算好的逐档 Gamma）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LadderPoint {
    pub strike: f64,
    pub current_value: Option<f64>,
    pub side: Option<String>,
}

/// 0DTE/1DTE+ 资金流指标（文档 §20.4：符号翻转 = 剧本失效器）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlowState {
    pub zcvr: Option<f64>,
    pub ocvr: Option<f64>,
    pub zgr: Option<f64>,
    pub ogr: Option<f64>,
    pub net_gex_vol: Option<f64>,
}

/// 订单流明细（报告"Delta & Flow"小节用；引擎只做透传）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderflowMetrics {
    pub dex_0dte: Option<f64>,
    pub call_dex_0dte: Option<f64>,
    pub put_dex_0dte: Option<f64>,
    pub cvr_0dte: Option<f64>,
    pub gex_ratio_0dte: Option<f64>,
    pub vanna_0dte: Option<f64>,
    pub charm_0dte: Option<f64>,
    pub dex_1dte: Option<f64>,
    pub call_dex_1dte: Option<f64>,
    pub put_dex_1dte: Option<f64>,
    pub cvr_1dte: Option<f64>,
    pub gex_ratio_1dte: Option<f64>,
    pub vanna_1dte: Option<f64>,
    pub charm_1dte: Option<f64>,
    pub net_dex: Option<f64>,
    pub net_call_dex: Option<f64>,
    pub net_put_dex: Option<f64>,
    pub dexoflow: Option<f64>,
    pub gexoflow: Option<f64>,
    pub cvroflow: Option<f64>,
}

/// 从上游快照提取的期权结构（ES 价格空间）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptionStructure {
    pub spot: Option<f64>,
    /// Gamma Flip（ES 空间，levels.zero_gamma）
    pub flip: Option<f64>,
    pub net_gex_vol: Option<f64>,
    pub net_gex_oi: Option<f64>,
    pub call_wall_0dte: Option<f64>,
    pub call_wall_1dte: Option<f64>,
    pub put_wall_0dte: Option<f64>,
    pub put_wall_1dte: Option<f64>,
    /// 若快照是昨收，记录昨日已结算的 Call 墙（仅作参考）
    #[serde(default)]
    pub call_wall_expired: Option<f64>,
    /// 若快照是昨收，记录昨日已结算的 Put 墙（仅作参考）
    #[serde(default)]
    pub put_wall_expired: Option<f64>,
    pub major_long_gamma: Option<f64>,
    pub major_short_gamma: Option<f64>,
    pub max_pos_oi: Option<f64>,
    pub max_neg_oi: Option<f64>,
    pub max_pos_vol: Option<f64>,
    pub max_neg_vol: Option<f64>,
    #[serde(default)]
    pub gamma_ladder: Vec<LadderPoint>,
    #[serde(default)]
    pub neg_gamma_strikes: Vec<f64>,
    #[serde(default)]
    pub pos_gamma_strikes: Vec<f64>,
    #[serde(default)]
    pub flow: FlowState,
    #[serde(default)]
    pub orderflow: OrderflowMetrics,
}

/// CBOE 期权链上的单合约（parity/EM 用；v1.1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainContract {
    pub expiry: Date,
    pub is_call: bool,
    pub strike: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub iv: Option<f64>,
    pub open_interest: Option<f64>,
    pub volume: Option<f64>,
}

/// 单个波动率指数观测（value + change；背离判定需要 change）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VixObs {
    pub value: Option<f64>,
    pub change: Option<f64>,
}

/// 波动率与事件数据（文档 §3.4）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VolatilityData {
    #[serde(default)]
    pub vix_family: BTreeMap<String, VixObs>,
    #[serde(default)]
    pub events: Vec<String>,
}

/// ES 技术结构（文档 §3.3），全部由调用方注入。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Technicals {
    pub vwap: Option<f64>,
    pub poc: Option<f64>,
    pub pdh: Option<f64>,
    pub pdl: Option<f64>,
    pub onh: Option<f64>,
    pub onl: Option<f64>,
    pub prior_pivot: Option<f64>,
    /// 前收（ES–VIX 背离判定的价格方向输入）
    pub prior_close: Option<f64>,
    pub realized_range: Option<f64>,
}

/// EM 估计（0DTE straddle 快照或 √T 近似或人工覆盖）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmEstimate {
    pub em_0dte_spx: f64,
    pub method: String,
    #[serde(default)]
    pub source_expiry: Option<String>,
    #[serde(default)]
    pub source_straddle: Option<f64>,
}

/// 引擎输入契约：一次盘前分析所需的全部数据。
/// `now_ts`（Unix 秒）与 `analysis_date`（ET 日期）显式注入，不读系统时钟。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInput {
    pub analysis_date: Date,
    /// 分析基准时点（Unix 秒，时效门禁/血缘用）
    pub now_ts: i64,
    /// 展示用时间戳（ET 字符串）
    pub as_of: String,
    pub ticker: String,
    /// 快照捕获时间（Unix 秒）
    pub captured_ts: Option<i64>,
    pub options: OptionStructure,
    #[serde(default)]
    pub volatility: VolatilityData,
    #[serde(default)]
    pub technicals: Technicals,
    /// EM 覆盖（--em 注入）或数据源估计
    pub em: Option<EmEstimate>,
    /// CBOE 原始链（v1.1 parity 合成远期/精确 straddle 用；可为空）
    #[serde(default)]
    pub chain: Vec<ChainContract>,
    /// TradingHub 内嵌 basis（ES_SPX.spot − SPX.spot，透明审计用）
    pub embedded_basis: Option<f64>,
    /// vix/vix1d 人工覆盖
    pub vix_override: Option<f64>,
    pub vix1d_override: Option<f64>,
}

// ---------------------------------------------------------------------------
// 输出
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bias {
    #[serde(rename = "BULLISH")]
    Bullish,
    #[serde(rename = "NEUTRAL")]
    Neutral,
    #[serde(rename = "BEARISH")]
    Bearish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regime {
    #[serde(rename = "CHOPPING")]
    Chopping,
    #[serde(rename = "COMPRESSION")]
    Compression,
    #[serde(rename = "NORMAL")]
    Normal,
    #[serde(rename = "EXPANSION")]
    Expansion,
}

/// 血缘/时效门禁状态（文档 §4.1 的四态 + 旧版 FRESH 语义合并）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineageStatus {
    /// 今日、<15 分钟
    #[serde(rename = "FRESH")]
    Fresh,
    /// 今日但滞后 >15 分钟
    #[serde(rename = "STALE_TODAY")]
    StaleToday,
    /// 非今日、当前盘前 → 昨结数据本就是盘前正解
    #[serde(rename = "PRIOR_CLOSE_OK")]
    PriorCloseOk,
    /// 非今日、且当前不在盘前 → 危险
    #[serde(rename = "STALE_PRIOR_DAY")]
    StalePriorDay,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

/// 聚类后的候选价位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub level: f64,
    #[serde(default)]
    pub types: Vec<String>,
    pub score: f64,
    /// 聚类覆盖区间（成员>1 时）
    pub band: Option<(f64, f64)>,
    /// 距现价的 EM 倍数（标准化距离，文档 §16.4）
    pub distance_em: Option<f64>,
    #[serde(default)]
    pub rationale: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pivot {
    pub level: f64,
    /// 文档 §9.1 buffer = max(2.0, 0.05×EM_total)
    pub buffer: f64,
    /// 参与融合的候选名
    pub sources: Vec<String>,
}

/// Parity 审计结果（文档 §4.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParityInfo {
    pub synthetic_forward: f64,
    pub pairs: usize,
    pub dispersion: f64,
    /// true=离散超限，EM/Flip/墙位应拒绝计算
    pub rejected: bool,
}

/// `build_plan` 输出契约（对应 Python plan dict，字段为报告渲染所需全集）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub ticker: String,
    pub spot: f64,
    pub as_of: String,
    pub analysis_date: Date,
    pub captured_ts: Option<i64>,
    pub embedded_basis: Option<f64>,
    /// "full" | "structure_only"
    pub data_mode: String,
    pub vix: Option<f64>,
    pub vix1d: Option<f64>,
    pub vix_family: BTreeMap<String, VixObs>,
    pub em: Option<EmEstimate>,
    pub flip: Option<f64>,
    pub net_gex_vol: Option<f64>,
    pub regime: Regime,
    #[serde(default)]
    pub regime_notes: Vec<String>,
    #[serde(default)]
    pub regime_prob: BTreeMap<String, f64>,
    pub bias: Option<Bias>,
    pub pivot: Option<Pivot>,
    pub bull_targets: Vec<Level>,
    pub bear_targets: Vec<Level>,
    pub major_long_support: Option<Level>,
    pub squeeze_zone: Option<Level>,
    pub negative_gamma_band: Option<(f64, f64)>,
    #[serde(default)]
    pub negative_gamma_strikes_near_spot: Vec<f64>,
    pub negative_gamma_total: usize,
    /// 聚类容差（调试用）
    pub levels_tolerance: f64,
    /// 聚类总数（调试用）
    pub all_clusters_n: usize,
    #[serde(default)]
    pub gamma_detail: serde_json::Value,
    #[serde(default)]
    pub delta_detail: serde_json::Value,
    #[serde(default)]
    pub narrative: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    // --- v1.1 审计字段 ---
    pub lineage: LineageStatus,
    #[serde(default)]
    pub lineage_notes: Vec<String>,
    pub synthetic_forward: Option<f64>,
    pub parity: Option<ParityInfo>,
    /// v1.1 双 EM 字段；remaining 在无盘中更新语义下恒等于 session
    pub session_em: Option<f64>,
    pub remaining_em: Option<f64>,
    /// 时效明细（对应旧版 plan.freshness；报告渲染用）
    pub freshness: Option<serde_json::Value>,
    /// 抓取层警告（CBOE 降级等）
    #[serde(default)]
    pub data_warnings: Vec<String>,
    /// 资金流状态（供未来失效判定）
    #[serde(default)]
    pub source_flow: FlowState,
}
