//! 抓取层：多数据源并发拉取（tokio）。
//! 每个 endpoint 先取 serde_json::Value（--raw 无损输出），再反序列化为
//! `payload` 模块的强类型——schema 漂移在此刻爆为 SchemaDrift 错误。

pub mod cboe;
pub mod payload;
pub mod tradinghub;

use thiserror::Error;

/// 抓取错误 → 进程退出码（沿用旧工具契约：0/2/3/4，新增 5=schema 漂移）。
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("凭据缺失或登录失败：{0}")]
    Auth(String),
    #[error("网络/接口请求失败：{0}")]
    Http(String),
    #[error("标的 {ticker} 无数据。可用标的：{available:?}")]
    NoData {
        ticker: String,
        available: Vec<String>,
    },
    #[error("TradingHub 接口疑似变更（反序列化失败）：{0}")]
    SchemaDrift(String),
}

impl FetchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Auth(_) => 2,
            Self::Http(_) => 3,
            Self::NoData { .. } => 4,
            Self::SchemaDrift(_) => 5,
        }
    }
}

/// `thc fetch`：登录 → 双接口 → 标的视图（或 --raw 原始合并 payload）。
pub async fn run_fetch(
    cfg: &crate::config::AppConfig,
    ticker: &str,
    raw: bool,
    output: Option<&str>,
) -> Result<i32, FetchError> {
    let client = tradinghub::Client::new();
    client
        .login(
            cfg.tradinghub.email.as_deref().unwrap_or(""),
            cfg.tradinghub.password.as_deref().unwrap_or(""),
        )
        .await?;
    let merged = client.fetch_merged().await?;

    let text = if raw {
        serde_json::to_string_pretty(&merged.raw()).expect("payload serialize")
    } else {
        let view = merged.view(ticker)?;
        serde_json::to_string_pretty(&view).expect("view serialize")
    };

    match output {
        Some(path) => {
            std::fs::write(path, format!("{text}\n"))
                .map_err(|e| FetchError::Http(format!("写入 {path} 失败: {e}")))?;
            eprintln!("✅ 已写入 {path}（{} 字节）", text.len());
        }
        None => println!("{text}"),
    }
    Ok(0)
}

/// `thc fetch --cboe`：CBOE 免费数据（VIX 家族 + SPX 链 EM），免登录。
pub async fn run_fetch_cboe(output: Option<&str>) -> Result<i32, FetchError> {
    let md = cboe::Cboe::new().market_data(true).await;
    let text = serde_json::to_string_pretty(&md).expect("market data serialize");
    match output {
        Some(path) => {
            std::fs::write(path, format!("{text}\n"))
                .map_err(|e| FetchError::Http(format!("写入 {path} 失败: {e}")))?;
            eprintln!("✅ 已写入 {path}（{} 字节）", text.len());
        }
        None => println!("{text}"),
    }
    Ok(0)
}
