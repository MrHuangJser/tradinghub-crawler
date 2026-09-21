//! 博主盘前帖（文本+截图）→ blogger.json 校准数据。
//! DeepSeek 视觉模型（OpenAI 兼容 /chat/completions），输出带候选/置信度/
//! reviewed=false 待人工复核。主分析路径不调 LLM——这是离线校准工具。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

/// 校准数据结构（与 compare 共享）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloggerLevels {
    pub date: String,
    #[serde(default)]
    pub contract: Option<String>,
    /// 多空转换位：value + 候选（截图标注歧义时多候选）+ 置信度
    #[serde(default)]
    pub pivot: Option<PivotObs>,
    #[serde(default)]
    pub bull_targets: Vec<f64>,
    #[serde(default)]
    pub bear_targets: Vec<f64>,
    #[serde(default)]
    pub major_long_support: Option<f64>,
    #[serde(default)]
    pub squeeze_zone: Option<f64>,
    /// 文本规则（如 "低于7697严禁做多"）
    #[serde(default)]
    pub rules: Vec<BloggerRule>,
    #[serde(default)]
    pub narrative_raw: Option<String>,
    /// 人工复核位：LLM 产出后必须人工确认才计为校准样本
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotObs {
    pub value: f64,
    #[serde(default)]
    pub candidates: Vec<f64>,
    #[serde(default)]
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloggerRule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub level: f64,
    #[serde(default)]
    pub text: Option<String>,
}

const SYSTEM_PROMPT: &str = r#"你是交易图表解析器。从博主 ES 盘前分析内容中提取结构化价位。

输入可能是：截图（K线图+价位标注线+文字注解）、文字帖，或两者。

提取以下字段，输出严格 JSON（不要 markdown 围栏）：
{
  "date": "YYYY-MM-DD（图表时间轴或帖子日期；推不出来用 null）",
  "contract": "ES合约代码如 ESZ2026（图上若有；否则 null）",
  "pivot": {"value": 数值, "candidates": [标签附近的所有候选价位], "confidence": "high|medium|low"},
  "bull_targets": [多头目标1/2/3 的价位，按从近到远排序],
  "bear_targets": [空头目标1/2/3 的价位，按从近到远排序],
  "major_long_support": 数值或 null（核心多头防守/多头底裤等表述）,
  "squeeze_zone": 数值或 null（Squeeze zone/挤压区）,
  "rules": [{"type": "no_long_below|no_short_above|other", "level": 数值, "text": "原文"}],
  "narrative_raw": "博主全部文字注解原文"
}

规则：
- 价位取标签文字最近的水平价格线对应的右轴数值；若标签附近有歧义（多条线），全部放入 candidates，confidence 降档
- 只提取图上明确标注的价位，不要自己算
- 无法确定的字段填 null/[]，不要编造
- 所有数字是 ES 期货点位"#;

fn mime_of(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    }
}

/// 读输入文件：图片→base64 image_url part，文本→text part。
fn load_inputs(inputs: &[String]) -> Result<Vec<Value>> {
    let mut parts = Vec::new();
    for p in inputs {
        let ext = Path::new(p)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
            let bytes = std::fs::read(p).map_err(|e| anyhow::anyhow!("读取 {p} 失败: {e}"))?;
            use base64::Engine as _;
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            parts.push(json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{};base64,{b64}", mime_of(p))}
            }));
        } else {
            let text =
                std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("读取 {p} 失败: {e}"))?;
            parts.push(json!({"type": "text", "text": text}));
        }
    }
    if parts.is_empty() {
        anyhow::bail!("没有输入文件");
    }
    Ok(parts)
}

/// 调 OpenAI 兼容 chat/completions，返回解析后的 JSON。
async fn call_llm(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    parts: Vec<Value>,
) -> Result<Value> {
    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": parts},
        ],
        "temperature": 0.0,
    });
    let resp = http
        .post(format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("LLM 请求失败: {e}"))?;
    let status = resp.status();
    let v: Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("LLM 响应非 JSON (HTTP {status}): {e}"))?;
    if !status.is_success() {
        anyhow::bail!("LLM API 错误 HTTP {status}: {}", v);
    }
    let content = v
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("LLM 响应缺 choices[0].message.content"))?;
    // 容忍 markdown 围栏
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(cleaned)
        .map_err(|e| anyhow::anyhow!("LLM 输出非合法 JSON: {e}; 原文: {cleaned:.300}"))
}

pub async fn run(
    cfg: &crate::config::AppConfig,
    inputs: Vec<String>,
    output: Option<String>,
) -> Result<i32> {
    let llm = &cfg.llm;
    let api_key = llm.api_key.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "未配置 LLM key：设 OPENAI_API_KEY / DEEPSEEK_API_KEY 或 config.toml [llm].api_key"
        )
    })?;
    let base_url = llm
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.deepseek.com/v1".into());
    let model = llm.model.clone().unwrap_or_else(|| "deepseek-flash".into());

    let parts = load_inputs(&inputs)?;
    let http = reqwest::Client::new();
    eprintln!("🔍 调用 {model} 解析 {} 份输入…", inputs.len());
    let mut v = call_llm(&http, &base_url, &api_key, &model, parts).await?;
    // 保底字段 + 强制 reviewed=false
    if let Some(obj) = v.as_object_mut() {
        obj.entry("date").or_insert(Value::Null);
        obj.insert("reviewed".into(), Value::Bool(false));
        obj.insert("parsed_by".into(), json!(model));
    }
    let parsed: Result<BloggerLevels, _> = serde_json::from_value(v.clone());
    if let Err(e) = &parsed {
        eprintln!("⚠️ 输出结构与 blogger schema 不完全吻合: {e}（仍写原始 JSON）");
    }
    let text = serde_json::to_string_pretty(&v)?;
    match output {
        Some(p) => {
            std::fs::write(&p, format!("{text}\n"))?;
            eprintln!("✅ 已写入 {p}（reviewed=false，请人工复核后改为 true）");
        }
        None => println!("{text}"),
    }
    Ok(0)
}
