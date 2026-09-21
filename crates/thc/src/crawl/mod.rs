//! 抓取层：多数据源并发拉取（tokio）。
//! 每个源实现 `DataSource`，产出 raw payload；`adapt` 模块负责映射到
//! `thc_engine::EngineInput`——engine 不感知源。

pub mod cboe;
pub mod payload;
pub mod tradinghub;

/// 数据源统一接口：拉取原始数据。
/// TODO(Phase 2)：定义返回类型与异步方法（Phase 0 调研结论决定第二源）。
#[allow(dead_code)] // Phase 2 crawler 实现后消费
pub trait DataSource {
    /// 源标识（写入血缘审计的 `source` 字段）。
    fn name(&self) -> &'static str;
}
