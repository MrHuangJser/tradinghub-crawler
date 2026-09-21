# ES 盘前分析工具（Rust 重写）免费/公开行情数据源调研报告

> 调研日期：2026-09-21（周一，美东时间）。方法：仅引用一手来源（官方文档 / 官方 API 页 / 官方价格页 / 官方条款页）+ 对可公开访问的端点做现场抓取验证（标注「实测」）。无法通过官网核实的结论一律标注「**未核实**」。
>
> 重要区分：**「数据存在」≠「08:30 ET 盘前时点可用」**。每条结论均注明盘前可用性。采样环境说明：实测均在 2026-09-21 凌晨（约 ET 04:40–05:00，Globex 夜盘时段）进行。

---

## 1. TL;DR

**（a）免费源能否完全替代 TradingHub？**
**能替代其原料和静态指标，不能替代其订单流方向类指标。** TradingHub 的核心加工品 GEX/CVR/straddle/合成远期，其原料（SPX 期权链逐档 OI/成交量/bid/ask/IV/greeks）可 100% 免费获得——CBOE CDN（免鉴权、58 个到期日全量字段；**本次核实到官网条款明文禁止自动抓取，属灰色**）+ **Tradier 免费 Lite（本次实测发现：$0 月费含 API，生产 token 给实时全字段链含 ORATS greeks，合法）**，两者互为冗余后这些指标可全部本地重算；另有官方免费的 SPX Put/Call 比率 CSV 可做 CVR 交叉校验。免费拿不到的是**期权逐笔成交方向（aggressor/买卖压）**——所有免费源只给每合约累计成交量，TradingHub 的实时订单流指标无免费等价物。建议：TradingHub 从核心依赖降级为「订单流增强插件 + 交叉校验」，而不是彻底删除。

**（b）免费的 ≤60 秒同步 ES 盘前报价存在吗？**
**不存在（合法免费口径下）。** CME 实时行情是付费品：官网 10 分钟延迟报价页明确禁止脚本抓取（实测 403 + 条款原文），TradingView scanner 同为 10 分钟延迟且无官方 API（灰色），Massive（原 Polygon）期货免费层纯 EOD、10 分钟延迟从 $29/月起，Databento 无免费实时（live 需订阅 + license fee）。但要点在于 **08:30 ET 时 SPX 现货本来就是静态的**（指数隔夜不交易，= 昨日收盘），所以「双边 ≤60s 同步」在盘前时点实际退化为「ES 单边新鲜度」问题：
- 免费最佳：**Yahoo `v8/finance/chart/ES=F`**（免鉴权，实测 ~15 分钟延迟，Globex 夜盘持续更新）——basis 口径 =「ES(08:15) − SPX(昨收)」，SPX 侧静态无漂移，误差仅来自 ES 的 15 分钟滞后（典型 <0.3%），对盘前 regime 锚定可接受；ONH/ONL 读数会截到 08:15。
- 合规优先：**IBKR 免费延迟流**（TWS API `reqMarketDataType(3)`，官方文档化支持程序化，10–20 分钟延迟）。
- 一次性例外：**Databento $125 注册额度**（6 个月过期）可按量拉 CME Globex 隔夜 ES 分钟线（historical/delayed 口径，无 license fee）——够验证数月，但非持续免费。
- 若要真·双边实时：最便宜合规组合 ≈ **$5.05/月**（IBKR：CME L1 $1.55 + Cboe Streaming Indexes $3.50，非专业身份 + 账户净值 ≥$500）。

**（c）推荐免费组合（字段 → 源）**

| 引擎字段 | 主源 | 冗余/备用 |
|---|---|---|
| SPX/SPXW 期权链逐档（OI/vol/bid/ask/IV/greeks，分 0DTE/周/月） | CBOE CDN `_SPX.json`（免鉴权，58 个到期日；条款灰色，每日一次低频） | **Tradier 免费 Lite**（生产 token 实时 + greeks + `expiration_type`，合法——合规首选/一键切换）；marketdata.app 免费层（仅昨收链基线） |
| VIX / VIX1D / VIX9D / VVIX / SKEW | CBOE CDN `quotes/_*.json`（无变化，已确认） | Yahoo `^VIX` chart；**官方历史 CSV（合法免费）** |
| SPX 现货 | CBOE CDN（`_SPX.json` 顶层 `current_price`，隔夜=昨收） | Tradier `/markets/quotes`（实时）；Yahoo `^GSPC`；SPX_History.csv |
| ES 报价 + ONH/ONL + PDH/PDL/昨收 | Yahoo `v8/finance/chart/ES=F`（1m K 线 + 日线 meta，~15min 延迟） | IBKR 延迟流（合规）；TradingView scanner（灰色） |
| ES 官方结算价 | **Massive（原 Polygon）免费层** `/futures/v1/aggs/ESZ5?resolution=1session` → `settlement_price`（文档核实、需注册 key，EOD 口径） | Yahoo `chartPreviousClose` 近似（非官方）；CME 结算页**人工查看**（免费含隔夜高低，仅近一周）；Nasdaq Data Link CHRIS/CME_ES1（**待人工核实**） |
| FOMC / CPI / NFP / GDP 日历 | federalreserve.gov / bls.gov（人工核对）/ bea.gov/news/schedule | BLS Public Data API（数据值） |
| OPEX 日历 | 本地推算第三周五（CBOE 规格页规则）+ CBOE 官方节假日 CSV | CBOE 年度期权日历 PDF |
| 期权流方向（加分项） | ❌ 免费无 → TradingHub 保留为增强源 | per-contract volume + 盘前/收盘 bid/ask 对比做粗粒度推断；官方 `spxpc.csv` 做 CVR 校验 |
| Volume Profile（加分项） | Yahoo/IBKR 1m K 线自建分钟级 VP（非 tick 级） | — |

**一句话架构**：CBOE CDN 继续当主力（期权链+VIX 家族，每日一次低频、加 Tradier 一键切换的合规退路）+ Tradier 免费 Lite 做合规实时冗余 + Yahoo ES=F 免费垫底（可接受 15min 滞后则免单）+ 官方日历本地推算；TradingHub 只保留订单流增强。

---

## 2. 对比矩阵

