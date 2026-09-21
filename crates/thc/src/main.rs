//! thc —— TradingHub 抓取 + ES 盘前报告 CLI。
//! 主路径零 LLM；`parse-blogger` 为离线校准工具（llm feature 门控）。

mod adapt;
mod archive;
mod config;
mod crawl;
mod render;
mod tools;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

/// 引擎输入覆盖：技术位 + 波动率 + 离线文件。
#[derive(Debug, Default, Args)]
struct Inputs {
    /// 精确 Expected Move（覆盖 CBOE 估算）
    #[arg(long)]
    em: Option<f64>,
    #[arg(long)]
    vwap: Option<f64>,
    /// 隔夜高
    #[arg(long)]
    onh: Option<f64>,
    /// 隔夜低
    #[arg(long)]
    onl: Option<f64>,
    /// 前日高
    #[arg(long)]
    pdh: Option<f64>,
    /// 前日低
    #[arg(long)]
    pdl: Option<f64>,
    /// Volume Profile POC
    #[arg(long)]
    poc: Option<f64>,
    /// 前日 Pivot
    #[arg(long)]
    prior_pivot: Option<f64>,
    /// 已实现波幅（点），用于 regime 细化
    #[arg(long)]
    realized_range: Option<f64>,
    #[arg(long)]
    vix: Option<f64>,
    #[arg(long)]
    vix1d: Option<f64>,
    /// 离线：读取 ES_SPX 的标的视图 JSON（替代联网抓取）
    #[arg(long)]
    es_file: Option<String>,
    /// 离线：读取 SPX 的标的视图 JSON（算内嵌 basis）
    #[arg(long)]
    spx_file: Option<String>,
}

#[derive(Parser)]
#[command(
    name = "thc",
    version,
    about = "ES 盘前分析：抓取 TradingHub/CBOE → 确定性引擎 → Markdown 报告"
)]
struct Cli {
    /// 配置文件路径（默认 ./config.toml）
    #[arg(long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 一键：抓取 → 引擎 → report.md（默认归档到 archive/YYYY-MM-DD/）
    Run {
        #[command(flatten)]
        inputs: Inputs,
        /// 输出路径（默认 report.md）
        #[arg(short, long)]
        output: Option<String>,
        /// 离线/纯结构模式：不联网 CBOE
        #[arg(long)]
        offline: bool,
        /// 关闭归档
        #[arg(long)]
        no_archive: bool,
        /// 报告打到 stdout
        #[arg(long)]
        stdout: bool,
    },
    /// 仅抓取：输出原始 payload
    Fetch {
        /// 标的（默认 ES_SPX）
        #[arg(long, default_value = "ES_SPX")]
        ticker: String,
        /// 输出两接口合并后的原始 payload
        #[arg(long)]
        raw: bool,
        /// 抓 CBOE 免费数据（VIX 家族 + SPX 链 EM），免登录
        #[arg(long)]
        cboe: bool,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 抓取 + 引擎 → plan.json
    Plan {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(short, long, default_value = "plan.json")]
        output: String,
        /// 跳过 CBOE（VIX/EM），纯结构模式
        #[arg(long)]
        offline: bool,
    },
    /// plan.json → Markdown 报告
    Report {
        /// 输入 plan JSON
        plan: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 博主盘前帖 → 结构化校准数据（LLM 视觉，离线工具）
    #[cfg(feature = "llm")]
    ParseBlogger {
        /// 输入文件：文本或截图图片，可多份
        inputs: Vec<String>,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// plan.json vs blogger.json 逐位残差对比
    Compare { plan: String, blogger: String },
}

#[tokio::main]
async fn main() {
    let code = match real_main().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("❌ {e:#}");
            1
        }
    };
    std::process::exit(code);
}

async fn real_main() -> Result<i32> {
    let cli = Cli::parse();
    let _cfg = config::AppConfig::load(cli.config.as_deref())?;

    match cli.command {
        Command::Run {
            inputs,
            output,
            offline,
            no_archive,
            stdout,
        } => {
            let _ = (inputs, output, offline, no_archive, stdout);
            todo_exit("run")
        }
        Command::Fetch {
            ticker,
            raw,
            cboe,
            output,
        } => {
            let res = if cboe {
                crawl::run_fetch_cboe(output.as_deref()).await
            } else {
                crawl::run_fetch(&_cfg, &ticker, raw, output.as_deref()).await
            };
            match res {
                Ok(code) => return Ok(code),
                Err(e) => {
                    eprintln!("❌ {e}");
                    return Ok(e.exit_code());
                }
            }
        }
        Command::Plan {
            inputs,
            output,
            offline,
        } => match run_plan(&_cfg, inputs, &output, offline).await {
            Ok(code) => return Ok(code),
            Err(e) => {
                eprintln!("❌ {e:#}");
                let code = e
                    .downcast_ref::<crawl::FetchError>()
                    .map(|f| f.exit_code())
                    .unwrap_or(1);
                return Ok(code);
            }
        },
        Command::Report { plan, output } => {
            let _ = (plan, output);
            todo_exit("report")
        }
        #[cfg(feature = "llm")]
        Command::ParseBlogger { inputs, output } => {
            let _ = (inputs, output);
            todo_exit("parse-blogger")
        }
        Command::Compare { plan, blogger } => {
            let _ = (plan, blogger);
            todo_exit("compare")
        }
    }
    .map(|()| 0)
}

