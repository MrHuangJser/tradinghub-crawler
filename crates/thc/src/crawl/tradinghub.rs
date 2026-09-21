//! TradingHub 客户端：登录 → session Cookie → 双接口。
//! 逆向细节见 docs/ANALYSIS.md。

use super::FetchError;
use super::payload::*;
use jiff::{Timestamp, tz::TimeZone};
use reqwest::header::{ACCEPT, CACHE_CONTROL, HeaderValue, REFERER, SET_COOKIE};
use serde_json::Value;
use std::time::Duration;

const BASE: &str = "https://tradinghubs.org";
const LOGIN_URL: &str = "https://tradinghubs.org/api/auth/login";
const GEX_URL: &str = "https://tradinghubs.org/beta-test/api/gex/live-data";
const EXPOSURE_URL: &str = "https://tradinghubs.org/beta-test/api/options-data/exposure";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";

pub struct Client {
    http: reqwest::Client,
    /// 登录后持有的会话 Cookie 值（手动携带，不用 cookie jar）
    session_cookie: tokio::sync::OnceCell<String>,
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(UA)
            // CF 边缘对 HTTP/2 大响应体 + br 解码有抽风行为（实测 2026-09-21
            // "error decoding response body"），降到 HTTP/1.1 + gzip/deflate
            // 与 Python requests 同口径；禁用 idle 连接池，每次新建连接避开
            // CF 对复用连接的间歇性截断。
            .http1_only()
            .pool_max_idle_per_host(0)
            .build()
            .expect("reqwest client build");
        Self {
            http,
            session_cookie: tokio::sync::OnceCell::new(),
        }
    }

    /// 登录。凭据缺失 → Auth(2)；登录失败 → Auth(2)；网络 → Http(3)。
    pub async fn login(&self, email: &str, password: &str) -> Result<(), FetchError> {
        if email.is_empty() || password.is_empty() {
            return Err(FetchError::Auth(
                "未配置凭据：config.toml [tradinghub] 或环境变量 \
                 TRADINGHUB_EMAIL/TRADINGHUB_PASSWORD"
                    .into(),
            ));
        }
        let resp = self
            .http
            .post(LOGIN_URL)
            .header(REFERER, format!("{BASE}/account/login"))
            .header("Origin", BASE)
            .json(&serde_json::json!({"email": email.trim(), "password": password}))
            .send()
            .await
            .map_err(|e| FetchError::Http(format!("登录请求失败: {e}")))?;

        let session = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v: &HeaderValue| v.to_str().ok())
            .find(|s| s.starts_with("tradinghub_user_session="))
            .and_then(|s| s.split(';').next())
            .map(str::to_string);
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| FetchError::Http(format!("登录响应非 JSON: {e}")))?;

        if body.get("ok").and_then(Value::as_bool) != Some(true) {
            let msg = body
                .get("error")
                .or_else(|| body.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(FetchError::Auth(format!("登录失败：{msg}")));
        }
        let Some(cookie) = session else {
            return Err(FetchError::Auth(
                "登录返回 ok 但未下发 tradinghub_user_session Cookie".into(),
            ));
        };
        let _ = self.session_cookie.set(cookie);
        Ok(())
    }

    /// 拉取两个接口（并发），返回保留原始 JSON 的合并体。
    pub async fn fetch_merged(&self) -> Result<Merged, FetchError> {
        let (gex, exp) = tokio::try_join!(
            self.get_json(GEX_URL, "/beta-test/OptionsDataViewer"),
            self.get_json(EXPOSURE_URL, "/beta-test/OptionsDataViewer"),
        )?;
        Merged::new(gex, exp)
    }

    /// GET JSON，传输层错误（连接/解码）重试至多 3 次——CF 边缘对复用连接
    /// 的大响应体偶发截断，重试通常一次即过。HTTP/接口错误不重试。
    async fn get_json(&self, url: &str, referer: &str) -> Result<Value, FetchError> {
        let url = format!("{url}?v={}", Timestamp::now().as_millisecond());
        let cookie = self
            .session_cookie
            .get()
            .cloned()
            .ok_or_else(|| FetchError::Auth("尚未登录".into()))?;

        let mut last_err = String::new();
        for attempt in 1..=3 {
            let resp = match self
                .http
                .get(&url)
                .header(reqwest::header::COOKIE, &cookie)
                .header(ACCEPT, "*/*")
                .header(REFERER, format!("{BASE}{referer}"))
                .header(CACHE_CONTROL, "no-cache")
                .header("Pragma", "no-cache")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = format!("{url}: {e}");
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        continue;
                    }
                    return Err(FetchError::Http(last_err));
                }
            };
            let status = resp.status();
            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    last_err = format!("{url}: 读取响应体失败: {e:#}");
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        continue;
                    }
                    return Err(FetchError::Http(last_err));
                }
            };
            let data: Value = serde_json::from_slice(&bytes).map_err(|e| {
                let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
                FetchError::Http(format!(
                    "{url}: HTTP {status}, JSON 解析失败 ({e}); body 头部: {head}"
                ))
            })?;
            if !status.is_success() {
                return Err(FetchError::Http(format!("{url}: HTTP {status}")));
            }
            if data.get("ok").and_then(Value::as_bool) == Some(false) {
                let msg = data
                    .get("error")
                    .or_else(|| data.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                return Err(FetchError::Http(format!("{url}: 接口返回错误 {msg}")));
            }
            return Ok(data);
        }
        Err(FetchError::Http(last_err))
    }
}

