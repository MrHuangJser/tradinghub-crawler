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

/// 技术位注入（解锁 pivot 融合与目标可达性判定）。
#[derive(Debug, Default, Args)]
struct TechOverrides {
    /// 精确 Expected Move（覆盖引擎计算值）
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
        tech: TechOverrides,
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
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 抓取 + 引擎 → plan.json
    Plan {
        #[command(flatten)]
        tech: TechOverrides,
        #[arg(short, long, default_value = "plan.json")]
        output: String,
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
            tech,
            output,
            offline,
            no_archive,
            stdout,
        } => {
            let _ = (tech, output, offline, no_archive, stdout);
            todo_exit("run")
        }
        Command::Fetch {
            ticker,
            raw,
            output,
        } => {
            let _ = (ticker, raw, output);
            todo_exit("fetch")
        }
        Command::Plan {
            tech,
            output,
            offline,
        } => {
            let _ = (tech, output, offline);
            todo_exit("plan")
        }
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
