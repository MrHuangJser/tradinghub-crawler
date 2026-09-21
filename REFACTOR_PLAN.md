# TradingHub Crawler → Rust 重构计划

> 版本 v1.0 · 2026-09-20。本文档是拷问式访谈（Q1–Q23）的收敛结果，作为重构的唯一事实来源。
> 算法依据：[`docs/SPX期权驱动的ES盘前分析算法-逆向重建.md`](./docs/SPX期权驱动的ES盘前分析算法-逆向重建.md)（下称"算法文档"，v1.1）。
>
> **执行状态（2026-09-21）**：Phase 0–5 已完成（调研/crawler/engine 1:1 移植+v1.1 补齐/render+run+archive/校准工具）；Phase 6（删 legacy、发 v1.0.0）**待用户首个实盘日验收后执行**。同输入对拍：与 legacy 零语义差异。`deepseek-flash` 已实测存在且视觉读图验证通过（合成截图 7/7 价位正确）。

---

## 1. 决策记录（访谈收敛）

| # | 决策点 | 结论 |
|---|---|---|
| Q1 | 动机 | 单二进制分发 + 类型安全（上游漂移编译期暴露）|
| Q2 | Rust 水平 | 熟练——直接上主流方案，无学习缓冲 |
| Q3 | 迁移策略 | **大爆炸重写**，但算法移植遵守纪律：先 1:1 移植为基线提交，优化作为后续独立提交（出 bug 可二分归因）|
| Q4/Q10 | 输出 | **重设计**，仍是 Markdown 单文件（结构重组，详见 §7）|
| Q5/Q11 | 范围 | 抓取 + 分析。砍：`--split`/`.schema.json` 生成、`--recalibrate`、`--sections`/`--compact`/`--list-tickers`、`.bat`/`.command` 包装脚本 |
| Q6 | 仓库 | 同仓库：Python 挪入 `legacy/` 保持可跑，根目录建 Rust workspace；全部验收后删 `legacy/` |
| Q7/Q21 | 平台 | 4 target：windows-x64、linux-x64(musl 静态)、macos-x64、macos-arm64；GitHub Actions 打 tag 发 release |
| Q8/Q20 | "优化"边界 | **= 补齐算法文档 v1.1 全集**（Python 缺失部分见 §5）；未解问题只动有数据支撑的，其余写入报告 limitations；权重/阈值全部 TOML 配置化 |
| Q9/Q18 | 验证 | 博主盘前分析校准（积累制）+ `--archive` 每日落盘 + `compare` 子命令；冒烟门槛：关键位残差 ≤2 SPX 点；正式框架用文档 §16.4 |
| Q12 | 结构 | Cargo workspace：`thc-engine` lib crate（纯算法，禁网络依赖）+ `thc` bin crate |
| Q13 | HTTP | async：tokio + reqwest（CBOE 等请求并发）|
| Q14 | 类型化 | **全量强类型**（Q14=A），关键字段缺失 → 明确报"接口疑似变更"；`--raw` dump 兜底 |
| Q16/Q23 | 数据源 | 调研市面全部源（含灰色接口、标注 ToS 风险），**实现只用合法免费源**；预算纯免费；调研不阻塞 engine 重写 |
| Q17/Q22 | LLM | 开发期辅助 + 离线 `parse-blogger` 子命令（**DeepSeek API + deepseek-flash 多模态**，OpenAI 兼容协议，env 配置）；**运行时主路径零 LLM** |
| Q19 | RTH 重校准 | **彻底砍掉**（A）——只做盘前一次性快照，无盘中重跑语义，接受计划过期 |

---

## 2. 目标与非目标

**目标**
- 单一静态二进制，四平台 CI 产出，双击/`cron` 即可跑，无 Python 依赖
- TradingHub/CBOE payload 全量强类型化，上游 schema 漂移 → 编译期或运行期明确报错
- engine 实现算法文档 v1.1 全集（现有 Python 只实现了子集）
- 报告 Markdown 重设计，保留中文输出
- 校准工具链：归档、博主帖解析、残差对比

