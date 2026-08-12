# TradingHub OptionsDataViewer — 数据接口逆向分析

## 结论（TL;DR）
页面 `https://tradinghubs.org/beta-test/OptionsDataViewer` 的**全部期权数据（表格 + 图表）**都来自两个 JSON 接口，
仅靠一个会话 Cookie 鉴权：`tradinghub_user_session`。无需截图、无需浏览器自动化。

页面用 `echarts` 在客户端把 JSON 渲染成图表，所以"图表的数据"也是 JSON 里现成的。

## 鉴权
- 整个 `/beta-test/` 路径（含静态文件 app.js）都要登录。
- 请求只需带 Cookie：`Cookie: tradinghub_user_session=<值>`
- 无额外 token 头（响应头里提到 `X-TradingHub-Internal-Token`，但 viewer 的数据接口不使用）。

## 两个核心接口

### 1. `GET /beta-test/api/gex/live-data?v=<ts>`
返回 `primary` 对象，按 ticker（SPX/NDX/SPY/QQQ/ES_SPX/NQ_NDX/TSLA/NVDA/AAPL/MSFT/GLD/IBIT）组织。
顶层字段：`ok, generated_at, last_updated_at, stale, status, primary, sources`
`primary` 字段：
- `tickers` — 全部标的列表
- `levels.<T>` — **关键价位概览**：`zero_gamma, mpos_vol, mpos_oi, mneg_vol, mneg_oi, net_gex_vol, net_gex_oi, timestamp, ticker, spot`
- `classic_chain.<T>` — 含 `strikes`（gamma ladder 原始行）、`sum_gex_vol/oi`、`major_pos_vol/oi`、`major_neg_vol/oi`、`zero_gamma`、`delta_risk_reversal`、`max_priors`
- `state_greeks.<T>` — `major_positive/negative, major_long_gamma, major_short_gamma, mini_contracts[]`
- `gex_proxy.<T>` — **前列 Gamma 行权价**：`metrics{levels_count,positive_gamma,negative_gamma,net_gamma,absolute_gamma,zero_gamma_proxy,largest_positive_strike,largest_negative_strike}` + `ladder[]`（每行：`strike,current_value,gamma,abs_value,side,distance_from_spot,distance_percent,lookback_values,dte_values,raw_row`）
- `orderflow.<T>` — **订单流看板**（37 字段）：`z_mlgamma,z_msgamma,o_mlgamma,o_msgamma,zero_mcall,zero_mput,one_mcall,one_mput,zcvr,ocvr,zgr,ogr,zvanna,ovanna,zcharm,ocharm,agg_dex,one_agg_dex,agg_call_dex,one_agg_call_dex,agg_put_dex,one_agg_put_dex,net_dex,one_net_dex,net_call_dex,one_net_call_dex,net_put_dex,one_net_put_dex,dexoflow,gexoflow,cvroflow,one_dexoflow,one_gexoflow,one_cvroflow`
- `gex_net/gex_zero/gex_one`、`dex_net/dex_zero/dex_one`、`vex_*`、`chex_*` — 按 DTE 模式（zero=0DTE, one=1DTE+, net=90Days）聚合的 exposure，按 ticker 组织
- `exposure.<T>` — 希腊值分布：`{symbol, underlyingPrice, updatedAt, metrics:{oi[],gex[],dex[],vex[],chex[]}, levels, rawCapabilities}`，每个 metric 是按行权价的点数组（`strike` + `call/put/net/total` 或 `zero/one/net`）

### 2. `GET /beta-test/api/options-data/exposure?v=<ts>`
返回 `primary.exposure.<T>` —— 希腊值分布（Greeks Profile）图表的权威数据，结构同上 `exposure`。
也带 `primary.levels / gex_proxy / orderflow`。

## 字段缩写对照（来自 app.js EXPOSURE_METRICS / DTE_MODE_LABELS）
- 指标：`oi`=Open Interest, `gex`=Gamma Exposure, `dex`=Delta Exposure, `vex`=Vanna Exposure, `chex`=Charm Exposure
- DTE 模式：`zero`=0DTE, `one`=Next DTE(1DTE+), `net`=90 Days
- 订单流：`z*`=0DTE, `o*`=1DTE+；`mlgamma`=主要多 Gamma, `msgamma`=主要空 Gamma, `mcall/mput`=主要看涨/看跌, `cvr`, `gr`(GEX), `vanna`, `charm`, `agg_dex`, `net_dex`, `*_call_dex/*_put_dex`

## 抓包实证（2026-08-12，SPX，与页面渲染一致）
- spot=7727.79，zero_gamma=7756.20（距零 Gamma 28.41 / 0.37%）
- net_gex_vol=-1029471.98（页面"-1.03M"），net_gex_oi=27815.23（"27.82K"）
- 前列正 Gamma：7725→5642.1("5.64K")，7735→2129("2.13K")…
- 前列负 Gamma：7730→-2182.09("2.18K")，7750→-1158.05("1.16K")…
- 订单流：zcvr=7899.9, ocvr=-862.11, zgr=-9577.62, ogr=-283.16, zvanna=1064.13, ovanna=-603.18 …

## 工具实现要点
- 两个 GET + Cookie，`cache: no-store`，带 `?v=<时间戳>` 防缓存。
- 合并两个响应，抽出指定 ticker（默认 SPX）的全部子结构，输出 JSON。
- `timestamp` 为 Unix 秒（EST/ET 捕获时间，如 1786478400 = 2026-08-11 16:00:00 EST）。