| 源 | 覆盖字段 | 价格 | 延迟 | API 形态 | 盘前可用性（08:30 ET） | ToS 风险 |
|---|---|---|---|---|---|---|
| CBOE CDN delayed_quotes（期权链） | SPX/SPXW 全 58 个到期日逐档：OI、成交量、bid/ask、IV、Greeks | 免费 | 官方未标分钟数；实测 GTH 盘口 ~5 分钟粒度更新、成交冻结在上一 RTH 收盘 | 免鉴权 JSON over HTTPS | ✅ 可用（volume/OI=上一交易日终值口径） | **灰色**（官网条款明文 "STRICTLY PROHIBITED … AUTO-EXTRACTION" + 威胁封 IP，见 §3.1） |
| CBOE CDN delayed_quotes（指数） | VIX/VIX1D/VIX9D/VVIX/SKEW/SPX 现值+收盘值 | 免费 | 同上 | 免鉴权 JSON | ✅ 可用（盘前=上一交易日收盘值） | 同上 |
| **CBOE 官方历史 CSV** | VIX 家族+SPX 历史（SPX 1975 起）；SPX Put/Call 比率；节假日 | 免费 | EOD（日更） | CSV 免登录直下 | ✅（昨日数据） | **合法免费** |
| Yahoo Finance v8 chart | ES 期货 OHLCV（1m/1d）、SPX 现货 | 免费 | 实测 ~15 分钟 | 免鉴权 JSON | ✅ 可用（但滞后 ~15min） | 灰色（无官方公开 API 文档，ToS 限制自动访问） |
| Yahoo Finance v7 options / quote | 期权链（IV/OI/bid/ask，无 greeks）、报价 | 免费（无官方 API） | 自述 15 分钟 | 需 cookie+crumb | ⚠️ 可用但违 ToS | 灰色（条款明文禁自动抓取） |
| **Tradier 免费 Lite** | **SPX/SPXW 期权链全字段（含 greeks/IV/OI）+ SPX 指数，实时** | **$0**（注册即得生产+沙盒 token） | 生产=实时；沙盒=15 分钟 | REST + Bearer token | ✅ 生产 token 可用（120 req/min） | 合法免费（个人内部用） |
| CME Group 官网（quotes/settlements 页） | ES 延迟报价、结算价、隔夜高低（近一周） | 免费（人看） | ≥10 分钟（官方标注） | 网页 + 内部 JSON | 页面对人可用；**脚本实测 403 封禁** | 明确禁止抓取（条款原文见 §3.3） |
| CME DataMine | EOD/逐笔/报价历史（1993 起） | 付费（登录后可见价） | — | 云端/SFTP 交付 | — | 付费（internal use only） |
| CME FedWatch | Fed 加降息概率（30D FF 期货隐含） | 免费工具（网页） | — | 网页；**无官方 API** | 页面可用 | 抓取同受 Data Terms 禁止 |
| marketdata.app 免费层 | 全 OPRA 链（IV/Greeks/OI/bid/ask） | $0：100 credits/天（按合约计费） | 24h（期权=次日 09:30:01 ET 才更新到昨收） | REST | ⚠️ 08:30 拿到的是上一交易日收盘链；credits 只够 ~100 合约 | 合法免费（禁再分发） |
| ORATS | 期权链/IV/greeks（EOD+延迟） | ❌ 无免费层（Delayed $199/月起） | 付费档 15 分钟 | REST + token | — | 付费 |
| OptionsDX | SPX 历史链 CSV（含 greeks/IV，至 1m 粒度） | 免费变体存在（$0–$50/文件） | 历史（季更，止于 2023） | CSV 下载（非 API） | ❌ 不适用每日工具（仅回测） | 免费但仅历史 |
| Massive（原 Polygon.io，2025-10 更名） | 免费层：股票/期权/期货 EOD 聚合 K 线（2 年历史、5 次/分钟）；**期货 session K 线含 settlement_price**；无期权快照、指数免费层无 SPX | $0 Basic（需注册 key）；10/15 分钟延迟从 $29–49/月起 | 免费层 EOD | REST（api.massive.com 与 api.polygon.io 并行） | ⚠️ 结算价/前日结构可用；当日隔夜分钟线不可用（EOD 截止时刻未核实） | 合法免费 |
| Databento | CME Globex 全量（ES 含隔夜时段），historical 按量无 license fee | $125 一次性注册额度（6 个月过期，非持续免费层）；ohlcv-1m $70/GB；live 包月 $1,500+ 且有 license fee | historical 含 "delayed intraday"（分钟数未核实） | REST + 官方 SDK | ⚠️ 用额度拉隔夜 ES 分钟线理论可行（一次性） | 合法（付费为主） |
| Barchart | ❓ 免费账号范围 / OnDemand 试用 / ES 页延迟 | — | — | — | — | **未核实**（官网 AWS WAF JS 挑战，archive.org 离线；需人工浏览器访问 barchart.com/futures/quotes/ES*1） |
| stooq | 指数/期货日线 EOD CSV | 免费 | EOD | CSV 下载 | ❌ **程序化实质不可用**：PoW 挑战解出后仍持续下发（实测），或需完整浏览器指纹 | 灰色（条款原文未取得） |
| Interactive Brokers | 延迟 ES/SPX（15–20 分钟，程序化官方支持）；付费后 ES 实时 L1 仅 $1.55/月 | 开户+净值 ≥$500；延迟数据免费无需订阅 | 15–20 分钟（CME 延迟表标 10 分钟） | TWS API（reqMarketDataType(3)）/ Client Portal | ✅ Globex 时段可取（延迟口径）；SPX 指数=昨收 | 合法（历史/结算数据需另订阅） |
| Nasdaq Data Link (CHRIS) | ES 连续合约 EOD（CHRIS/CME_ES1） | 免费注册 key：50k 次/天；匿名仅 50 次/天（共享池，极易耗尽） | EOD | REST JSON/CSV | ⚠️ CHRIS 现状**未核实**（Incapsula 反爬 + 匿名限额耗尽） | 合法免费 |
| FRED / BLS / BEA / Fed | FOMC/CPI/NFP/GDP 日历 + 宏观数据值 | 免费 | — | 网页（FOMC/BEA 可直抓；BLS 反爬）；BLS 数据值有官方 JSON API | ✅ | 合法免费 |
| CBOE 假期/OPEX 日历 | 期权到期规则 + 节假日 CSV | 免费 | — | **官方 CSV** `cboe.com/us/options/holidays/csv/` | ✅ | 合法免费 |
| TradingView / investing.com 非官方 | scanner（实测 10 分钟延迟）/ 经济日历网页 | — | — | 无官方 API（TradingView rest-api-reference 404；investing 403） | — | 灰色（仅记录） |