**非目标（显式不做）**
- 不做 RTH/盘中重校准（Q19=A）；`session_em`/`remaining_em` 字段保留在 plan schema 中以便未来启用，但无盘中更新语义
- 不做 `--split`/schema 文件生成（原喂 AI 的权宜之计）
- 不做定时/常驻服务；不买付费数据源；运行时主路径不调 LLM

---

## 3. 仓库与 workspace 结构

```
tradinghub-crawler/
├── Cargo.toml                    # workspace 根
├── crates/
│   ├── thc-engine/               # lib crate：纯算法
│   │   └── src/
│   │       ├── lib.rs            # pub fn build_plan(EngineInput, &Config) -> Result<Plan>
│   │       ├── lineage.rs        # §4.1 数据血缘审计（四态门禁）
│   │       ├── parity.rs         # §4.2 Put-Call Parity 合成远期
│   │       ├── gex.rs            # §5.1–5.4 GEX 计算、Flip、动态/静态节点
│   │       ├── em.rs             # §5.5 EM（0DTE straddle 优先、0.87 折扣、双轨字段）
│   │       ├── basis.rs          # §6 同步基差校验与映射
│   │       ├── levels.rs         # §7–9 聚类、评分、pivot、目标、防守位、Squeeze
│   │       ├── regime.rs         # §10 波动状态 + ES–VIX 背离分离
│   │       ├── direction.rs      # 方向状态机
│   │       ├── narrative.rs      # §11 条件式文案
│   │       └── types.rs          # EngineInput / Plan / 枚举
│   └── thc/                      # bin crate：CLI + 网络 + 渲染
│       └── src/
│           ├── main.rs           # clap 入口
│           ├── crawl/
│           │   ├── mod.rs        # DataSource trait + 并发调度
│           │   ├── tradinghub.rs # 登录 + live-data + exposure 两接口
│           │   ├── cboe.rs       # VIX 家族 + 0DTE straddle
│           │   └── payload.rs    # 上游原始类型（serde，全量强类型）
│           ├── adapt.rs          # raw payload → EngineInput 映射（engine 不认识 TradingHub）
│           ├── render.rs         # Plan → Markdown
│           ├── config.rs         # config.toml + env + CLI 三级合并
│           ├── archive.rs        # 每日落盘归档
│           └── tools/
│               ├── parse_blogger.rs  # 博主帖 → blogger.json（LLM 视觉）
│               └── compare.rs        # plan.json vs blogger.json 残差表
├── legacy/                       # 原 Python（保持可跑供参照，验收后删除）
├── docs/                         # ANALYSIS.md、算法文档迁入；新增 data-sources.md
├── archive/                      # gitignored：YYYY-MM-DD/ 每日原始数据
├── fixtures/                     # 精选快照，engine 测试语料
├── config.example.toml           # 凭据 + engine 参数（权重/阈值）
└── .github/workflows/
    ├── ci.yml                    # test + clippy + fmt
    └── release.yml               # 4 target 构建 + Release
```

**关键架构约束**：`thc-engine` 不依赖 reqwest/tokio/时钟——`build_plan` 是纯函数，有效时间戳作为 `EngineInput` 的显式字段注入。编译器强制守住这条边界（Q12 的核心收益）。

---

## 4. 技术选型（已定论，不再讨论）

| 关注点 | 选择 | 理由 |
|---|---|---|
| HTTP/并发 | tokio + reqwest（rustls-tls）| Q13=B；rustls 让 linux-musl 静态链接零痛苦 |
| 序列化 | serde + serde_json + toml | — |
| CLI | clap derive | — |
| 错误 | bin 用 anyhow；engine 用 thiserror | 库给枚举错误，应用给上下文 |
| 时区 | jiff（America/New_York）| ET 时间处处涉及，jiff 是当前最优解 |
| LLM 调用 | reqwest 直连 OpenAI 兼容 `/chat/completions`，base64 图片 | 不引 SDK；`OPENAI_BASE_URL`/`OPENAI_API_KEY`/`THC_LLM_MODEL`（默认 `deepseek-flash`）env 配置；`llm` cargo feature 默认开，`--no-default-features` 可得纯瘦二进制 |
| 配置优先级 | CLI > env > `config.toml` | 沿用原三源语义，统一收进 TOML（凭据+引擎参数一个文件）|
| 退出码 | 保留 0/2/3/4 语义 | 已写进 README 的 CLI 契约 |

