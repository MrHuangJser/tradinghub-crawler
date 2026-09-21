//! Plan → Markdown 渲染（重设计版；骨架见 REFACTOR_PLAN.md §7）。
//! 展示层唯一允许做取整的地方；价位中间值保留一位小数（文档 §20.6 教训）。

use std::fmt::Write as _;
use thc_engine::{Bias, LineageStatus, Plan, Regime};

fn fmt(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.2}")).unwrap_or("—".into())
}
fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn lineage_badge(s: LineageStatus) -> &'static str {
    match s {
        LineageStatus::Fresh => "🟢 FRESH",
        LineageStatus::StaleToday => "🟡 STALE_TODAY",
        LineageStatus::PriorCloseOk => "🟡 PRIOR_CLOSE_OK",
        LineageStatus::StalePriorDay => "🔴 STALE_PRIOR_DAY",
        LineageStatus::Unknown => "⚪ UNKNOWN",
    }
}

fn bias_cn(b: Option<Bias>) -> &'static str {
    match b {
        Some(Bias::Bullish) => "偏多",
        Some(Bias::Bearish) => "偏空",
        Some(Bias::Neutral) => "中性",
        None => "—",
    }
}

fn regime_cn(r: Regime) -> &'static str {
    match r {
        Regime::Chopping => "震荡 CHOPPING",
        Regime::Compression => "压缩 COMPRESSION",
        Regime::Normal => "正常 NORMAL",
        Regime::Expansion => "扩张 EXPANSION",
    }
}