---

## 3. 逐源详述

### 3.1 CBOE CDN delayed_quotes（已在使用，重点核实）

**端点（实测 2026-09-21）**

- SPX 期权链：`https://cdn.cboe.com/api/global/delayed_quotes/options/_SPX.json`（HTTP 200，约 13 MB）
- 指数报价：`https://cdn.cboe.com/api/global/delayed_quotes/quotes/_<SYM>.json`，`<SYM>` ∈ {SPX, VIX, VIX1D, VIX9D, VVIX, SKEW}（全部 HTTP 200）

**实测字段覆盖（期权链，29,518 行 / 58 个到期日）**

每个合约（如 `SPX261016C00200000`、`SPXW260921...`）：

```
option, bid, bid_size, ask, ask_size, iv, open_interest, volume,
delta, gamma, vega, theta, rho, theo, change,
open, high, low, tick, last_trade_price, last_trade_time,
percent_change, prev_day_close
```

- ✅ **引擎硬需求第 1 项全部字段原生覆盖**：OI、当日成交量、bid/ask、IV（另有全套 Greeks 白送）。
- ✅ 到期日维度（58 个）：当日 0DTE（`260921`）+ SPXW 逐日/周度 + 月度（`SPX` 前缀，第三个周五）+ LEAPS 到 2030-12，可按合约前缀（SPXW=周/日度 vs SPX=月度）+到期日直接区分 0DTE/周内/月度。
- ✅ ATM straddle / put-call parity 合成远期所需数据齐备（bid/ask + theo）。
- 顶层附标的现货 `current_price / bid / ask`，及全局 `timestamp`（与 `quotes/_SPX.json` 等价）。

**实测延迟与盘前行为（关键，两路独立采样互证）**

- HTTP 响应头 `cache-control: max-age=0, s-maxage=5, stale-while-revalidate=5`；文件由 CloudFront 提供服务。
- **盘口隔夜会动，成交不会动**：Cboe GTH 时段（实测 ET 04:45–05:00）ATM 0DTE bid/ask 约 5 分钟粒度持续变动（43.7/44.0 → 45.5/45.7）；但**全部合约的 `volume` 与 `last_trade_time` 冻结在上一 RTH 收盘**（最近成交=上周五 16:14:59 ET，当日夜盘成交 0 笔）→ 隔夜拿到的 volume/OI = 上一交易日终值口径（正好是 GEX/CVR 想要的口径），但**拿不到隔夜期权成交量**。
- 时间戳口径（实测）：顶层 `timestamp` 为 **UTC**，合约 `last_trade_time` 为 **ET**——混用易错，Rust 侧需显式时区处理。
- 08:30 ET 属于 Cboe 早间 GTH 时段（结束于 09:15 ET；起始时点口径不一，**未核实**），推断报价继续更新（08:30 整点未实测）。
- 官方延迟分钟数：**官方任何页面均未写明（未核实；社区通称 15 分钟，非一手）**，页面仅有 "Delayed Quotes" 标题。
- 官方文档：**该 CDN API 无官方文档页**，属公开可访问端点（仅见社区逆向项目引用）。

**ToS 风险（重要，本次核实到官方原文）**：该 JSON 驱动的官网页面 `https://www.cboe.com/delayed_quotes/spx/quote_table`（实测 HTTP 200）内嵌条款原文：

> "IT IS STRICTLY PROHIBITED TO DOWNLOAD DELAYED QUOTE TABLE DATA FROM THIS WEB SITE BY USING AUTO-EXTRACTION PROGRAMS/QUERIES AND/OR SOFTWARE. CBOE WILL BLOCK IP ADDRESSES OF ALL PARTIES WHO ATTEMPT TO DO SO. … DOWNLOADING THIS DATA IN ANY OTHER WAY THAN BY MANUAL TICKER SYMBOL ENTRY IS STRICTLY PROHIBITED."

- 即：端点无鉴权、可匿名访问，但**官方明文禁止程序化下载并威胁封 IP** → 定性为**灰色**。当前引擎每日 1 次低频拉取属于低风险用法（风险自担），但应把 **Tradier 免费 Lite 作为可一键切换的合规备源**。主站 ToS（https://www.cboe.com/terms/）无抓取专门条款，页面级条款如上优先。

**结论**：字段覆盖无变化、继续可用；新增两点认知——(1) 条款明禁自动抓取（升级风险评级）；(2) 隔夜盘口刷新/成交冻结的口径分离。建议：schema 校验 + fixture 回归 + Tradier fallback + 低频（每日一次）+ 显式 UTC/ET 处理。

**同族「合法免费」CSV（官方公开下载，与灰色 JSON 区分，实测全部 200、免登录、更新至上一交易日 09/18/2026）**

| 数据 | URL | 起点 | 用途 |
|---|---|---|---|
| VIX 历史 | `https://cdn.cboe.com/api/global/us_indices/daily_prices/VIX_History.csv` | 1990 | OHLC 日线 |
| VIX1D 历史 | `…/us_indices/daily_prices/VIX1D_History.csv` | 2022-05 | |
| VIX9D 历史 | `…/us_indices/daily_prices/VIX9D_History.csv` | 2011 | |
| VVIX 历史 | `…/us_indices/daily_prices/VVIX_History.csv` | 2006（仅收盘） | |
| SKEW 历史 | `…/us_indices/daily_prices/SKEW_History.csv` | 1990 | |
| SPX 现货历史 | `…/us_indices/daily_prices/SPX_History.csv` | **1975** | 收盘价 |
| **SPX Put/Call 比率** | `https://cdn.cboe.com/resources/options/volume_and_call_put_ratios/spxpc.csv` | 2019-10 | **CVR 交叉校验**（Date/Put·Call Vol/Total Vol） |
| 期权节假日 | `https://www.cboe.com/us/options/holidays/csv/` | 当年 | 见 §3.11 |