**Python → Rust 映射**：`spx_options.py`→`crawl/tradinghub.rs`+`payload.rs`；`market_data.py`→`crawl/cboe.rs`；`es_engine/*`→`thc-engine` 对应模块；`es_plan/es_report/es_run`→`main.rs`+`render.rs`；`schemas.py`、`*.bat`、`*.command` → 删除。

---

## 5. Engine v1.1 补齐清单（"优化"的主体）

grep 核实：Python 已实现聚类、评分、VIX1D<10 规则、regime 分层。**缺失项按依赖序逐个独立提交**（先 1:1 移植现有逻辑，再逐项补齐）：

| 序 | 文档章节 | 补齐内容 |
|---|---|---|
| 1 | — | 现有 es_engine 逻辑 1:1 移植（基线提交）|
| 2 | §4.1 | 数据血缘审计：`CURRENT`/`DELAYED_USABLE`/`STALE_CONTEXT_ONLY`/`REJECTED` 四态；`trade_date==analysis_date`、`expiry>=today`、有效时间戳非空硬门禁；报告输出血缘表 |
| 3 | §4.2 | Put-Call Parity 合成远期：`F=K+C_mid−P_mid` ATM 多配对中位数 + 离散度校验（离散→拒绝 EM/GEX/墙位计算）；报告必输 `synthetic_forward`、配对数、离散度 |
| 4 | §5.5 | EM 修订：0DTE straddle 快照优先（禁 VIX 年化替代）；可达性判断 ×0.87，止损距离不折扣；`session_em`/`remaining_em` 双字段 |
| 5 | §9.1 | 隔夜中轴进 pivot 候选（8/4 实证：转换位≈ON 中轴而非 Flip）|
| 6 | §10.1 | ES–VIX 背离 vs 期限结构倒挂分离；背离 → 降级突破质量、首触记止盈 |
| 7 | §5.4 | 动态 Gamma 状态 vs 静态 OI 节点分离建模（失效规则实现为引擎函数，供未来重校准/对比使用）|

**配置化参数**（`config.toml` 默认值=文档值，校准数据积累后调参不动代码）：评分权重 0.30/0.25/0.20/0.15/0.10、聚类容差 `max(1.0, 0.02·EM)`、pivot buffer `max(2.0, 0.05·EM)`、VIX1D 阈值 10、EM 折扣 0.87、Flip 失效阈值 10 SPX 点、basis 同步窗口 60s。

**写入报告 limitations 区**（文档 §15 未解决项）：dealer 方向假设、GEX 符号与期限权重、Squeeze 精确定义、人工裁量规则、数据源未知性等。

---

## 6. CLI 设计

```
thc run                          # 默认：抓取 → 引擎 → report.md（--archive 默认开）
thc fetch [--ticker ES_SPX] [--raw] [--output x.json]   # 仅抓取
thc report plan.json -o report.md                      # plan → Markdown
thc plan --output plan.json                            # 抓取+引擎 → plan JSON
thc parse-blogger <file|img...> [-o blogger.json]      # LLM 视觉解析（llm feature）
thc compare plan.json blogger.json                     # 逐位残差表
```

- `--em/--vwap/--onh/--onl/--pdh/--pdl/--poc` 技术位注入保留；`--no-cboe` 保留（改名 `--offline` 更贴切，跑归档/纯结构模式）
- 每次运行落盘 `archive/YYYY-MM-DD/{raw_payload.json, plan.json, report.md, blogger.json}`——**归档是默认行为**，校准集零成本积累

---