/// 两接口合并体：保留原始 Value（--raw 无损），同时提供强类型视图。
pub struct Merged {
    gex_raw: Value,
    exposure_raw: Value,
    gex: GexResponse,
    exposure: ExposureResponse,
}

impl Merged {
    fn new(gex_raw: Value, exposure_raw: Value) -> Result<Self, FetchError> {
        let gex = serde_json::from_value(gex_raw.clone())
            .map_err(|e| FetchError::SchemaDrift(format!("gex/live-data: {e}")))?;
        let exposure = serde_json::from_value(exposure_raw.clone())
            .map_err(|e| FetchError::SchemaDrift(format!("options-data/exposure: {e}")))?;
        Ok(Self {
            gex_raw,
            exposure_raw,
            gex,
            exposure,
        })
    }

    /// --raw 输出：两接口原始合并 JSON。
    pub fn raw(&self) -> Value {
        serde_json::json!({"gex": self.gex_raw, "exposure": self.exposure_raw})
    }

    pub fn tickers(&self) -> Vec<String> {
        let p = &self.gex.primary;
        if !p.tickers.is_empty() {
            return p.tickers.clone();
        }
        p.levels.keys().cloned().collect()
    }

    /// 抽出指定标的的视图（对应 legacy extract_ticker）。
    pub fn view(&self, ticker: &str) -> Result<TickerView, FetchError> {
        let t = ticker.to_uppercase();
        let g = &self.gex.primary;
        let e = &self.exposure.primary;

        let levels = g.levels.get(&t).cloned();
        let view = TickerView {
            ticker: t.clone(),
            generated_at: self.gex.generated_at.clone(),
            last_updated_at: self.gex.last_updated_at.clone(),
            stale: self.gex.stale,
            spot: levels.as_ref().map(|l| l.spot),
            captured_at: levels.as_ref().map(|l| fmt_et(l.timestamp)),
            captured_ts: levels.as_ref().map(|l| l.timestamp),
            levels_summary: levels,
            gamma_ladder: g.gex_proxy.get(&t).cloned(),
            classic_chain: g.classic_chain.get(&t).cloned(),
            state_greeks: g.state_greeks.get(&t).cloned(),
            orderflow: g.orderflow.get(&t).cloned(),
            exposure: e.exposure.get(&t).or_else(|| g.exposure.get(&t)).cloned(),
            dte_exposure: DteExposure {
                gex: DteSet {
                    zero: g.gex_zero.get(&t).cloned(),
                    one: g.gex_one.get(&t).cloned(),
                    net: g.gex_net.get(&t).cloned(),
                },
                dex: DteSet {
                    zero: g.dex_zero.get(&t).cloned(),
                    one: g.dex_one.get(&t).cloned(),
                    net: g.dex_net.get(&t).cloned(),
                },
                vex: DteSet {
                    zero: g.vex_zero.get(&t).cloned(),
                    one: g.vex_one.get(&t).cloned(),
                    net: g.vex_net.get(&t).cloned(),
                },
                chex: DteSet {
                    zero: g.chex_zero.get(&t).cloned(),
                    one: g.chex_one.get(&t).cloned(),
                    net: g.chex_net.get(&t).cloned(),
                },
            },
        };

        if view.levels_summary.is_none() && view.exposure.is_none() {
            return Err(FetchError::NoData {
                ticker: t,
                available: self.tickers(),
            });
        }
        Ok(view)
    }
}

/// Unix 秒 → "YYYY-MM-DD HH:MM:SS ET"（沿用旧工具口径）。
fn fmt_et(ts: i64) -> String {
    Timestamp::from_second(ts)
        .ok()
        .map(|t| t.to_zoned(TimeZone::get("America/New_York").expect("ET tz")))
        .map(|z| z.strftime("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| ts.to_string())
}
