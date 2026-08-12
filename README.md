# TradingHub SPX 期权数据抓取工具

把 `https://tradinghubs.org/beta-test/OptionsDataViewer` 页面上的**全部期权数据（表格 + 图表）**
直接抓成 JSON，免去手动截图喂给 AI 的麻烦。

> 工作原理：该页面的所有数据（包括 echarts 图表的数据）都来自两个 JSON 接口，
> 仅靠一个会话 Cookie 鉴权。本工具用账号密码自动登录拿到 Cookie，再请求这两个接口，
> 合并后抽出指定标的（默认 SPX）输出。逆向分析详见 [`ANALYSIS.md`](./ANALYSIS.md)。

## 安装

```bash
pip install -r requirements.txt   # 仅依赖 requests
```

## 配置凭据（三选一）

**方式 A：配置文件（推荐）**

```bash
cp config.example.json config.json
# 编辑 config.json，填入你的 TradingHub 邮箱和密码
```

**方式 B：环境变量**

```bash
export TRADINGHUB_EMAIL="your_email@example.com"
export TRADINGHUB_PASSWORD="your_password"
```

**方式 C：命令行参数**

```bash
python3 spx_options.py --email your_email@example.com --password 'your_password'
```

> 凭据仅用于在本机向 `tradinghubs.org/api/auth/login` 登录换取会话 Cookie，
> 不会上传到任何第三方。`config.json` 已在 `.gitignore` 中，不会被提交。

## 一键脚本（最省事）

配好 `config.json` 后，双击即可，脚本会自动找 Python、装依赖、检查凭据、默认抓 SPX 并拆分：

| 系统 | 脚本 | 用法 |
|---|---|---|
| macOS | `fetch.command` | 双击，或终端 `./fetch.command`（首次需 `chmod +x fetch.command`，仓库里已带执行权限）|
| Windows | `fetch.bat` | 双击，或 cmd 里 `fetch.bat` |

```bash
./fetch.command                       # 默认：SPX + 拆分到 out/
./fetch.command --ticker NDX          # 换标的
./fetch.command --output spx.json     # 切回单文件模式
TICKER=SPY ./fetch.command            # 用环境变量指定标的（Windows: set TICKER=SPY && fetch.bat）
```

> 脚本只是 `spx_options.py` 的便捷封装，所有 `--ticker/--sections/--output/--split ...` 参数都能透传。


## 使用

```bash
# 默认：抓 SPX 全量数据，pretty JSON 输出到 stdout
python3 spx_options.py

# 指定其他标的（NDX / SPY / QQQ / ES_SPX / NQ_NDX / TSLA / NVDA / AAPL / MSFT / GLD / IBIT）
python3 spx_options.py --ticker NDX

# 只抓某个/某些分区
python3 spx_options.py --sections levels_summary,orderflow,gamma_ladder

# 写入文件（便于直接把文件丢给 AI 助手）
python3 spx_options.py --output spx.json

# 压缩成一行 JSON（适合管道）
python3 spx_options.py --compact | jq .

# 列出当前可用的全部标的
python3 spx_options.py --list-tickers

# 输出两接口合并后的原始 payload（不做标的抽取，调试用）
python3 spx_options.py --raw
```

### 按页面板块拆分（推荐用于喂 AI）

单文件太大时，用 `--split` 按页面板块拆成多个小文件，**每个数据文件都配一份同名 `.schema.json`**（逐字段中文释义，方便 AI 感知字段含义）：

```bash
python3 spx_options.py --split out          # 拆到 out/ 目录（结构化子目录）
python3 spx_options.py --split out --split-flat   # 扁平命名：SPX__板块__子项.json
```

产出（SPX 为例，16 个数据文件 + 16 个 schema，零数据损失）：