## 7. 报告重设计（Markdown 骨架提案）

```markdown
# ES 盘前计划 — 2026-09-19 (ESZ2026)
> 数据有效时间 08:31 ET · 血缘 CURRENT · Basis +28.5（同步✓）· 0DTE EM ±41

## 摘要
| 方向 | Regime | Pivot | 第一目标 |
| 偏多 | NORMAL | 7715.25 | T1 7721.5 |

## 价位地图
| 标签 | ES 价位 | SPX 源 | 构成(聚类成员) | 评分 | 距现价/EM |
（多空转换、多头 T1-3、空头 T1-3、核心防守、Squeeze Zone，按价位降序一表到底）

## 判定依据
（regime 规则命中明细：净 GEX 符号、隔夜幅/EM 比、VIX1D、背离标记）

## 数据质量与血缘
（逐源时间戳表、parity 配对数/离散度、降级/拒绝标注）

## 模型局限
（§15 未解项的固定声明）
```

与旧版差异：血缘/parity 审计结果上报告（v1.1 硬要求）；价位表带"构成"列暴露聚类来源；limitations 固定尾注。

---

## 8. Phase 0：数据源调研（先行，不阻塞 engine）

**✅ 已完成（2026-09-21）**，报告：[`docs/data-sources.md`](./docs/data-sources.md)（16 个源、一手来源+实测标注）。核心结论：

1. **TradingHub 可降级不可全替**：期权链原料（OI/vol/bid/ask/IV/greeks）免费可得，GEX/CVR/straddle 可本地重算；**免费拿不到的只有逐笔成交方向（订单流）**——TradingHub 保留为"订单流增强插件 + 交叉校验"。
2. **⚠️ CBOE CDN 被标灰**：官网条款明文禁自动抓取（"STRICTLY PROHIBITED … AUTO-EXTRACTION"）。现有 `cboe.rs`（继承自 Python 版）每日一次低频使用属灰色地带；**合法替代 = Tradier 免费 Lite**（$0 月费含 API，实时全字段链 + ORATS greeks，需注册 token）——**待用户决策**：接受低频灰色使用，还是切 Tradier。
3. **免费同步 ES 盘前报价不存在**（悬念证实）：但 08:30 ET 时 SPX 现货静态（=昨收），"双边同步"退化为"ES 单边新鲜度"——**Yahoo `ES=F`**（免鉴权 ~15min 延迟，Globex 夜盘更新）可支撑 basis=ES(08:15)−SPX(昨收) 口径，误差 <0.3%。真·双边实时最低 $5.05/月。
4. **后续增强清单**（v1.0 后）：Yahoo ES=F 补 ES 报价/ONH/ONL/PDH/PDL/昨收（自动注入 technicals，免手填）；Massive 免费层拿结算价；FOMC/CPI 日历官方源；OPEX 第三周五本地推算。

**必查清单**：CBOE 延迟链（已在文档 §21.3 实战验证）、Yahoo Finance 期权链、Tradier、Polygon/Massive 免费层、IBKR、Databento、stooq、CME 延迟报价、TradingView/英为财情（灰色，仅记录）。引擎硬需求字段：逐 strike OI/成交量/bid/ask/IV、0DTE 区分、VIX 家族、ES 与 SPX 报价、事件日历。

**已知悬念**：免费的"同步 ES 盘前报价"大概率不存在（CME 实时报价是付费品）→ 若证实，TradingHub 的 `ES_SPX`（内嵌 basis）仍是主源，basis 同步校验降级为"未知同步状态"标注而非硬门禁。**已证实并按上述口径处理。**

---

## 9. 校准机制

**流水线**：每天 `thc run`（自动归档）→ 博主帖到手后 `thc parse-blogger` → 人工复核 → `thc compare`。

**博主 JSON schema**（以 0918 实图为例，截图中"多空转换"标签旁有 7,715.19/7,717.25 两条线——解析器输出候选+置信度，`reviewed:false` 直到人工确认）：

