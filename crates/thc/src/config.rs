//! 配置：config.toml + 环境变量 + CLI 三级合并。
//! 优先级：CLI > env > config.toml。凭据同源于此。

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub tradinghub: TradingHubConfig,
    #[serde(default)]
    #[allow(dead_code)] // Phase 3 引擎接入后消费
    pub engine: thc_engine::EngineConfig,
    #[serde(default)]
    pub llm: LlmConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct TradingHubConfig {
    pub email: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    /// OpenAI 兼容 base URL（DeepSeek 默认 https://api.deepseek.com/v1）
    #[allow(dead_code)] // Phase 5 parse-blogger 消费
    pub base_url: Option<String>,
    /// 视觉模型名（默认 deepseek-flash，env THC_LLM_MODEL 可覆盖）
    pub model: Option<String>,
    /// API key；env 优先走 OPENAI_API_KEY / DEEPSEEK_API_KEY
    pub api_key: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.deepseek.com/v1".into()),
            model: Some("deepseek-flash".into()),
            api_key: None,
        }
    }
}

impl AppConfig {
    /// 加载顺序：config.toml（可选）→ env 覆盖。CLI 覆盖在调用点处理。
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let mut cfg = match path {
            Some(p) => {
                let text = std::fs::read_to_string(p)
                    .map_err(|e| anyhow::anyhow!("读取配置 {p} 失败: {e}"))?;
                toml::from_str::<AppConfig>(&text)
                    .map_err(|e| anyhow::anyhow!("解析配置 {p} 失败: {e}"))?
            }
            None => std::fs::read_to_string("config.toml")
                .ok()
                .map(|t| toml::from_str::<AppConfig>(&t))
                .transpose()
                .map_err(|e| anyhow::anyhow!("解析 config.toml 失败: {e}"))?
                .unwrap_or_default(),
        };
        // env 覆盖凭据（沿用旧工具的变量名）
        if let Ok(v) = std::env::var("TRADINGHUB_EMAIL") {
            cfg.tradinghub.email = Some(v);
        }
        if let Ok(v) = std::env::var("TRADINGHUB_PASSWORD") {
            cfg.tradinghub.password = Some(v);
        }
        if let Ok(v) =
            std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        {
            cfg.llm.api_key = Some(v);
        }
        if let Ok(v) = std::env::var("THC_LLM_MODEL") {
            cfg.llm.model = Some(v);
        }
        Ok(cfg)
    }
}
