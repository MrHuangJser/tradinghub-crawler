//! thc-engine —— SPX 期权驱动的 ES 盘前分析引擎。
//!
//! 纯计算 crate：**禁止依赖 reqwest/tokio 或读取系统时钟**；
//! 时间与数据一律经 `EngineInput` 显式注入（见 `types.rs`）。
//! 算法依据：`docs/SPX期权驱动的ES盘前分析算法-逆向重建.md`（v1.1）。

mod types;

pub use types::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("数据血缘审计拒绝: {0}")]
    LineageRejected(String),
    #[error("Put-Call Parity 离散度超限: {dispersion:.2} > {tolerance:.2}")]
    ParityDispersion { dispersion: f64, tolerance: f64 },
    #[error("输入数据不足: {0}")]
    InsufficientData(String),
    #[error("引擎尚未实现")]
    NotImplemented,
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
    /// pivot 缓冲：`max(2.0, coef * EM)`（文档 §9.1）。
    pub pivot_buffer_em_coef: f64,
    /// VIX1D 极端低位阈值（文档 §5.5 方法C）。
    pub vix1d_low_threshold: f64,
    /// EM 目标可达性折扣（止损判断不折扣）。
    pub em_reachability_discount: f64,
    /// Flip 失效阈值（SPX 点，文档 §5.4/20.3）。
    pub flip_invalidation_spx: f64,
    /// basis 同步窗口（秒，文档 §6.1）。
    pub basis_sync_max_skew_secs: i64,
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
        }
    }
}

/// 引擎入口：`输入 JSON 等价物 → Plan`。纯函数，无副作用。
pub fn build_plan(_input: &EngineInput, _config: &EngineConfig) -> Result<Plan, EngineError> {
    Err(EngineError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_config_defaults_match_doc() {
        let c = EngineConfig::default();
        assert_eq!(c.em_reachability_discount, 0.87);
        assert_eq!(c.vix1d_low_threshold, 10.0);
        assert_eq!(c.basis_sync_max_skew_secs, 60);
    }

    #[test]
    fn plan_roundtrip_serialization() {
        let level = Level {
            level: 7700.25,
            source_spx: Some(7672.0),
            members: vec!["gamma_flip".into()],
            score: 0.8,
            distance_em: Some(0.4),
        };
        let json = serde_json::to_string(&level).unwrap();
        let back: Level = serde_json::from_str(&json).unwrap();
        assert_eq!(back.level, 7700.25);
    }
}