fn todo_exit(name: &str) -> Result<()> {
    anyhow::bail!("子命令 `{name}` 尚未实现（Phase 2+）。骨架已就绪。")
}

/// `thc plan`：抓取（或离线文件）→ adapt → engine → plan.json。
async fn run_plan(
    cfg: &config::AppConfig,
    inputs: Inputs,
    output: &str,
    offline: bool,
) -> Result<i32> {
    use thc_engine::{EmEstimate, EngineInput};

    // 1) 标的视图（在线抓 ES_SPX+SPX，或离线文件）
    let (snap_es, snap_spx) = if let Some(es_file) = &inputs.es_file {
        let es: crawl::payload::TickerView = serde_json::from_str(
            &std::fs::read_to_string(es_file)
                .map_err(|e| anyhow::anyhow!("读取 {es_file} 失败: {e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("解析 {es_file} 失败: {e}"))?;
        let spx = match &inputs.spx_file {
            Some(f) => Some(
                serde_json::from_str(
                    &std::fs::read_to_string(f)
                        .map_err(|e| anyhow::anyhow!("读取 {f} 失败: {e}"))?,
                )
                .map_err(|e| anyhow::anyhow!("解析 {f} 失败: {e}"))?,
            ),
            None => None,
        };
        (es, spx)
    } else {
        let client = crawl::tradinghub::Client::new();
        client
            .login(
                cfg.tradinghub.email.as_deref().unwrap_or(""),
                cfg.tradinghub.password.as_deref().unwrap_or(""),
            )
            .await
            .map_err(anyhow::Error::from)?;
        let merged = client.fetch_merged().await.map_err(anyhow::Error::from)?;
        let es = merged.view("ES_SPX").map_err(anyhow::Error::from)?;
        let spx = merged.view("SPX").ok();
        (es, spx)
    };

    // 2) CBOE 市场数据（除非 --offline 或 --em 已给）
    let market = if !offline && inputs.em.is_none() {
        Some(crawl::cboe::Cboe::new().market_data(true).await)
    } else {
        None
    };

    // 3) 组装 EngineInput
    let em = match inputs.em {
        Some(e) => Some(EmEstimate {
            em_0dte_spx: e,
            method: "manual override".into(),
            source_expiry: None,
            source_straddle: None,
        }),
        None => market
            .as_ref()
            .and_then(|m| m.em.as_ref().map(adapt::em_estimate)),
    };
    let input = EngineInput {
        analysis_date: adapt::et_today(),
        now_ts: jiff::Timestamp::now().as_second(),
        as_of: snap_es
            .captured_at
            .clone()
            .or_else(|| snap_es.generated_at.clone())
            .unwrap_or_default(),
        ticker: "ES_SPX".into(),
        captured_ts: snap_es
            .captured_ts
            .or_else(|| snap_es.levels_summary.as_ref().map(|l| l.timestamp)),
        options: adapt::options_structure(&snap_es),
        volatility: market.as_ref().map(adapt::volatility).unwrap_or_default(),
        technicals: adapt::technicals(
            inputs.vwap,
            inputs.poc,
            inputs.pdh,
            inputs.pdl,
            inputs.onh,
            inputs.onl,
            inputs.prior_pivot,
            inputs.realized_range,
        ),
        em,
        chain: market
            .as_ref()
            .and_then(|m| m.chain.as_ref())
            .map(|c| adapt::chain_contracts(&c.options))
            .unwrap_or_default(),
        embedded_basis: adapt::embedded_basis(&snap_es, snap_spx.as_ref()),
        vix_override: inputs.vix,
        vix1d_override: inputs.vix1d,
    };

    // 4) 引擎
    let plan = thc_engine::build_plan(&input, &cfg.engine)
        .map_err(|e| anyhow::anyhow!("引擎失败: {e}"))?;

    // 5) 输出
    let text = serde_json::to_string_pretty(&plan)?;
    std::fs::write(output, format!("{text}\n"))?;
    eprintln!("✅ 计划已写入 {output}");
    for line in &plan.narrative {
        eprintln!("  {line}");
    }
    Ok(0)
}