```
out/
├── meta.json / meta.schema.json                 索引：标的/时间/现价 + 全部文件清单
├── 01_levels_summary.json + .schema.json        【数据看板】关键价位概览
├── 02_key_levels/                               【数据看板→关键价位图】+【希腊值分布页】
│   ├── _meta / oi / gex / dex / vex / chex.json + 各自 .schema.json
├── 03_gamma_ladder.json + .schema.json          【数据看板】前列 Gamma 行权价
├── 04_orderflow.json + .schema.json             【数据看板】订单流看板
├── 05_classic_chain.json + .schema.json         经典期权链摘要
├── 06_state_greeks.json + .schema.json          状态希腊值
└── 07_dte_exposure/                             按 0DTE/1DTE+/90天 拆分的逐档数据
    ├── gex / dex / vex / chex.json + 各自 .schema.json
```

喂 AI 时建议：**先读 `meta.json` 找到要看的板块 → 读该板块的 `.schema.json` 了解字段 → 再读数据文件**。
schema 里对每个字段都标注了含义/单位/页面位置，并对 `strikes`、`mini_contracts`、`raw_row` 等"位置数组"逐位说明了含义。

## 输出结构（单文件模式，SPX 为例）

```jsonc
{
  "ticker": "SPX",
  "generated_at": "...",          // 接口生成时间（UTC）
  "last_updated_at": "...",       // 数据最后更新时间
  "stale": false,
  "spot": 7727.79,                // 现价
  "captured_at": "2026-08-11 16:00:00 EDT",  // 数据捕获时间（美东）
  "levels_summary": { ... },      // 关键价位概览：零Gamma / 最大正负OI / 净GEX ...
  "gamma_ladder": {               // 前列 Gamma 行权价
    "metrics": { ... },
    "ladder": [ { "strike", "current_value", "abs_value", "side", "distance_from_spot", "distance_percent", "lookback_values", ... } ]
  },
  "classic_chain": { ... },       // 经典期权链摘要
  "state_greeks": { ... },        // 状态希腊值
  "orderflow": { ... },           // 订单流看板（0DTE/1DTE+ 的 GEX/Vanna/Charm/CVR/DEX ...）
  "exposure": {                   // 希腊值分布（Greeks Profile）图表数据
    "underlyingPrice": 7727.79, "updatedAt": "...",
    "metrics": { "oi": [...], "gex": [...], "dex": [...], "vex": [...], "chex": [...] }
  },
  "dte_exposure": {               // 按 DTE 模式聚合
    "gex": { "zero": ..., "one": ..., "net": ... },
    "dex": { ... }, "vex": { ... }, "chex": { ... }
  }
}
```

可用分区（`--sections`）：`levels_summary`、`gamma_ladder`、`classic_chain`、
`state_greeks`、`orderflow`、`exposure`、`dte_exposure`。

## 字段速查（详见 ANALYSIS.md）

- 指标：`oi`=未平仓量, `gex`=Gamma暴露, `dex`=Delta暴露, `vex`=Vanna暴露, `chex`=Charm暴露
- DTE 模式：`zero`=0DTE, `one`=1DTE+, `net`=90天
- 订单流前缀：`z*`=0DTE, `o*`=1DTE+；`mlgamma`/`msgamma`=主要多/空Gamma，`mcall`/`mput`=主要看涨/看跌

## 退出码

- `0` 成功
- `2` 凭据缺失或登录失败
- `3` 网络/接口请求失败
- `4` 指定标的无数据

## 注意事项

- 这是付费订阅（Options Level Pro）的数据。本工具仅供订阅者自动化自己访问使用，
  请遵守 [TradingHub 服务条款](https://tradinghubs.org)。不要把抓到的数据再分发。
- 登录接口无验证码；若账号开启了设备授权/二次验证，自动登录可能失败——
  此时改用浏览器登录后复制 Cookie 的方式（可参考 `ANALYSIS.md` 自行改造）。
- 数据为快照（页面默认 30s 刷新一次后端缓存）。每次运行获取的是当时最新的一份。