pub fn render(plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# ES 盘前计划 — {}（{}）",
        plan.analysis_date, plan.ticker
    );
    let _ = writeln!(out);
    // 头部审计行
    let mut head: Vec<String> = vec![format!("数据有效时间 {}", plan.as_of)];
    head.push(format!("血缘 {}", lineage_badge(plan.lineage)));
    if let Some(b) = plan.embedded_basis {
        head.push(format!("Basis {b:+.2}（ES_SPX 内嵌，同步性未独立验证）"));
    }
    if let Some(em) = plan.session_em {
        head.push(format!("0DTE EM ±{em:.2}"));
    }
    let _ = writeln!(out, "> {}", head.join(" · "));
    let _ = writeln!(out);

    // 摘要
    let _ = writeln!(out, "## 摘要");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| 方向 | Regime | 转换位 | 现价 | 第一多头目标 | 第一空头目标 |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|");
    let bt1 = plan
        .bull_targets
        .first()
        .map(|l| fmt(Some(l.level)))
        .unwrap_or("—".into());
    let st1 = plan
        .bear_targets
        .first()
        .map(|l| fmt(Some(l.level)))
        .unwrap_or("—".into());
    let _ = writeln!(
        out,
        "| {} | {} | {} | {:.2} | {} | {} |",
        bias_cn(plan.bias),
        regime_cn(plan.regime),
        plan.pivot
            .as_ref()
            .map(|p| format!("{:.2}", p.level))
            .unwrap_or("—".into()),
        plan.spot,
        bt1,
        st1,
    );
    if let Some(p) = &plan.pivot {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "转换位 buffer ±{:.2}；候选来源：{}",
            p.buffer,
            p.sources.join(", ")
        );
    }
    let _ = writeln!(out);

    // 价位地图（按价位降序一表到底）
    let _ = writeln!(out, "## 价位地图");
    let _ = writeln!(out);
    let _ = writeln!(out, "| 标签 | ES 价位 | 区间 | 构成 | 评分 | 距现价(EM) |");
    let _ = writeln!(out, "|---|---|---|---|---|---|");
    let mut rows: Vec<(String, &thc_engine::Level)> = Vec::new();
    for (i, l) in plan.bull_targets.iter().enumerate() {
        rows.push((format!("多头目标{}", i + 1), l));
    }
    for (i, l) in plan.bear_targets.iter().enumerate() {
        rows.push((format!("空头目标{}", i + 1), l));
    }
    if let Some(s) = &plan.major_long_support {
        rows.push(("核心多头防守".to_string(), s));
    }
    if let Some(s) = &plan.squeeze_zone {
        rows.push(("Squeeze Zone".to_string(), s));
    }
    rows.sort_by(|a, b| b.1.level.partial_cmp(&a.1.level).unwrap());
    for (label, l) in rows {
        let band = l
            .band
            .map(|(lo, hi)| format!("{lo:.2}–{hi:.2}"))
            .unwrap_or("—".into());
        let dem = l
            .distance_em
            .map(|d| format!("{d:.2}×"))
            .unwrap_or("—".into());
        let _ = writeln!(
            out,
            "| {label} | {:.2} | {} | {} | {:.3} | {} |",
            l.level,
            band,
            l.types.join(", "),
            l.score,
            dem,
        );
    }
    if let Some((lo, hi)) = plan.negative_gamma_band {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "负 Gamma 带（现价±1EM 内）：{lo:.2}–{hi:.2}（共 {} 档）",
            plan.negative_gamma_strikes_near_spot.len()
        );
    }
    let _ = writeln!(out);

    // 判定依据
    let _ = writeln!(out, "## 判定依据");
    let _ = writeln!(out);
    if !plan.regime_prob.is_empty() {
        let prob: Vec<String> = plan
            .regime_prob
            .iter()
            .map(|(k, v)| format!("{k} {}", pct(*v)))
            .collect();
        let _ = writeln!(out, "概率先验：{}", prob.join(" / "));
        let _ = writeln!(out);
    }
    for n in &plan.regime_notes {
        let _ = writeln!(out, "- {n}");
    }
    let _ = writeln!(out);

    // Delta & Flow
    let dd = &plan.delta_detail;
    if !dd.is_null() {
        let _ = writeln!(out, "## Delta 与资金流（订单流看板）");
        let _ = writeln!(out);
        let _ = writeln!(out, "| 指标 | 0DTE | 1DTE+ | 净额 |");
        let _ = writeln!(out, "|---|---|---|---|");
        let g = |k: &str| dd.get(k).and_then(|v| v.as_f64());
        let rows = [
            ("DEX", "dex_0dte", "dex_1dte", "net_dex"),
            ("Call DEX", "call_dex_0dte", "call_dex_1dte", "net_call_dex"),
            ("Put DEX", "put_dex_0dte", "put_dex_1dte", "net_put_dex"),
            ("CVR", "cvr_0dte", "cvr_1dte", ""),
            ("GEX 比值", "gex_ratio_0dte", "gex_ratio_1dte", ""),
            ("Vanna", "vanna_0dte", "vanna_1dte", ""),
            ("Charm", "charm_0dte", "charm_1dte", ""),
        ];
        for (label, z, o, n) in rows {
            let net = if n.is_empty() {
                "—".into()
            } else {
                fmt(g(n))
            };
            let _ = writeln!(out, "| {label} | {} | {} | {} |", fmt(g(z)), fmt(g(o)), net);
        }
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "流方向：dexoflow {} / gexoflow {} / cvroflow {}",
            fmt(g("dexoflow")),
            fmt(g("gexoflow")),
            fmt(g("cvroflow"))
        );
        let _ = writeln!(out);
    }

    // 条件式文案
    let _ = writeln!(out, "## 盘前判断");
    let _ = writeln!(out);
    for line in &plan.narrative {
        let _ = writeln!(out, "- {line}");
    }
    let _ = writeln!(out);

    // 数据质量与血缘
    let _ = writeln!(out, "## 数据质量与血缘");
    let _ = writeln!(out);
    for n in &plan.lineage_notes {
        let _ = writeln!(out, "- {n}");
    }
    if let Some(p) = &plan.parity {
        let _ = writeln!(
            out,
            "- Parity 合成远期 {:.2}（{} 对，离散 {:.2}）{}",
            p.synthetic_forward,
            p.pairs,
            p.dispersion,
            if p.rejected {
                " → 离散超限，链内衍生指标已拒绝"
            } else {
                ""
            }
        );
    }
    for w in &plan.data_warnings {
        let _ = writeln!(out, "- ⚠️ {w}");
    }
    let _ = writeln!(out);

    // 模型局限
    let _ = writeln!(out, "## 模型局限");
    let _ = writeln!(out);
    for l in &plan.limitations {
        let _ = writeln!(out, "- {l}");
    }
    for l in [
        "公开 OI ≠ dealer 实际持仓（文档 §4.3）",
        "dealer 方向假设、GEX 符号约定、期限权重未经历史样本验证（文档 §15）",
        "Squeeze Zone 为推断性定义；路径叙事不纳入评分（§20.4）",
        "Basis 为 TradingHub 内嵌值，同步性未独立验证（§6.1 要求同步报价）",
    ] {
        let _ = writeln!(out, "- {l}");
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "---\n*由 thc {} 生成 · 盘前一次性快照，无盘中重校准*",
        env!("CARGO_PKG_VERSION")
    );
    out
}
