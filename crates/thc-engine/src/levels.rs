//! 价位生成：候选 → 聚类 → 评分 → 目标/核心防守/Squeeze（文档 §7/8/9）。
//! 1:1 移植自 levels.py；硬编码权重改为 EngineConfig 注入（文档 §8 要求可验证）。

use crate::EngineConfig;
use crate::types::{Level, OptionStructure, Technicals};
use std::collections::{BTreeMap, BTreeSet};

/// 某行权价的 |GEX|（从 gamma_ladder 最近档取，用于评分强度）。
fn gex_abs_at(opt: &OptionStructure, level: f64) -> f64 {
    opt.gamma_ladder
        .iter()
        .min_by(|a, b| {
            (a.strike - level)
                .abs()
                .partial_cmp(&(b.strike - level).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| r.current_value.unwrap_or(0.0).abs())
        .unwrap_or(0.0)
}

/// 按 |current_value| 取前 N 个 Gamma 行权价；side 可选 "positive"/"negative"。
pub fn top_gamma_strikes(
    opt: &OptionStructure,
    n: usize,
    side: Option<&str>,
) -> Vec<crate::types::LadderPoint> {
    let mut rows: Vec<_> = opt
        .gamma_ladder
        .iter()
        .filter(|r| side.is_none() || r.side.as_deref() == side)
        .cloned()
        .collect();
    rows.sort_by(|a, b| {
        b.current_value
            .unwrap_or(0.0)
            .abs()
            .partial_cmp(&a.current_value.unwrap_or(0.0).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(n);
    rows
}

#[derive(Debug, Clone)]
struct Cand {
    level: f64,
    typ: String,
    gex_abs: f64,
}

fn build_candidates(opt: &OptionStructure, em: Option<f64>, tech: &Technicals) -> Vec<Cand> {
    let mut cands = Vec::new();
    let mut add = |level: Option<f64>, typ: &str, boost: f64| {
        if let Some(l) = level {
            cands.push(Cand {
                level: l,
                typ: typ.to_string(),
                gex_abs: gex_abs_at(opt, l) * boost,
            });
        }
    };

    add(opt.flip, "gamma_flip", 1.0);
    add(opt.call_wall_0dte, "call_wall(0DTE)", 1.5);
    add(opt.call_wall_1dte, "call_wall(1DTE+)", 1.5);
    add(opt.put_wall_0dte, "put_wall(0DTE)", 1.5);
    add(opt.put_wall_1dte, "put_wall(1DTE+)", 1.5);
    add(opt.major_long_gamma, "major_long_gamma", 1.0);
    add(opt.major_short_gamma, "major_short_gamma", 1.0);
    add(opt.max_pos_oi, "max_pos_oi", 1.3);
    add(opt.max_neg_oi, "max_neg_oi", 1.3);
    add(opt.max_pos_vol, "max_pos_vol", 1.0);
    add(opt.max_neg_vol, "max_neg_vol", 1.0);
    for r in top_gamma_strikes(opt, 6, None) {
        add(Some(r.strike), "top_gamma_strike", 1.0);
    }
    if let (Some(em), Some(spot)) = (em, opt.spot) {
        add(Some(spot + em), "upper_em", 1.0);
        add(Some(spot - em), "lower_em", 1.0);
    }
    add(tech.vwap, "VWAP", 1.0);
    add(tech.poc, "POC", 1.0);
    add(tech.pdh, "PDH", 1.0);
    add(tech.pdl, "PDL", 1.0);
    add(tech.onh, "ONH", 1.0);
    add(tech.onl, "ONL", 1.0);
    add(tech.prior_pivot, "PRIOR_PIVOT", 1.0);
    cands
}

#[derive(Debug, Clone)]
struct Cluster {
    mean: f64,
    levels: Vec<f64>,
    members: Vec<Cand>,
    gex_abs: f64,
    // 输出用
    types: Vec<String>,
    level: f64,
    band: Option<(f64, f64)>,
    score: f64,
    score_detail: BTreeMap<String, f64>,
}

fn cluster_levels(mut cands: Vec<Cand>, tol: f64) -> Vec<Cluster> {
    cands.sort_by(|a, b| {
        a.level
            .partial_cmp(&b.level)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut clusters: Vec<Cluster> = Vec::new();
    for c in cands {
        if let Some(cl) = clusters.last_mut()
            && (c.level - cl.mean).abs() <= tol
        {
            cl.levels.push(c.level);
            cl.gex_abs = cl.gex_abs.max(c.gex_abs);
            cl.mean = cl.levels.iter().sum::<f64>() / cl.levels.len() as f64;
            cl.members.push(c);
            continue;
        }
        clusters.push(Cluster {
            mean: c.level,
            levels: vec![c.level],
            members: vec![c.clone()],
            gex_abs: c.gex_abs,
            types: vec![],
            level: 0.0,
            band: None,
            score: 0.0,
            score_detail: BTreeMap::new(),
        });
    }
    for cl in &mut clusters {
        let mut set: Vec<String> = cl.members.iter().map(|m| m.typ.clone()).collect();
        let unique: BTreeSet<String> = set.drain(..).collect();
        cl.types = unique.into_iter().collect();
        cl.level = (cl.mean * 100.0).round() / 100.0;
        cl.band = if cl.levels.len() > 1 {
            let lo = cl.levels.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = cl.levels.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Some((r2(lo), r2(hi)))
        } else {
            None
        };
    }
    clusters
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// 评分（文档 §8）。无可用项的权重重分配到已选项。
fn score_cluster(
    cl: &Cluster,
    price: f64,
    em: Option<f64>,
    max_gex: f64,
    has_tech: bool,
    cfg: &EngineConfig,
) -> (f64, BTreeMap<String, f64>) {
    let dist = (cl.level - price).abs();
    let (prox, em_align) = if let Some(em) = em {
        let prox = if 0.3 * em <= dist && dist <= 1.0 * em {
            1.0
        } else {
            (1.0 - (dist - 0.65 * em).abs() / em).max(0.0)
        };
        let em_align = if dist <= 1.2 * em {
            1.0
        } else {
            (1.0 - (dist - 1.2 * em) / em).max(0.0)
        };
        (prox, Some(em_align))
    } else {
        let prox = if (5.0..=25.0).contains(&dist) {
            1.0
        } else {
            (1.0 - (dist - 15.0).abs() / 30.0).max(0.0)
        };
        (prox, None)
    };
    let opt_strength = if max_gex > 0.0 {
        cl.gex_abs / max_gex
    } else {
        0.0
    };
    let tech_conf = if has_tech
        && cl.types.iter().any(|t| {
            matches!(
                t.as_str(),
                "VWAP" | "POC" | "PDH" | "PDL" | "ONH" | "ONL" | "PRIOR_PIVOT"
            )
        }) {
        Some(1.0)
    } else {
        None
    };

    let mut items: Vec<(&str, f64, f64)> = vec![
        ("options", cfg.weight_options_strength, opt_strength),
        ("proximity", cfg.weight_proximity, prox),
    ];
    if let Some(v) = em_align {
        items.push(("em_align", cfg.weight_expected_move_alignment, v));
    }
    if let Some(v) = tech_conf {
        items.push(("tech", cfg.weight_technical_confluence, v));
    }
    let total_w: f64 = items.iter().map(|(_, w, _)| w).sum();
    let score: f64 = items.iter().map(|(_, w, v)| v * (w / total_w)).sum();
    let detail = items
        .into_iter()
        .map(|(k, _, v)| (k.to_string(), (v * 1000.0).round() / 1000.0))
        .collect();
    ((score * 1000.0).round() / 1000.0, detail)
}

fn brief(cl: &Cluster, price: f64, em: Option<f64>) -> Level {
    Level {
        level: cl.level,
        types: cl.types.clone(),
        score: cl.score,
        band: cl.band,
        distance_em: em.map(|e| ((cl.level - price).abs() / e * 100.0).round() / 100.0),
        rationale: vec![],
    }
}

pub struct LevelsOut {
    pub bull_targets: Vec<Level>,
    pub bear_targets: Vec<Level>,
    pub major_long_support: Option<Level>,
    pub squeeze_zone: Option<Level>,
    pub negative_gamma_band: Option<(f64, f64)>,
    pub negative_gamma_strikes_near_spot: Vec<f64>,
    pub negative_gamma_total: usize,
    pub tolerance: f64,
    pub all_clusters_n: usize,
}

pub fn build_levels(
    opt: &OptionStructure,
    price: f64,
    em: Option<f64>,
    tech: &Technicals,
    cfg: &EngineConfig,
) -> LevelsOut {
    let cands = build_candidates(opt, em, tech);
    let tol = em
        .map(|e| (cfg.cluster_tolerance_em_coef * e).max(1.0))
        .unwrap_or(1.0);
    let mut clusters = cluster_levels(cands, tol);
    let has_tech = [tech.vwap, tech.poc, tech.pdh, tech.pdl, tech.onh, tech.onl]
        .iter()
        .any(|v| v.is_some());
    let all_cands: Vec<Cand> = clusters.iter().flat_map(|c| c.members.clone()).collect();
    let max_gex = all_cands.iter().map(|c| c.gex_abs).fold(0.0_f64, f64::max);
    for cl in &mut clusters {
        let (s, d) = score_cluster(cl, price, em, max_gex, has_tech, cfg);
        cl.score = s;
        cl.score_detail = d;
    }

    let mut above: Vec<Cluster> = clusters
        .iter()
        .filter(|c| c.level > price)
        .cloned()
        .collect();
    above.sort_by(|a, b| a.level.partial_cmp(&b.level).unwrap());
    let mut below: Vec<Cluster> = clusters
        .iter()
        .filter(|c| c.level < price)
        .cloned()
        .collect();
    below.sort_by(|a, b| b.level.partial_cmp(&a.level).unwrap());

    // gamma_flip 是天然的多空参考线（结构参考位）
    let flip_level = all_cands
        .iter()
        .find(|c| c.typ == "gamma_flip")
        .map(|c| c.level)
        .or_else(|| {
            clusters
                .iter()
                .find(|c| c.types.iter().any(|t| t == "gamma_flip"))
                .map(|c| c.level)
        });
    let upper_em_level = em.map(|e| price + e);
    let lower_em_level = em.map(|e| price - e);

    /// 按评分取 top n，保证至少一个目标在 far_threshold 远端（目标多样性约束）。
    fn diverse_top_n(
        rows: &[Cluster],
        n: usize,
        far_threshold: Option<f64>,
        far_fallback: Option<f64>,
        upper: bool,
        price: f64,
        em: Option<f64>,
    ) -> Vec<Level> {
        let mut picked: Vec<Cluster> = if rows.len() <= n {
            rows.to_vec()
        } else {
            let mut s = rows.to_vec();
            s.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
            s.truncate(n);
            s
        };
        let has_far = far_threshold.is_none()
            || picked.iter().any(|c| {
                if upper {
                    c.level > far_threshold.unwrap()
                } else {
                    c.level < far_threshold.unwrap()
                }
            });
        if !has_far && picked.len() >= 2 {
            let far_pool: Vec<Cluster> = rows
                .iter()
                .filter(|c| !picked.iter().any(|p| (p.level - c.level).abs() < 1e-9))
                .filter(|c| {
                    let t = far_threshold.unwrap();
                    if upper { c.level > t } else { c.level < t }
                })
                .cloned()
                .collect();
            if let Some(best_far) = far_pool
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
            {
                if let Some(min_idx) = picked[1..]
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.score.partial_cmp(&b.1.score).unwrap())
                    .map(|(i, _)| i + 1)
                {
                    picked.remove(min_idx);
                    picked.push(best_far.clone());
                }
            } else if let Some(fb) = far_fallback {
                let synthetic = Cluster {
                    mean: r2(fb),
                    levels: vec![r2(fb)],
                    members: vec![],
                    gex_abs: 0.0,
                    types: vec![if upper { "upper_em" } else { "lower_em" }.to_string()],
                    level: r2(fb),
                    band: None,
                    score: 0.3,
                    score_detail: BTreeMap::new(),
                };
                if picked.len() >= 3
                    && let Some(min_idx) = picked[1..]
                        .iter()
                        .enumerate()
                        .min_by(|a, b| a.1.score.partial_cmp(&b.1.score).unwrap())
                        .map(|(i, _)| i + 1)
                {
                    picked.remove(min_idx);
                }
                picked.push(synthetic);
            }
        }
        picked.sort_by(|a, b| {
            (a.level - price)
                .abs()
                .partial_cmp(&(b.level - price).abs())
                .unwrap()
        });
        picked.iter().map(|c| brief(c, price, em)).collect()
    }

    let bull_far = flip_level.or(em.map(|e| price + 0.5 * e));
    let bear_far = flip_level.or(em.map(|e| price - 0.5 * e));
    let bull_targets = diverse_top_n(&above, 3, bull_far, upper_em_level, true, price, em);
    let bear_targets = diverse_top_n(&below, 3, bear_far, lower_em_level, false, price, em);

    // 核心多头防守：下方有 put 支撑特征、距现价足够远的强簇
    let is_put_support = |c: &Cluster| {
        c.types.iter().any(|t| {
            t.starts_with("put_wall")
                || t.contains("neg_oi")
                || t == "lower_em"
                || t == "max_neg_vol"
        })
    };
    let dist_thresh = em.map(|e| 0.5 * e).unwrap_or(8.0);
    let support_pool: Vec<Cluster> = below
        .iter()
        .filter(|c| is_put_support(c) && (c.level - price).abs() >= dist_thresh)
        .cloned()
        .collect();
    let major_long_support = support_pool
        .iter()
        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        .map(|c| brief(c, price, em));

    // Squeeze：核心防守下方、落在负 Gamma 区或 lower_em 的最近强簇
    let neg_lo = opt
        .neg_gamma_strikes
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let neg_lo = neg_lo.is_finite().then_some(neg_lo);
    let mut squeeze: Option<Level> = None;
    if let Some(sup) = &major_long_support {
        let below_support: Vec<&Cluster> = below.iter().filter(|c| c.level < sup.level).collect();
        if let Some(best) = below_support
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        {
            squeeze = Some(brief(best, price, em));
        }
    }
    if squeeze.is_none()
        && let Some(le) = lower_em_level
    {
        squeeze = Some(Level {
            level: r2(le),
            types: vec!["lower_em".into()],
            score: 0.0,
            band: None,
            distance_em: em.map(|e| ((le - price).abs() / e * 100.0).round() / 100.0),
            rationale: vec!["下侧 EM 边界".into()],
        });
    }
    if let Some(sq) = &mut squeeze {
        if neg_lo.is_some_and(|n| sq.level <= n) {
            sq.rationale.push("位于负 Gamma 区".into());
        }
        sq.rationale
            .push("支撑失守后的波动/流动性加速区（非 dealer 必卖）".into());
    }

    // 负 Gamma 带：只取现价 ±1EM 内的负 Gamma 行权价
    let neg_all = &opt.neg_gamma_strikes;
    let (lo, hi) = em
        .map(|e| (price - e, price + e))
        .unwrap_or((price - 25.0, price + 25.0));
    let mut neg_near: Vec<f64> = neg_all
        .iter()
        .cloned()
        .filter(|s| *s >= lo && *s <= hi)
        .collect();
    neg_near.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let neg_band = if neg_near.is_empty() {
        None
    } else {
        Some((r2(neg_near[0]), r2(*neg_near.last().unwrap())))
    };

    LevelsOut {
        bull_targets,
        bear_targets,
        major_long_support,
        squeeze_zone: squeeze,
        negative_gamma_band: neg_band,
        negative_gamma_strikes_near_spot: neg_near.iter().map(|s| r2(*s)).collect(),
        negative_gamma_total: neg_all.len(),
        tolerance: r2(tol),
        all_clusters_n: clusters.len(),
    }
}