- 官方入口页：https://www.cboe.com/tradable_products/vix/vix_historical_data 、https://www.cboe.com/us/indices/dashboard/daily-prices/ （下载按钮即指向上述 CDN CSV）。旧路径 `us_eod_indices_data/data/VIX_History.csv` 已失效（实测 AccessDenied）。
- 注意 `spxpc.csv` 文件头自带免责声明（数据可能基于 preliminary volume）。

**CBOE DataShop**（https://datashop.cboe.com/）：**付费**数据商店（购物车+订单+SFTP 交付，注册免费、数据付费）。产品：Option EOD Summary（EOD + 15:45 ET 快照，可选 IV/Greeks）、Option Trades、Option Quotes（分钟级 NBBO）、Open-Close Volume Summary、CFE Futures Trades（仅 VIX 期货，无 ES）。授权原文：*"Raw data is licensed for internal use only and may not be redistributed externally in any form."* 官方页面**未见任何免费数据集**（具体价格表匿名不可见，**未核实**）；亦无免费 SPX 期权结算价文件（相关 URL 404）。

### 3.2 Yahoo Finance

**实测（2026-09-21）**

- `v7/finance/options/%5ESPX`（无认证）→ **HTTP 401 `Invalid Crumb`**：需先 `GET fc.yahoo.com` 取 cookie → `GET /v1/test/getcrumb` → 带 `?crumb=` 请求 → HTTP 200。
- 期权链字段（实测）：`openInterest`、`volume`、`bid`、`ask`、`impliedVolatility`、`lastPrice`、`lastTradeDate`、`strike`、`expiration`；**无 greeks**。56 个到期日一次返回（可用 `?date=` 按到期取），SPXW 周权在链内（`contractSymbol` 前缀 SPXW）。响应自述 `exchangeDataDelayedBy: 15`。
- `v7/finance/quote`（无认证）→ **HTTP 401**，指向 https://bit.ly/yahoo-finance-api-feedback（官方反馈位）。
- `v8/finance/chart/ES%3DF?interval=1m`（无认证）→ **HTTP 200 可用**：
  - `regularMarketPrice=7762.75`，`regularMarketDayHigh=7763.25 / DayLow=7713.75`（夜盘累计高低）；
  - 最后 1m K 线时间 `08:35:03 UTC` vs 抓取时刻 `08:50 UTC` → **延迟约 15 分钟**（CME 延迟行情源特征：10 分钟牌价 + 处理滞后）；
  - 日线 meta 含 `chartPreviousClose=7556.5`（上一交易时段收盘，**非官方结算价**）；
  - 夜盘时段数据持续更新（Globex 时段有 K 线）→ **08:30 ET 可用，但价格口径 = 08:15 ET 左右的 ES**。
- `v8/finance/chart/%5EGSPC`（无认证）→ HTTP 200；盘前 `marketState=PRE`，现货价停在上一收盘（实测 7650.5，与 CBOE SPX 收盘一致）。
- **官方无公开 API 文档**：`developer.yahoo.com/finance` 重定向至开发者首页；`developer.yahoo.com/api` 仅列 OAuth / Fantasy Sports / Sign In——官方 Finance API 已不存在（https://developer.yahoo.com/api/）。

**ToS（灰色偏黑）**：Yahoo Terms（https://legal.yahoo.com/us/en/yahoo/terms/otos/index.html）原文：

> "access or collect data, or attempt to access or collect data, from our Services using any automated means... including but not limited to robots, spiders, scrapers, data mining tools... for any purpose without our express, prior permission."

**结论**：v8 chart 是免费组合里 ES 期货报价的最佳「免鉴权」候选（代价 ~15 分钟延迟）；v7 期权链可用但明文违反 ToS，且 CBOE CDN 字段更全（有 greeks），无理由用 Yahoo 期权链。未核实：429 限流阈值。

### 3.3 CME Group（官网报价 / 结算 / DataMine / FedWatch）

**实测（2026-09-21，数据中心 IP）**

- 官网报价页 `https://www.cmegroup.com/markets/equity-index/us/s-and-p-500/e-mini-s-p-500.quotes.html` → **HTTP 403**。
- 官网内部报价 API `https://www.cmegroup.com/CmeWS/mvc/Quotes/Future/305/G` → **HTTP 403**，返回原文：

> "This IP address is blocked due to suspected web scraping activity associated with it on this CMEgroup.com page. Use of scripts, software, spiders, robots, avatars, agents, tools or other scraping mechanisms is strictly prohibited by CME Group's website Data Terms of Use…"

- 即：**CME 明确禁止脚本抓取官网数据，且对数据中心 IP 主动封禁**。住宅 IP 浏览器可看 10 分钟延迟报价，但不能作为程序化数据源。

**延迟报价口径**（官方 about-quotes，Wayback 存档 https://www.cmegroup.com/trading/about-quotes.html）：*"Prices are delayed at least 10 minutes."*

**结算价页（人工免费，程序化禁止）**

- 官方中文站（一手，https://www.cmegroup.cn/how-get-historical-data/，实测可访问）：*"芝商所的官方网站提供几乎所有合约过去一周的结算价、交易量、以及持仓量的数据，以及当天每笔成交…"* —— 结算页免费含 **Settle（结算价）+ High/Low**，且 High/Low 官方定义覆盖 Globex 与场内时段（about-settlements 存档原文："during either Globex or the Open Outcry session"）→ **ONH/ONL 素材其实就在结算日报里，但只能人工查看近一周，导出格式未核实（页面被拦截）**。

**CME DataMine**（https://datamine.cmegroup.com/）：付费自助历史数据平台——官方中文页：*"DataMine 可提供大部分的 CME Globex 电子交易产品自 1993 年以来的当日结束价数据（End-of-Day Data）"*。EOD/逐笔/报价序列，云端/SFTP 交付；具体定价需登录（**未核实**）。