```json
{
  "date": "2026-09-18", "contract": "ESZ2026",
  "pivot": {"value": 7717.25, "candidates": [7715.19, 7717.25], "confidence": "low"},
  "bull_targets": [7721.5, 7727.0, 7752.75],
  "bear_targets": [7697.0, 7682.0, 7662.25],
  "rules": [{"type": "no_long_below", "level": 7697}],
  "narrative_raw": "看空,或者是双向拍卖,7682-7717都会成为目标;低于7697严禁做多",
  "reviewed": false
}
```

**`thc compare` 输出**：逐标签残差表（我方 vs 博主），关键位 ≤2 SPX 点记命中；输出命中率汇总。

**诚实边界**（文档 §16.3）：<20 个交易日的校准只能当冒烟测试——证明引擎不离谱，不足以调参。正式验证按 §16.4 框架（basis/EM/Flip 假说 + 标准化触及距离 + §21.4 状态标签），积累到 60+ 日再谈权重修订。博主叙事文本不纳入评分（文档 §20.4 教训：价位与路径叙事分离记账）。

---

## 10. CI/CD

**ci.yml**（push/PR）：`cargo fmt --check`、`clippy -- -D warnings`、`cargo test`（engine 全 fixtures 跑通）。

**release.yml**（`v*` tag）：

| runner | target | 产物 |
|---|---|---|
| windows-latest | x86_64-pc-windows-msvc | `thc-windows-x64.exe` |
| macos-13 | x86_64-apple-darwin | `thc-macos-x64` |
| macos-latest | aarch64-apple-darwin | `thc-macos-arm64` |
| ubuntu-latest | x86_64-unknown-linux-musl | `thc-linux-x64` |

附 SHA256 校验文件；README 注明 macOS Gatekeeper 需 `xattr -d com.apple.quarantine`。

---

## 11. 阶段与提交节奏

| Phase | 内容 | 出口 |
|---|---|---|
| 0 | 数据源调研 → `docs/data-sources.md` | 矩阵定稿 |
| 1 | workspace 骨架 + CI + config/CLI 框架 | `cargo build` 四端通 |
| 2 | crawler：TradingHub 登录+双接口+CBOE，全量类型化 | `thc fetch` 对拍 `legacy/` 输出 |
| 3 | engine：先 1:1 移植（基线）→ §5 表逐提交补齐 v1.1 | `thc plan` + fixtures 测试全绿 |
| 4 | render + `run`/`report` 全流程 + archive | `thc run` 出新版报告 |
| 5 | `parse-blogger`（DeepSeek 视觉）+ `compare` | 0918 截图解析入库 |
| 6 | 删除 `legacy/`、README 重写、v1.0.0 tag | release 产物验收 |

Phase 0 与 1–3 并行（engine 输入契约=文档 §3 字段清单，与源无关）；Phase 2 的 crawler 层等调研结论再定第二源。

---

## 12. 风险登记

| 风险 | 应对 |
|---|---|
| 免费同步 ES 盘前报价不存在 | 调研证实后接受：TradingHub 内嵌 basis + 报告标注"同步性未验证" |
| `deepseek-flash` 模型名/多模态能力未经 API 验证 | env 可换模型名；首次冒烟测试验证视觉读图能力，不行换模型 |
| TradingHub 接口漂移 | Q14=A：强类型反序列化失败 → 明确报错"接口疑似变更" + `--raw` dump 现场 |
| 大爆炸重写期间 Python 烂尾 | `legacy/` 保持可跑；Phase 6 才删 |
| 校准样本不足时误调参 | 权重全在 config.toml，<60 日只冒烟不调参 |
| 截图解析歧义（如 0918 双候选线）| 候选+置信度+`reviewed` 位，人工复核是流程的一部分 |

## 13. 验收标准

- 四平台 release 二进制可跑 `thc run` 产出重设计报告
- engine 对 `fixtures/` 全部快照测试通过（含 lineage/parity 负例）
- `thc compare` 对 ≥3 个博主样本输出残差表
- `legacy/` 删除，仓库纯 Rust
