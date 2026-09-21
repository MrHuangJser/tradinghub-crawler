//! plan.json vs blogger.json 逐位残差对比。
//! 冒烟门槛（计划 §9）：关键位残差 ≤2 ES 点记命中。

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

const HIT_TOL: f64 = 2.0;

#[derive(Debug, Deserialize)]
struct Blogger {
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    pivot: Option<PivotObs>,
    #[serde(default)]
    bull_targets: Vec<f64>,
    #[serde(default)]
    bear_targets: Vec<f64>,
    #[serde(default)]
    major_long_support: Option<f64>,
    #[serde(default)]
    squeeze_zone: Option<f64>,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
struct PivotObs {
    value: f64,
}

#[derive(Debug, Deserialize)]
struct Rule {
    #[serde(rename = "type")]
    rule_type: String,
    level: f64,
}

pub fn run(plan_path: &str, blogger_path: &str) -> Result<i32> {
    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(plan_path)
            .map_err(|e| anyhow::anyhow!("读取 {plan_path} 失败: {e}"))?,
    )?;
    let blogger: Blogger = serde_json::from_str(
        &std::fs::read_to_string(blogger_path)
            .map_err(|e| anyhow::anyhow!("读取 {blogger_path} 失败: {e}"))?,
    )?;

    let get_f64 = |path: &str| plan.pointer(path).and_then(Value::as_f64);
    let get_lvl = |arr: &str, i: usize| {
        plan.pointer(&format!("{arr}/{i}/level"))
            .and_then(Value::as_f64)
    };

    println!("# 校准对比 — {}", blogger.date.as_deref().unwrap_or("?"));
    println!();
    println!("| 位 | 博主 | 我方 | 残差 | 判定 |");
    println!("|---|---|---|---|---|");

    let mut hits = 0usize;
    let mut total = 0usize;
    let mut row = |label: &str, mine: Option<f64>, theirs: Option<f64>| {
        let (m, t, verdict) = match (mine, theirs) {
            (Some(m), Some(t)) => {
                let d = (m - t).abs();
                let hit = d <= HIT_TOL;
                if hit {
                    hits += 1;
                }
                total += 1;
                (
                    format!("{m:.2}"),
                    format!("{t:.2}"),
                    format!("{d:.2} {}", if hit { "✅" } else { "❌" }),
                )
            }
            (Some(m), None) => (format!("{m:.2}"), "—".into(), "—".into()),
            (None, Some(t)) => ("—".into(), format!("{t:.2}"), "我方缺失".into()),
            (None, None) => return,
        };
        println!("| {label} | {t} | {m} | {verdict} |");
    };

    row(
        "多空转换位",
        get_f64("/pivot/level"),
        blogger.pivot.map(|p| p.value),
    );
    for (i, t) in blogger.bull_targets.iter().enumerate() {
        row(
            &format!("多头目标{}", i + 1),
            get_lvl("/bull_targets", i),
            Some(*t),
        );
    }
    for (i, t) in blogger.bear_targets.iter().enumerate() {
        row(
            &format!("空头目标{}", i + 1),
            get_lvl("/bear_targets", i),
            Some(*t),
        );
    }
    row(
        "核心多头防守",
        get_f64("/major_long_support/level"),
        blogger.major_long_support,
    );
    row(
        "Squeeze Zone",
        get_f64("/squeeze_zone/level"),
        blogger.squeeze_zone,
    );

    println!();
    if total > 0 {
        println!("命中率：{hits}/{total}（阈值 ≤{HIT_TOL} 点）");
    }
    if !blogger.rules.is_empty() {
        println!();
        println!("博主规则（人工判断，不自动评分）：");
        for r in &blogger.rules {
            println!("- {} @ {:.2}", r.rule_type, r.level);
        }
    }
    Ok(0)
}
