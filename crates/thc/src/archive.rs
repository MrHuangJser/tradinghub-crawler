//! 每日归档：`archive/YYYY-MM-DD/`，校准集的数据来源。

use anyhow::Result;
use std::path::PathBuf;

/// 当日归档目录（按 ET 日期）。
pub fn day_dir(date: &jiff::civil::Date) -> PathBuf {
    PathBuf::from("archive").join(date.to_string())
}

/// 落盘：raw payload / plan.json / report.md / blogger.json 可选写入。
/// 返回实际写入的文件路径。
pub fn save(
    date: &jiff::civil::Date,
    raw_payload: Option<&serde_json::Value>,
    plan_json: Option<&str>,
    report: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let dir = day_dir(date);
    std::fs::create_dir_all(&dir)?;
    let mut written = Vec::new();
    let mut put = |name: &str, content: &str| -> Result<()> {
        let p = dir.join(name);
        std::fs::write(&p, content)?;
        written.push(p);
        Ok(())
    };
    if let Some(v) = raw_payload {
        put("raw_payload.json", &serde_json::to_string_pretty(v)?)?;
    }
    if let Some(s) = plan_json {
        put("plan.json", s)?;
    }
    if let Some(s) = report {
        put("report.md", s)?;
    }
    Ok(written)
}
