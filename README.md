# thc — SPX 期权驱动的 ES 盘前分析

单一 Rust 二进制：登录 TradingHub 抓期权结构 + CBOE 免费数据 → **确定性引擎**（无 LLM）→ 中文 Markdown 盘前报告。算法依据与设计决策见 [`docs/SPX期权驱动的ES盘前分析算法-逆向重建.md`](./docs/SPX期权驱动的ES盘前分析算法-逆向重建.md)（下称"算法文档"）与 [`REFACTOR_PLAN.md`](./REFACTOR_PLAN.md)。

> 重构自 Python 版（原实现保留在 [`legacy/`](./legacy/) 供参照，验收后移除）。
> Rust 版引擎与 Python 版在相同输入下已做逐字段对拍（零语义差异），并按算法文档 v1.1 补齐了 parity 合成远期、真 0DTE EM、血缘审计等 Python 版缺失的功能。

## 安装

从 [Releases](https://github.com/MrHuangJser/tradinghub-crawler/releases) 下载对应平台二进制（windows-x64 / macos-x64 / macos-arm64 / linux-x64-musl 静态链接），或自行构建：

```bash
cargo build --release -p thc    # 或 cargo build --release（整个 workspace）
```

> macOS 首次运行若被 Gatekeeper 拦：`xattr -d com.apple.quarantine ./thc-macos-*`

## 配置

```bash
cp config.example.toml config.toml   # 填入 TradingHub 凭据
```

凭据三选一（优先级 CLI > 环境变量 > 配置文件）：`config.toml` `[tradinghub]` / 环境变量 `TRADINGHUB_EMAIL`+`TRADINGHUB_PASSWORD`。`[engine]` 段的全部参数（评分权重/聚类容差/VIX1D 阈值/EM 折扣/parity 容忍度…）默认值 = 算法文档 v1.1，**调参只改配置不动代码**。

## 使用

```bash
# 一键：抓取 → 引擎 → report.md（默认归档到 archive/当日/）
thc run
thc run --em 45 --vwap 7748 --onh 7760 --onl 7735   # 注入技术位/精确 EM
thc run --offline                                     # 纯结构模式（不联网 CBOE）
thc run --es-file es.json --spx-file spx.json --offline  # 完全离线（对拍/复盘）
thc run --no-archive                                  # 关闭归档

# 分步
thc fetch --ticker ES_SPX -o es.json                 # 仅抓取（--raw 原始 payload）
thc fetch --cboe                                      # CBOE 免费数据（免登录）
thc plan -o plan.json                                 # 抓取+引擎 → plan JSON
thc report plan.json -o report.md                     # plan → Markdown

# 校准工具（积累制：每天归档 + 博主帖解析 + 残差对比）
export DEEPSEEK_API_KEY=sk-...
thc parse-blogger 博主截图.png 博主文字.md -o blogger.json   # LLM 视觉解析（需人工复核）
thc compare archive/2026-09-21/plan.json blogger.json        # 逐位残差（≤2 点记命中）
```

## 与 Python 版（legacy/）的差异

| 能力 | Python | thc (Rust) |
|---|---|---|
| EM 口径 | CBOE 月度 straddle √T 近似 | **当日 0DTE ATM straddle 优先**（算法文档 §5.5C；实证 √T 口径高估约 30%）|
| Put-Call Parity 合成远期 | ✗ | ✅（§4.2，离散度审计，超限拒绝链内衍生指标）|
| 数据血缘审计 | freshness 时效 | 四态血缘（FRESH/STALE_TODAY/PRIOR_CLOSE_OK/STALE_PRIOR_DAY）+ 链到期日门禁 + parity/背离入报告 |
| EM 可达性折扣 | ✗ | ×0.87 只作用于目标可达性评分（止损不折扣）|
| ES–VIX 背离 / 期限结构 | ✗ | 分开判定（§10.1）|
| 引擎参数 | 硬编码 | 全部 `config.toml` 配置化 |
| `--split`/.schema.json 生成、RTH `--recalibrate` | 有 | **已移除**（设计决策见 REFACTOR_PLAN §1）|

## 退出码

`0` 成功 · `2` 凭据缺失/登录失败 · `3` 网络/接口失败 · `4` 标的无数据 · `5` TradingHub 接口疑似变更（schema 漂移，用 `thc fetch --raw` 留现场）

## 校准机制（进行中）

每个交易日：`thc run`（自动归档 raw payload/plan/报告）→ 博主帖 `parse-blogger`（LLM 读图 → 人工复核 `reviewed:true`）→ `thc compare` 出残差表。按算法文档 §16.3 的诚实边界：<20 交易日只算冒烟校准；权重修订等 60+ 日样本。

## 开发

```bash
cargo test --workspace            # 引擎合成数据测试（真实数据不入库，见 fixtures/README.md）
cargo clippy --workspace --all-targets -- -D warnings
```

结构：`crates/thc-engine`（纯算法 lib，禁止网络/时钟依赖）+ `crates/thc`（CLI/抓取/渲染/工具）。CI：fmt/clippy/test；打 `v*` tag 出四平台 Release。

## TradingHub 产品关系

- OptionsDataViewer（ODV）是 TradingHub 网站上的免费期权数据查看与分析工具，注册并登录 TradingHub 账号即可使用。
- Options Level Pro（OLP）是 TradingHub 面向 ATAS 平台提供的独立插件产品，需要下载安装到 ATAS 中使用，并采用单独授权机制。
- 本工具读取的是登录后的 ODV 页面数据，并不要求用户购买 Options Level Pro 订阅。

## 注意事项

- 本工具仅供 TradingHub 注册用户自动化访问自己账号可见的 OptionsDataViewer 数据使用，请遵守 [TradingHub 服务条款](https://tradinghubs.org)。**不要把抓到的数据再分发**（因此本仓库不提交任何真实快照，测试用合成数据）。
- 登录接口无验证码；若账号开启了设备授权/二次验证，自动登录可能失败——此时改用浏览器登录后复制 Cookie 的方式（可参考 [`docs/ANALYSIS.md`](./docs/ANALYSIS.md) 自行改造）。
- 数据为快照；血缘状态会写进报告头部，STALE_PRIOR_DAY 时慎用（flip 可能偏数十点）。
- 报告是结构判断的地图，非交易信号；模型局限固定写在每份报告尾部。

## License

[MIT](./LICENSE) © 2026 MrHuangJser。代码 MIT 开源；抓取到的行情数据版权归 TradingHub 所有，不在授权范围内。