**CME FedWatch**（https://www.cmegroup.com/markets/interest-rates/cme-fedwatch-tool.html）：免费公开工具（存档实测：*"Use FedWatch to track the probabilities of changes to the Fed rate, as implied by 30-Day Fed Funds futures prices."*）；**无官方 API**（内部数据端点同样受 Data Terms 禁止）。

**CME 抓取条款**（核心原文，https://www.cmegroup.com/trading/market-data-explanation-disclaimer.html，2026-09-18 存档）：

> "You may access content only for your personal use for non-commercial purposes."（且非商业用途明确**不包括** "development of any software program"）
>
> "Unless CME Group gives you prior written permission, use of any Web browsers (other than generally available third-party browsers), engines, scripts, software, spiders, robots, avatars, agents, tools or other devices or mechanisms … to navigate, access, copy in bulk, retrieve, harvest, index, search or analyze any portion of the Website is strictly prohibited."

**结论**：CME 官网数据（延迟报价/结算价/隔夜高低）对「人工浏览」免费，对「程序化」条款+ WAF 双重封死 → **ES 期货数据无法从 CME 官方渠道免费程序化获取**，只能人工、DataMine 付费或授权分销商。

### 3.4 stooq

**实测（2026-09-21，双路独立验证）**

- `https://stooq.com/q/d/l/?s=^spx&i=d` 与 `?s=es.f&i=d`：均返回 **JS 工作量证明（PoW）浏览器校验页**（SHA-256 找 4 个前导零 → POST `/__verify` 换 auth cookie → reload），非 CSV。
- 第二路实测：**即便解出 PoW 并取得 auth cookie，服务端对后续请求仍持续下发挑战**（疑似会话/IP 绑定或要求完整浏览器指纹）→ 程序化取数**实质不可用**。
- 即使绕过，该端点也只有日线 EOD CSV（无盘中、无期权）→ 对 08:30 ET 盘前场景无当日隔夜数据。
- 条款页同样在挑战墙之后 → 官方 ToS 原文**未取得（未核实）**；挑战页 `meta noindex,nofollow`。
- 结论：不推荐作为 Rust 引擎依赖。

### 3.5 marketdata.app

**官方页面核实（pricing / docs）**

- 免费层 "Free Forever" $0：**100 Daily API Credits**、Standard API Endpoints、1 Year Historical Data、**24h Delayed Stock Data、24h Delayed Options Data**（https://www.marketdata.app/pricing/）。
- 数据新鲜度规则（关键，https://www.marketdata.app/docs/account/data-freshness/）原文：

> "Options: Historical at the next session's open — 9:30:01 AM ET the next trading day, not at the prior session's close."

- 即 **08:30 ET 时免费层拿到的是上一交易日的完整收盘链**（昨日终值 OI/volume + 收盘 bid/ask/IV）——作「昨收基线」可用，无当日晨间新鲜度。
- 计费按合约数（https://www.marketdata.app/docs/api/universal-parameters/mode.md）：免费层每个含报价的符号 = 1 credit → 整条 SPX 链 20,000+ 合约，100 credits/天 只够 ~100 张合约（须重度过滤 strike/expiry）；`cached` 模式（1 credit 全链）仅付费层。
- 覆盖（付费层）：全 OPRA 链 "Bid/ask, last, volume, open interest, IV, and full Greeks"；**无期货行情**（官方页面仅 stocks/options/mutual funds）。
- ToS（https://www.marketdata.app/terms/）：禁再分发/转售（"You may not sell, resell, retransmit, or redistribute…"），个人内部用合法。

**结论**：免费层对盘前引擎价值有限（credits 太少 + 只有昨收链）；30 天全功能试用（无信用卡）可作短期评估。

### 3.6 Tradier（本次调研最大发现：免费层含实时 SPX 期权链 API）

**端点**

- 生产：`GET https://api.tradier.com/v1/markets/options/chains?symbol=SPX&expiration=YYYY-MM-DD&greeks=true`
- 沙盒：`https://sandbox.tradier.com/v1/...`（注册即得生产 + 沙盒双 token；沙盒无 token 请求实测 401）

**官方文档核实的字段覆盖**（https://docs.tradier.com/reference/brokerage-api-markets-get-options-chains.md）

- 逐档：`bid/ask/bidsize/asksize`（含 `bid_date/ask_date` 时间戳）、`volume`、`open_interest`、`strike`、`expiration_type`（**standard/weekly，直接区分月度/周内/0DTE**）、`option_type`、`root_symbol`；
- `greeks=true`：`delta/gamma/theta/vega/rho/phi`、**`bid_iv/mid_iv/ask_iv`**、`smv_vol`、`updated_at`（ORATS 提供的 Greeks）；
- 另有 `/v1/markets/quotes`（**SPX 指数实时**，生产环境）、`/v1/markets/calendar`、`/v1/markets/clock`（市场日历，非经济数据日历）。

**价格与层级**（官方页面）

- pricing（https://www.tradier.com/pricing）：**Lite $0/月，"API Access Included"**；api-overview（https://www.tradier.com/develop/api-overview）原文："*Including the free Lite plan. Sign up in about 10 minutes and generate your API token*"。
- 实时门槛（https://docs.tradier.com/docs/market-data.md）：*"Real-time data is available to all Tradier Brokerage account holders for US-based stocks and options. If you are not a Tradier Brokerage account holder, we are unable to provide you with any real-time data solution."*（免费 Lite 即经纪账户；未注资能否即时开通实时权限**未实测**。）
- **沙盒 vs 生产**（同上文档的表格）：沙盒 = 真实 **15 分钟延迟**行情（DTN 源，非模拟），但**指数不可用、greeks 不可用**；生产 = 股票/期权/指数实时，greeks 每小时更新。
- 速率限制（https://docs.tradier.com/docs/rate-limiting.md）：生产 120 req/min、沙盒 60 req/min → 08:30 按到期日循环 ~10 个 expiration 完全够用。
- **ES 期货：无**。官方文档全索引（llms.txt，87 页）"futures" 零提及；期货仅作为 CQG/Rithmic 经纪通道存在，不在 API 行情内。
- ToS（https://api.tradier.com/v2/applications/agreements?key=api_agreement，PDF）：个人内部研究 = 合法免费；"we may revoke an API's authorization at any time, for any reason"。

