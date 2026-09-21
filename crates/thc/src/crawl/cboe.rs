//! CBOE 免费数据（延时 15 分钟，免 key 免登录）。移植自 legacy/market_data.py。
// Phase 3 引擎接入后才有调用方，暂豁免 dead_code。
#![allow(dead_code)]
//! - VIX 家族现价：VIX / VIX1D / VIX9D / VVIX / SKEW
//! - SPX 月度期权链 → 最近到期 ATM Straddle → 0DTE EM（√T 近似）
//!   注：SPXW（真 0DTE）该端点返回 403，故 EM 用月度 straddle 的 √T 近似。

use jiff::civil::Date;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::Value;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";
const QUOTE_BASE: &str = "https://cdn.cboe.com/api/global/delayed_quotes/quotes/_{sym}.json";
const SPX_OPTIONS: &str = "https://cdn.cboe.com/api/global/delayed_quotes/options/_SPX.json";
const VIX_FAMILY: [&str; 5] = ["VIX", "VIX1D", "VIX9D", "VVIX", "SKEW"];

#[derive(Debug, Clone, serde::Serialize)]
pub struct VixQuote {
    pub value: Option<f64>,
    pub change: Option<f64>,
    pub timestamp: Option<String>,
}

/// CBOE 期权链上的一档合约（OCC 符号解析后）。
#[derive(Debug, Clone)]
pub struct CboeOption {
    pub expiry: Date,
    pub is_call: bool,
    pub strike: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub iv: Option<f64>,
    pub open_interest: Option<f64>,
    pub volume: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CboeChain {
    pub spot: Option<f64>,
    pub timestamp: Option<String>,
    #[serde(skip)]
    pub options: Vec<CboeOption>,
    pub contracts: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmInfo {
    pub em_0dte_spx: f64,
    pub method: String,
    pub source_expiry: String,
    pub source_straddle: f64,
    pub atm_strike: f64,
    pub atm_iv: Option<f64>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct MarketData {
    pub source: String,
    pub vix_family: std::collections::BTreeMap<String, VixQuote>,
    pub em: Option<EmInfo>,
    pub cboe_spot: Option<f64>,
    pub cboe_timestamp: Option<String>,
    pub spx_chain_contracts: usize,
    pub warnings: Vec<String>,
    /// 原始链保留给 engine（parity/straddle 计算），不随 JSON 输出
    #[serde(skip)]
    pub chain: Option<CboeChain>,
}

pub struct Cboe {
    http: reqwest::Client,
}

impl Default for Cboe {
    fn default() -> Self {
        Self::new()
    }
}

impl Cboe {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("reqwest client build");
        Self { http }
    }

    /// 并发抓 VIX 家族；单个失败不阻塞其它（记 warnings）。
    pub async fn vix_family(&self) -> (Vec<(String, VixQuote)>, Vec<String>) {
        let futs = VIX_FAMILY.iter().map(|sym| {
            let http = self.http.clone();
            let url = QUOTE_BASE.replace("{sym}", sym);
            let sym = sym.to_string();
            async move {
                let r = get_json(&http, &url).await;
                (sym, r)
            }
        });
        let results = futures_join_all(futs).await;
        let mut quotes = Vec::new();
        let mut warnings = Vec::new();
        for (sym, r) in results {
            match r {
                Ok(data) => {
                    let d = data.get("data").cloned().unwrap_or(Value::Null);
                    quotes.push((
                        sym,
                        VixQuote {
                            value: d.get("current_price").and_then(Value::as_f64),
                            change: d.get("price_change").and_then(Value::as_f64),
                            timestamp: data
                                .get("timestamp")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        },
                    ));
                }
                Err(e) => warnings.push(format!("{sym}: {e}")),
            }
        }
        (quotes, warnings)
    }

    pub async fn spx_chain(&self) -> Result<CboeChain, String> {
        let data = get_json(&self.http, SPX_OPTIONS).await?;
        let d = data.get("data").cloned().unwrap_or(Value::Null);
        let mut options = Vec::new();
        if let Some(arr) = d.get("options").and_then(Value::as_array) {
            for o in arr {
                let Some(sym) = o.get("option").and_then(Value::as_str) else {
                    continue;
                };
                let Some((expiry, is_call, strike)) = parse_occ(sym) else {
                    continue;
                };
                options.push(CboeOption {
                    expiry,
                    is_call,
                    strike,
                    bid: o.get("bid").and_then(Value::as_f64),
                    ask: o.get("ask").and_then(Value::as_f64),
                    iv: o.get("iv").and_then(Value::as_f64),
                    open_interest: o.get("open_interest").and_then(Value::as_f64),
                    volume: o.get("volume").and_then(Value::as_f64),
                });
            }
        }
        let spot = d
            .get("current_price")
            .and_then(Value::as_f64)
            .or_else(|| d.get("close").and_then(Value::as_f64));
        let contracts = options.len();
        Ok(CboeChain {
            spot,
            timestamp: data
                .get("timestamp")
                .and_then(Value::as_str)
                .map(str::to_string),
            options,
            contracts,
        })
    }

    /// 汇总：VIX 家族 + 链 + EM，任一失败降级不抛。
    pub async fn market_data(&self, want_em: bool) -> MarketData {
        let (vix_f, chain_r) = if want_em {
            tokio::join!(self.vix_family(), self.spx_chain())
        } else {
            (self.vix_family().await, Err("skipped".to_string()))
        };
        let (vix_vec, mut warnings) = vix_f;
        let mut md = MarketData {
            source: "CBOE delayed (15min)".into(),
            vix_family: vix_vec.into_iter().collect(),
            ..Default::default()
        };
        warnings.extend(md.warnings.drain(..).collect::<Vec<_>>());
        md.warnings = warnings;

        match chain_r {
            Ok(chain) => {
                md.cboe_spot = chain.spot;
                md.cboe_timestamp = chain.timestamp.clone();
                md.spx_chain_contracts = chain.contracts;
                md.em = chain
                    .nearest_atm_straddle(et_today())
                    .and_then(|s| s.em_0dte());
                md.chain = Some(chain);
            }
            Err(e) if want_em => md.warnings.push(format!("SPX chain: {e}")),
            _ => {}
        }
        md
    }
}

/// 带 UA 的 GET JSON。
async fn get_json(http: &reqwest::Client, url: &str) -> Result<Value, String> {
    let resp = http
        .get(url)
        .header(USER_AGENT, UA)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

/// 解析 OCC 符号：SPX260821C07730000 → (expiry, is_call, strike)
pub fn parse_occ(symbol: &str) -> Option<(Date, bool, f64)> {
    let s: String = symbol.chars().filter(|c| !c.is_whitespace()).collect();
    // root(字母) + YYMMDD + C/P + 8位strike(千分之一)
    let root_end = s.find(|c: char| c.is_ascii_digit())?;
    let digits = &s[root_end..];
    if digits.len() != 15 {
        return None;
    }
    let yy: i16 = digits[0..2].parse().ok()?;
    let mm: i8 = digits[2..4].parse().ok()?;
    let dd: i8 = digits[4..6].parse().ok()?;
    let cp = digits.as_bytes()[6];
    let strike: f64 = digits[7..15].parse::<i64>().ok()? as f64 / 1000.0;
    let expiry = Date::new(2000 + yy, mm, dd).ok()?;
    Some((expiry, cp == b'C', strike))
}

/// 最近到期 ATM straddle 的中间量。
pub struct Straddle {
    pub expiry: Date,
    pub dte: i32,
    strike: f64,
    call_mid: f64,
    put_mid: f64,
    iv_atm: Option<f64>,
}

impl Straddle {
    /// 月度 straddle → 0DTE EM = straddle/√DTE（flat term structure 近似）
    fn em_0dte(&self) -> Option<EmInfo> {
        if self.dte < 1 {
            return None;
        }
        let straddle = self.call_mid + self.put_mid;
        Some(EmInfo {
            em_0dte_spx: ((straddle / (self.dte as f64).sqrt()) * 100.0).round() / 100.0,
            method: format!("monthly ATM straddle / √DTE (DTE={})", self.dte),
            source_expiry: self.expiry.to_string(),
            source_straddle: (straddle * 100.0).round() / 100.0,
            atm_strike: self.strike,
            atm_iv: self.iv_atm,
        })
    }
}

impl CboeChain {
    /// 最近到期 ATM（最接近 spot 的 strike 的 call+put mid 之和）。
    pub fn nearest_atm_straddle(&self, as_of: Date) -> Option<Straddle> {
        let spot = self.spot?;
        let expiry = self
            .options
            .iter()
            .filter(|o| o.expiry >= as_of)
            .map(|o| o.expiry)
            .min()?;
        let dte = as_of.until(expiry).map(|s| s.get_days()).ok()?;
        let valid: Vec<&CboeOption> = self
            .options
            .iter()
            .filter(|o| {
                o.expiry == expiry
                    && o.bid.is_some_and(|b| b > 0.0)
                    && o.ask.is_some_and(|a| a > 0.0)
            })
            .collect();
        let atm = |is_call: bool| {
            valid
                .iter()
                .filter(|o| o.is_call == is_call)
                .min_by(|a, b| {
                    (a.strike - spot)
                        .abs()
                        .partial_cmp(&(b.strike - spot).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        };
        let (c, p) = (atm(true)?, atm(false)?);
        Some(Straddle {
            expiry,
            dte: dte.max(1),
            strike: c.strike,
            call_mid: (c.bid? + c.ask?) / 2.0,
            put_mid: (p.bid? + p.ask?) / 2.0,
            iv_atm: c.iv,
        })
    }
}

/// 并发 join_all 的极简实现（不引 futures crate）。
async fn futures_join_all<F, T>(futs: impl IntoIterator<Item = F>) -> Vec<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<_> = futs.into_iter().map(tokio::spawn).collect();
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(v) = h.await {
            out.push(v);
        }
    }
    out
}

/// 当前 ET 日期。
fn et_today() -> Date {
    jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::get("America/New_York").expect("ET tz"))
        .date()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_occ_basic() {
        let (expiry, is_call, strike) = parse_occ("SPX260821C07730000").unwrap();
        assert_eq!(expiry.to_string(), "2026-08-21");
        assert!(is_call);
        assert_eq!(strike, 7730.0);
        let (_, is_call2, _) = parse_occ("SPXW260918P07650000").unwrap();
        assert!(!is_call2);
        assert!(parse_occ("garbage").is_none());
    }
}