**盘前 08:30 ET**：生产 token 可用。注意 SPX 期权 08:30 时本身处于 Cboe ETH 时段，报价按交易所实际状况返回。

**结论**：**CBOE CDN 的最佳合规冗余源**——字段超集（greeks+triple IV+expiration_type），实时，免费。与 CBOE 互为校验。

### 3.7 ORATS（无免费层）

- 官方 data-api 页（https://orats.com/data-api）：Delayed **$199/月**（20k req/月）、Live $299/月、Live Intraday $599/月、All-In $899/月。
- FAQ 原文："*We do not offer free trials at this time.*" → **无免费层、无试用**。
- 数据：Strikes 逐档 bid/ask + greeks + IV（"*Gathered 14 minutes before the close*"，SMV 清洗），EOD 历史回溯 2007，覆盖 "*All US equity options including stocks, ETFs, and indexes*"，"*SPX AM and PM expirations both work*"。无期货。
- 对本项目：付费档才可用 → 记录备查，不进免费组合。

### 3.8 OptionsDX（免费历史期权链下载，仅回测用）

- 是什么（https://www.optionsdx.com/）：历史期权链 CSV 下载商店（ShareFile 交付，非 API）："*Free Historical Options Trading Data… including the SPX, VIX, SPY… Datasets includes pre-calculated greeks, IV, and underlying price at up to minutely intervals.*"
- SPX 产品页（https://www.optionsdx.com/product/spx-option-chain/）：年份 **2010–2023**，频率 EOD/30m/15m/5m/1m，价格 $0.00–$50.00（存在免费变体），月度 CSV。
- 更新频率（FAQ，https://www.optionsdx.com/faq/）：**季度**（Q1 数据 4 月上旬发布）→ 对每日 08:30 工具**不可用**。
- 条款（https://www.optionsdx.com/terms/）仅 "*All Sales for Digital Content or Services are Final*"；数据授权链**未核实**；哪些年份/频率变体为 $0 **未核实**（变体价格接口被 WAF 拦截）。

### 3.9 Interactive Brokers TWS/API（四个核查点全部官方核实）

**官方来源**

- 行情订阅价目：https://www.interactivebrokers.com/en/pricing/market-data-pricing.php（实测 200）
- 最低净值要求：https://www.interactivebrokers.com/en/index.php?f=14193&conf=am&amref=1
- 延迟数据 API 文档：https://ibkrcampus.com/docs/tws-api/doc/market-data-delayed/introduction.md（旧站 interactivebrokers.github.io/tws-api/delayed_data.html 已标 deprecated）
- 文档索引：https://ibkrcampus.com/docs/llms.txt

**(a) 是否需要入金**：订阅并维持行情需账户净值 ≥ **USD 500**（个人账户；官方原文 "This does not include the cost of the service"）。

**(b) 免费延迟数据（核心发现）**：**免费、无需任何订阅、官方文档化支持程序化调用**——TWS API `reqMarketDataType(3)` 即官方机制，官方文档原文：

> "Free, delayed data is 15 – 20 minutes delayed… you are telling TWS to automatically switch to delayed market data if the user does not have the necessary real time data subscription."

- 定价页各交易所延迟表显示 CME 延迟 = **10 分钟**（与 API 文档 15–20 分钟口径略不一致，以实测为准）。
- 美股另有免费实时项：Cboe One + IEX 非合并实时流（免费）、100 次/月免费快照（SNAPSHOT）。
- 限制：延迟流仅支持 `reqMktData`/`reqHistoricalData`，不支持 tick 级；**历史/结算数据仍需订阅**（官方原文 "historical data still requires market data subscriptions"）。

**(c) 若要实时 ES 的最低成本（非专业）**：CME L1 **USD 1.55/月**、L2 USD 12.10/月；SPX 现货指数流式需 Cboe Streaming Market Indexes **USD 3.50/月** → 「ES 实时 + SPX 指数实时、双边 ≤60 秒」最便宜合规组合 ≈ **USD 5.05/月**（前提：账户净值 ≥ $500 且非专业身份——个人、非金融从业者、非注册投资顾问）。

**(d) 盘前 08:30 ET**：Globex 在交易，实时/延迟 ES 均可取；SPX 指数隔夜无连续报价，只能拿昨收。

**结论**：合规优先时的首选架构位——免费延迟档（10–20 分钟）做兜底，$5/月 级别即可升级为真正的双边实时。

### 3.10 Nasdaq Data Link（原 Quandl）

**官方核实**

- API host：`https://data.nasdaq.com/api/v3/`；限额文档 https://docs.data.nasdaq.com/docs/rate-limits-1（已 301 迁移，内容经 web.archive.org 存档核实）。
- 实测匿名请求返回官方错误文案：*"You have exceeded the anonymous user limit of 50 calls per day"* → **匿名 50 次/天（全球共享池，极易耗尽）**；免费注册 key：300 次/10 秒、2,000 次/10 分钟、50,000 次/天、并发 1。
- **CHRIS/CME_ES1（ES 连续合约 EOD）是否仍免费：未核实**——data.nasdaq.com 有 Incapsula 反爬（本次实测 403），官方文档站又已迁移，无法程序化确认。第三方（Packt 书籍 GitHub issue）报告其已弃用，但非一手。**构建前必须注册免费 key 人工实测一次**。
- EOD 更新时间（ET）、盘中数据有无、免费期权数据有无：均**未核实**。

### 3.11 事件日历的官方免费来源（逐个核实）

| 事件 | 最佳免费官方来源 | 机器可读性 | 核实状态 |
|---|---|---|---|
| FOMC | https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm | HTML（无官方 JSON，结构多年稳定） | ✅ 实测 200，2025/2026/2027 全部日期在页 |
| CPI | https://www.bls.gov/schedule/news_release/cpi.htm | HTML（**Akamai 反爬 403**） | ⚠️ URL 官方确认，内容需人工浏览器核对 |
| NFP（就业报告） | https://www.bls.gov/schedule/news_release/empsit.htm | HTML（同上）；年度总表 bls.gov/schedule/2026_sched.htm | ⚠️ 同上 |
| GDP/PCE（BEA） | https://bea.gov/news/schedule | HTML，**curl 可直抓** | ✅ 实测 200（2026 日程全列，均为 8:30 AM ET） |
| SPX OPEX | CBOE 规格页 https://www.cboe.com/tradable-products/sp-500/spx-options/spx-specifications | 规则：原文 "*Expiration Date — The third Friday of the expiration month.*" → 本地推算 | ✅ 实测 200 |
| 期权节假日 | https://www.cboe.com/us/options/holidays/csv/ | **官方 CSV**（含 2026 全年假期+交易时段），可直接入库 | ✅ 实测 200 |
| 年度期权日历 | https://cdn.cboe.com/resources/options/Cboe2026OPTIONSCalendar.pdf | PDF（官网直链） | ✅ |
| ES 期货到期 | CME 产品页（403） | — | ⚠️ 未核实；ES 季月（H/M/U/Z）第三周五与 SPX 月度 OPEX 同日，可规则推算 |

- BLS 反爬页原文：*"bot activity that doesn't conform to BLS usage policy is prohibited"*；但**数据值本身**可走 BLS Public Data API（`https://api.bls.gov/publicAPI/v2/timeseries/data/`，实测 REQUEST_SUCCEEDED，官方免费 JSON，不含发布日程）。
- BEA API `https://apps.bea.gov/api/data` 在线但需 key（免费）。
- treasury.gov 无官方经济日历 JSON（仅国债收益率 CSV 可直抓：`home.treasury.gov/resource-center/data-chart-center/interest-rates/daily-treasury-rates.csv/...`）。
- FRED（fred.stlouisfed.org）：宏观数据值 API 免费（需注册 key），**不含发布日程**；FOMC 日历以 federalreserve.gov 为准。

### 3.12 TradingView / Investing.com（灰色，仅记录）

- **TradingView**：无公开官方免费 API（https://www.tradingview.com/rest-api-reference/ 实测 404）。scanner 端点 `POST https://scanner.tradingview.com/america/scan` 实测在线但无文档，未登录 `update_mode="delayed_streaming_600"`（600 秒延迟）——功能正常、随时可变、自动化有封禁风险。**灰色**。
- **Investing.com**：无官方 API；`api.investing.com/api/financial/calendar/` 实测 403；经济日历仅网页；第三方封装均灰色。

### 3.13 Massive（原 Polygon.io，2025-10 更名——重大变化）

**更名公告**（2025-10-30，https://massive.com/blog/polygon-is-now-massive）：

> "We have just renamed Polygon.io to Massive.com... your APIs, accounts, and data quality continue to work exactly as they do today."

- `api.polygon.io` 与 `api.massive.com` 并行运行，旧 API key/SDK 继续有效。定价页：https://massive.com/pricing（按 `?product=stocks|options|indices|currencies|futures` 切换）。

**免费层（各资产 Basic，$0，需注册 key；官方定价页原文）**

- Stocks/Options/Futures Basic：5 API calls/分钟、2 年历史、EOD 聚合 K 线（分钟+日）、参考数据、公司行动、技术指标。
- **期权快照不在免费层**——官方 KB（https://massive.com/knowledge-base/article/does-massive-offer-free-trials）原文：*"the free Basic tier reaches aggregate bars, reference data, corporate actions and technical indicators. Trades, quotes, snapshots, the WebSockets and the flat files need a paid tier."*
- **指数免费层不含 SPX**——官方 KB（https://massive.com/knowledge-base/article/are-all-indices-available-through-the-basic-subscription-tier）：*"SPX, DJI, RUT and VIX all need a paid tier"*（NDX 免费）。
- 期货免费层 = 纯历史 EOD（CME/CBOT/NYMEX/COMEX）；**10 分钟延迟数据从 Futures Starter $29/月起**。

**对引擎的两个关键点**

1. **ES 结算价可从免费层拿**（文档核实，未实测——需注册 key）：`GET /futures/v1/aggs/{ticker}?resolution=1session` 的 session K 线含 `settlement_price`（官方文档 https://massive.com/docs/rest/futures/aggregates.md 原文："results[].settlement_price... Included for session, week, month, quarter, and year candles"）。注意官方会话约定：*"A futures session opens the evening before the date it settles on"*——ONH/ONL 切分需按此对齐。
2. SPX 期权链（`/v3/snapshot/options/I:SPX`，含 greeks/IV/OI）为付费档（Options Starter $29/月起，15 分钟延迟+实时 Greeks）。

**付费档速览**：Options Starter $29 → Developer $79 → Advanced $199（实时报价）；Futures Starter $29（10 分钟延迟）→ Developer $79（trades+top-of-book）→ Advanced $199（实时）；Indices Starter $49（含 SPX）→ Advanced $99。

**盘前 08:30 ET**：免费层 EOD 口径 → 昨日 session 结算 K 线应已可用（EOD 截止时刻官方未写明，**未核实**）；当日隔夜分钟线不可用。限速：免费 5 次/分钟/资产类。

### 3.14 Databento

**$125 免费额度（官方属实，但为一次性）**

- 官网主页（https://databento.com）官方文案："Sign up for $125 in free credits."
- 计费文档（https://databento.com/docs/billing/pricing-and-discounts）原文：*"$125 in free credits upon signup. Credits can be used on historical data, or can be used towards the cost of the first month of a subscription plan. These credits are shared across your team and expire after 6 months."* → **一次性试用额度（6 个月过期），不是持续免费层**。

**CME（GLBX.MDP3，含 ES、Globex 全时段）定价——官方公开 API**（`https://api.databento.com/v0/dataset/catalog`，免鉴权）

- 按量 historical（$/GB）：mbp-10 0.50 / trades·tbbo 28 / ohlcv-1m 70 / ohlcv-1d 190；live：trades 33.60 / ohlcv-1m 84 等；包月：historical $2,500/月、live $1,500/月、all $4,000/月。
- 模式（官方文档）：historical/delayed intraday = *"Usage-based or flat-rate. No monthly license fees"*；live = *"Flat-rate. Monthly license fees apply"*（CME Personal license "Included with Standard plan, up to 2 devices"）。**免费 live 数据不存在**。
- **08:30 ET 可用性**：historical API 提供 "delayed intraday data"（受交易所限制），用 $125 额度按量拉隔夜 ES 分钟线理论可行；具体延迟分钟数**未核实**。样本数据（portal 内 "real data extracts for quality assurance"）GLBX 覆盖范围**未核实**。

### 3.15 Barchart（未核实）

- barchart.com / ondemand.barchart.com / /tos 全部返回 **HTTP 202 + AWS WAF JS 挑战**，本调研环境无法通过；archive.org 本时段离线，无存档佐证。
- 免费账号范围、OnDemand 免费试用条款、`https://www.barchart.com/futures/quotes/ES*1` 页面延迟均**未核实** → 需人工浏览器访问后补。

### 3.16 其他（一句话记录）

- **EODHD**（eodhd.com）：官网确认有 EOD Options Data API / Commodities API 产品线，免费层具体范围未能从官网核实（定价页 JS 渲染）→ 不纳入推荐组合。
- **TradingView / Investing.com**：见 §3.12。

---

## 4. 缺口清单（免费源拿不到的硬需求 + 降级策略）

| # | 缺口 | 现状 | 降级策略 |
|---|---|---|---|
| 1 | **ES 官方结算价**（结算价 ≠ 收盘价，用于 pivot 锚定） | CME 官网结算页对脚本 403；IBKR 历史数据需订阅；Nasdaq Data Link CHRIS 现状未核实 | (a) **Massive（原 Polygon）免费层** session K 线的 `settlement_price`（官方文档核实，需注册免费 key，注意其「会话从结算日前一晚开始」的切分约定）；(b) Yahoo `chartPreviousClose` 近似并标注非官方；(c) 注册 Nasdaq Data Link key 实测 CHRIS/CME_ES1；(d) IBKR 加 `US Securities Snapshot and Futures Value Bundle`（$10/月，月佣金 ≥$30 豁免） |
| 2 | **期权逐笔成交方向 / 真实订单流**（TradingHub 核心增值） | 所有免费源（CBOE CDN/Tradier/Yahoo）只给每合约**累计**成交量与盘口，无 aggressor 方向、无逐笔 | (a) TradingHub 保留为唯一订单流增强源（加冗余而非替代）；(b) 本地近似：同一合约 T-1 收盘 vs T 晨间的 volume/OI/bid-ask 变化 + `tick` 字段方向统计，产出「弱订单流」指标并注明口径 |
| 3 | **≤60 秒双边实时报价**（严格口径） | 免费不存在：CME 实时是付费品（官网延迟页禁抓） | 盘前口径重构：SPX 隔夜静态 → 只需 ES 单边新鲜度。免费= Yahoo（15min）/ IBKR 延迟（10–20min）；付费最低 $5.05/月（IBKR CME L1 + Cboe 指数流）即达真·实时 |
| 4 | **BLS CPI/NFP 发布日程的机器可读性** | bls.gov 官方 URL 确认但对脚本 403（官方反爬条款）；官方 API 不含日程 | 季度人工核对写入静态配置（`calendar.json` 入库 + 代码评审更新）；发布当日 08:30 前仅作布尔标记使用 |
| 5 | **tick 级 Volume Profile** | 免费源最高粒度 = 1 分钟 K 线（Yahoo / IBKR 延迟历史） | 分钟级 OHLCV 自建近似 VP（POC/VAH/VAL 误差可接受，注明非 tick 级）；加分项不做硬依赖 |
| 6 | **CBOE CDN 无 SLA / 无文档 / 条款禁自动抓取** | 官网条款明文 "STRICTLY PROHIBITED … AUTO-EXTRACTION" 并威胁封 IP；端点结构随官网改版可能变动 | 每日 1 次低频拉取 + 固定 UA + 失败退避；schema 校验 + fixture 回归；**Tradier 免费 Lite 做同 schema 一键切换**；记录 `timestamp`(UTC) 与 `last_trade_time`(ET) 双口径 |
| 7 | **隔夜期权成交量缺失（口径确认）** | 双路实测：GTH 时段盘口（bid/ask/IV）约 5 分钟粒度更新，但 `volume`/`last_trade_time` 冻结在上一 RTH 收盘（夜盘成交 0 笔） | 引擎按「上一交易日终值」口径取用（GEX/CVR 本就如此）；若未来需要隔夜成交量 → 只有付费源（DataShop Option Trades / OPRA 分销商）；上线首周 08:25 抓样再确认一次 08:30 时点口径 |

### 未核实事项汇总（构建前需人工确认）

1. **Nasdaq Data Link CHRIS/CME_ES1** 是否仍免费提供、EOD 更新时间（ET）→ 注册免费 key 实测。
2. **Tradier 免费 Lite 未注资账户**能否即时开通生产实时行情 entitlement（官方文字称所有经纪账户持有人可得，未实测）。
3. **CBOE CDN 延迟的具体分钟数**（官方任何页面未标明；社区通称 15 分钟，非一手）。
4. **CBOE CDN 期权链在交易日 08:30 ET 整点**的成交口径（凌晨采样显示 volume 冻结在上一 RTH；整点行为未实测）。
5. **IBKR CME 延迟数据实际延迟**（定价页标 10 分钟，API 文档标 15–20 分钟，口径不一致）。
6. **CME ES 报价/结算页在 Globex 盘前的实际显示**与 settlements 导出格式（官网被 WAF 全域拦截，仅经官方存档核实）。
7. **CME DataMine / CBOE DataShop 定价明细**（均需登录）。
8. **stooq 条款原文**（端点已实测被 PoW 反爬拦截且解出后仍被挑战，未再深入）。
9. **Massive（原 Polygon）免费层**：`settlement_price` 实际可得性、EOD 截止时刻（08:30 前一日 session 是否已就绪）→ 注册免费 key 实测。
10. **Databento**：delayed intraday 具体延迟分钟数、$125 额度下拉隔夜 ES 分钟线的实际成本、GLBX 样本数据覆盖范围。
11. **Barchart**：免费账号范围 / OnDemand 试用条款 / ES 页面延迟（官网 AWS WAF 拦截，需人工浏览器访问 `barchart.com/futures/quotes/ES*1`）。
12. **EODHD**：官网确认有 EOD Options Data API / Commodities API 产品线，免费层具体范围未能从官网核实——不纳入推荐组合。
