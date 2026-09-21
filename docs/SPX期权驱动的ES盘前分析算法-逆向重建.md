# SPX 期权驱动的 ES 盘前分析算法：逆向重建说明

> 本文根据盘前截图、价位结构及公开市场资料，逆向重建一套可能的 SPX 期权—ES 日内分析算法。
>
> **重要说明：**本文重建的是最可能的分析框架，不代表已经获得或确认原作者的真实公式、源码、数据源或参数。公开 OI 不能直接等同于 dealer 实际持仓，文中的 GEX、Call Wall、Put Wall 等均属于模型推断。
>
> **文档版本：v1.1（2026-08-06）**。本版加入 2026-08-05 样本验证，强化快照时效、合成远期、动态/静态期权节点分离、双 Expected Move、ES–VIX 背离和 RTH 重校准规则。

---

## 1. 算法目标

该算法试图在美国现金市场开盘前完成以下任务：

1. 判断当日更可能处于：
   - 震荡/均值回归；
   - 正常双向波动；
   - 趋势/波动扩张。
2. 给出 ES 日内方向状态：
   - 偏多；
   - 中性；
   - 偏空。
3. 生成一组分层价位：
   - 多空转换位；
   - 多头目标 1/2/3；
   - 空头目标 1/2/3；
   - 核心多头防守位；
   - Squeeze Zone。
4. 将 SPX/SPXW 期权结构转换为可供 ES 交易者观察的价位。
5. 根据当前价格与关键位置的关系，生成条件式盘前文字判断。

算法并不直接预测“今天必涨或必跌”，而是构造一个日内状态机：

```text
当前市场属于什么波动环境？
价格位于状态轴的哪一侧？
下一处重要节点在哪里？
什么条件会使当前判断失效？
关键支撑或阻力失守后，价格可能向哪里加速？
```

---

## 2. 总体架构

最可能的算法不是单一指标，而是一个五层混合框架：

$$
\text{最终输出}
=
\text{期权价位引擎}
+
\text{SPX→ES转换}
+
\text{波动状态分类}
+
\text{方向状态机}
+
\text{人工裁量}
$$

### 五层结构

| 层级 | 功能 | 可能使用的数据或指标 |
|---|---|---|
| 1. 价位引擎 | 生成候选支撑、阻力和波动边界 | OI、GEX、Delta、Gamma、ATM Straddle、Expected Move |
| 2. 基差转换 | 将 SPX 期权价位转换为 ES 合约价位 | 同步 ES 与 SPX 报价、ES–SPX Basis |
| 3. 波动分类 | 判断震荡、压缩或扩张环境 | 净 GEX、VIX1D、IV、预期波幅、事件风险 |
| 4. 方向状态机 | 根据价格相对转换位的位置判断偏多或偏空 | Gamma Flip、VWAP、POC、关键 Pivot |
| 5. 人工裁量 | 综合风险收益比和市场背景形成盘前文案 | 事件日、隔夜结构、历史经验、价位共振 |

流程示意：

```text
SPX/SPXW期权链
        │
        ├─ OI / Gamma / Delta / IV
        ├─ 0DTE与近周结构
        ├─ Call/Put集中执行价
        └─ ATM Straddle / Expected Move
        │
        ▼
期权候选价位引擎
        │
        ├─ Call节点
        ├─ Put节点
        ├─ Gamma Flip
        ├─ Volatility Trigger
        └─ 上下波动边界
        │
        ▼
同步ES–SPX Basis转换
        │
        ▼
与ES技术结构聚类
        │
        ├─ ONH / ONL
        ├─ PDH / PDL
        ├─ VWAP / Anchored VWAP
        ├─ Volume Profile
        └─ ATR / Opening Range
        │
        ▼
生成多空转换位与上下目标
        │
        ▼
判断波动状态和方向Bias
        │
        ▼
输出条件式盘前计划
```

---

## 3. 输入数据

### 3.1 SPX/SPXW 期权数据

建议按执行价和到期日读取：

- Call/Put Open Interest；
- Call/Put Volume；
- Bid、Ask 和 Mid Price；
- Implied Volatility；
- Delta；
- Gamma；
- Vega；
- Theta；
- 0DTE、1DTE、近周和月度到期；
- 若可获得，逐笔成交方向与实时 Options Flow。

需要明确区分：

- `SPX` 月度系列；
- `SPXW` 周度/日度系列；
- 上一交易日最终 OI；
- 当日实时成交量；
- 0DTE 与全期限数据。

### 3.2 ES 与 SPX 价格数据

- 指定月份 ES 合约，例如 `ESU26`；
- 同步 SPX 现金指数；
- ES bid/ask；
- SPX 指数值；
- ES Overnight High/Low；
- 前日 RTH High/Low/Close；
- ES 结算价；
- 现金开盘价；
- 1 分钟 OHLCV。

不能用连续符号的异步报价，与陈旧 SPX 现货值直接计算 basis。

### 3.3 ES 技术结构

- RTH VWAP；
- Anchored VWAP；
- Opening Range：OR5、OR15 或 OR30；
- 前日高低点：PDH/PDL；
- 隔夜高低点：ONH/ONL；
- Volume Profile：POC、VAH、VAL、HVN、LVN；
- ATR；
- 前日 Pivot/CPR；
- 重要 Swing High/Low；
- 整数心理关口。

### 3.4 波动率和事件数据

- VIX；
- VIX1D；
- VIX9D；
- VVIX；
- SKEW；
- SPX ATM Straddle；
- 当日经济数据；
- FOMC、CPI、NFP、OPEX 等事件标记。

---

## 4. 数据清洗与审计

在计算之前，必须确认数据在时间和产品维度上匹配。

### 4.1 时间戳、交易日与数据血缘检查（v1.1）

每条数据至少记录：

```text
symbol
contract_or_expiry
analysis_date
trade_date
download_timestamp
effective_quote_timestamp
underlying_timestamp
last_trade_timestamp
timezone
source
real_time_or_delayed
latency_seconds
```

必须区分**文件下载时间**与**行情有效时间**。例如，10:15 ET 下载的延迟链，其标的和期权报价可能只代表 10:00 ET 的市场状态；剩余期限 $T$、Gamma 重定价和事件前后标签均使用 `effective_quote_timestamp`，不得使用下载完成时间代替。

生产运行设置以下硬门禁：

```pseudo
assert snapshot.trade_date == analysis_date
assert option.expiry >= analysis_date
assert effective_quote_timestamp is not None

if snapshot.trade_date < analysis_date or option.expiry < analysis_date:
    status = STALE_CONTEXT_ONLY       # 仅可进入历史样本库

if purpose == "ES_SPX_BASIS":
    assert abs(es_timestamp - spx_timestamp) <= 60_seconds

time_to_expiry = expiry_timestamp - effective_option_timestamp
```

建议把每个输入标记为：

- `CURRENT`：满足当日、同产品和同步要求；
- `DELAYED_USABLE`：延迟已知、内部报价一致，可用于对应有效时点的结构计算；
- `STALE_CONTEXT_ONLY`：仅作历史背景，不进入今日模型；
- `REJECTED`：日期、到期日、产品或报价一致性不合格。

截图上的日期优先级高于文件创建时间。昨日 0DTE 截图即使在今日重新上传，也不能作为今日 GEX/DEX/VEX/CHEX 输入。

### 4.2 Put-Call Parity与合成远期检查（v1.1）

对同执行价、同到期日的 Call 和 Put，使用中间价反推合成远期。0DTE 近似为：

$$
F_{syn,i}\approx K_i+C_{mid,i}-P_{mid,i}
$$

更严格的欧式关系为：

$$
C-P=S e^{-qT}-K e^{-rT}
$$

生产计算不依赖单一执行价，使用 ATM 附近多组配对报价：

```pseudo
pairs = paired_calls_puts(
    same_expiry=True,
    valid_bid_ask=True,
    max_spread=spread_limit,
    near_atm=True
)

synthetic_forward = median(
    strike + call_mid - put_mid
    for pair in pairs
)

parity_dispersion = robust_dispersion(F_syn_by_strike)
assert parity_dispersion <= parity_tolerance
```

审计分三种情况：

1. **Parity集中，页面SPX也新鲜：**可计算全部期权指标并与现货交叉验证；
2. **Parity集中，但页面SPX陈旧：**可用 `synthetic_forward` 作为期权内部的ATM、EM和Gamma重定价锚；不得用陈旧页面现货直接计算Basis；
3. **Parity跨执行价离散或报价交叉：**拒绝该链的EM、Skew、GEX、Gamma Flip和墙位计算。

因此，“Parity与页面现价不一致”不再自动否定整条链；先判断是页面标的陈旧，还是期权配对本身不一致。所有报告必须同时输出 `synthetic_forward`、配对数量和离散度。

### 4.3 OI使用限制

OI 通常为上一交易日结算后的存量，不能直接识别：

- 客户买入或卖出；
- 开仓或平仓；
- Dealer 净多或净空；
- Dealer 是否已经完成对冲；
- 复杂价差、蝶式、跨期或组合仓位。

因此：

$$
\text{公开OI}
\neq
\text{Dealer实际持仓}
\neq
\text{Dealer实际对冲流}
$$

---

## 5. 期权价位引擎

## 5.1 单合约 Gamma

Black–Scholes 框架下，单份期权 Gamma 可近似表示为：

$$
\Gamma_i=
\frac{\phi(d_{1,i})}
{S\sigma_i\sqrt{\tau_i}}
$$

其中：

- $S$：SPX 参考价格；
- $\sigma_i$：该合约隐含波动率；
- $\tau_i$：剩余期限；
- $\phi(d_1)$：标准正态密度函数。

### 名义 Gamma Exposure 近似

$$
GEX_i=
 s_i\times OI_i\times M\times S^2\times\Gamma_i\times0.01
$$

其中：

- $OI_i$：未平仓量；
- $M=100$：SPX 期权合约乘数；
- $s_i$：模型假定的仓位方向符号。

最大的不确定性来自 $s_i$。公开 OI 无法告诉我们 Dealer 实际站在哪一边。因此计算结果只能称为：

- Model-Implied GEX；
- Assumption-Based GEX Proxy；
- 不能称为 Dealer 真实净 Gamma。

---

## 5.2 分期限计算

建议分别计算：

$$
GEX_{0DTE}(S)
$$

$$
GEX_{week}(S)
$$

$$
GEX_{all}(S)
$$

再按用途组合：

$$
GEX_{hybrid}(S)
=
w_0GEX_{0DTE}(S)
+w_1GEX_{week}(S)
+w_2GEX_{all}(S)
$$

一种可能的功能划分：

| 功能 | 更可能使用的期限 |
|---|---|
| 日内波动边界 | 0DTE、1DTE |
| 近价方向状态 | 0DTE + 近周 |
| 核心 Call/Put 支撑阻力 | 近周 + 全期限 |
| 结构性墙位 | 全期限或近月 |

权重必须通过历史样本验证，不能任意设定后再用当日结果解释。

---

## 5.3 Call/Put节点

对每个执行价 $K$，聚合同类暴露：

```pseudo
for each strike K:
    call_gex[K] = sum(call GEX at strike K)
    put_gex[K]  = sum(put GEX at strike K)
    net_gex[K]  = call_gex[K] + put_gex[K]
```

候选定义：

```pseudo
call_nodes = top_local_maxima(call_gex)
put_nodes  = top_local_extrema(abs(put_gex))
```

需要注意：

- 最大 Call OI 不一定等于 Call Wall；
- 最大 Put OI 不一定等于 Put Wall；
- Wall 可能按 Gamma、Delta、成交方向或专有评分定义；
- 同一执行价可能同时包含多种复杂仓位。

---

## 5.4 Gamma Flip / Zero Gamma

对一组假设 SPX 价格重新计算所有期权 Gamma：

```pseudo
for hypothetical_spot S_test in price_grid:
    total_gex[S_test] = reprice_and_sum_options(
        option_chain,
        spot=S_test
    )
```

Gamma Flip 可定义为：

$$
GEX_{total}(S^*)=0
$$

求得零交叉：

```pseudo
gamma_flip = root_or_nearest_zero_crossing(total_gex)
```

v1.1 将期权结构拆成两类：

| 类型 | 典型指标 | 主要变化来源 | 失效方式 |
|---|---|---|---|
| 动态 Gamma 状态 | Gamma Flip、净Gamma、IV敏感GEX | 现价、IV、剩余时间、成交结构 | 新旧Flip偏移、符号翻转 |
| 存量离散节点 | Call/Put OI墙、主要执行价 | 结算OI、排名变化 | 节点排名或执行价迁移 |

Flip偏移必须在同一坐标系比较，优先直接使用SPX空间：

```pseudo
flip_shift_spx = abs(new_flip_spx - premarket_flip_spx)

if flip_shift_spx > 10:
    invalidate(dynamic_gamma_levels)
    rebuild_dynamic_gamma_state()

if wall_rank_changed or wall_level_shift > wall_tolerance:
    invalidate(discrete_oi_nodes)
    rebuild_discrete_nodes()
```

不能把 `new_flip_es` 与技术 `pivot` 比较，也不能因为Flip移动就自动删除仍未迁移的OI墙。Gamma重定价中的 $T$ 使用期权报价的有效时间，而非文件下载时间。

若价格位于正 Gamma 区，理论上更容易出现：

- Dealer 逆势对冲；
- 波动受到抑制；
- 均值回归；
- 价格在重要执行价附近 pinning。

若价格进入负 Gamma 区，理论上更容易出现：

- Dealer 顺势对冲；
- 突破延续；
- 波动扩张；
- 价格穿越低流动性区域后加速。

以上结论依赖 Dealer 仓位方向假设，必须由实际价格行为确认。

---

## 5.5 Expected Move

### 方法A：ATM Straddle

对当日到期、接近平值的 Call 和 Put：

$$
EM_{0DTE}\approx C_{ATM,mid}+P_{ATM,mid}
$$

其中：

$$
C_{mid}=\frac{C_{bid}+C_{ask}}{2}
$$

$$
P_{mid}=\frac{P_{bid}+P_{ask}}{2}
$$

上下边界可先近似为：

$$
Upper=S+EM
$$

$$
Lower=S-EM
$$

若需要考虑 Skew，可分别估算上行和下行波幅：

$$
Upper=S+EM_{up}
$$

$$
Lower=S-EM_{down}
$$

### 方法B：隐含波动率换算

统计性近似：

$$
EM_{1d}\approx S\times\frac{IV}{\sqrt{N}}
$$

其中 $N$ 的选择必须与波动率指数或模型的年化约定一致。该方法只能作为风险尺度，不能冒充实际 0DTE Straddle 定价。

### 方法C：当日0DTE快照优先（2026-08-04 实证修订）

Expected Move 必须优先使用**当日 0DTE 到期的 ATM Straddle 或 ATM IV 快照**，不能用 VIX 年化换算替代：

- 2026-08-04 盘前：VIX 15.86 年化换算给出约 ±55 SPX 点，但当日期权市场实际定价的 0DTE EM 仅为 **±39–40 点**（ATM IV 9.9%，隐含区间 7,565.55–7,644.37）。沿用 VIX 换算会把上下边界系统性放宽约 30–40%，并把 1 倍 EM 之外的位置误标为“日内可达目标”。
- 统计修正：0DTE ATM Straddle 历史上平均**高估**实际波动约 13%，实际收盘约 71% 的时间落在 Straddle 隐含范围内。因此 EM 用于“目标可达性”判断时可打约 0.87 折，用于“止损距离”判断时不应打折。
- 预测市场（期货结算概率分布）可作为独立交叉验证：2026-08-04 盘前隐含日 $\sigma\approx45$–$50$ ES 点，与 0DTE 快照同向，与 VIX 换算矛盾。
- **VIX1D 极端低位规则**：当 VIX1D 处于极端低位（经验阈值 <10；2026-08-04 收盘 9.43、日内低 6.75）时，短端隐含波动率缺乏进一步压缩空间，实际波动**超过** 0DTE 定价 EM 的概率显著上升（该日实际波动最终约 1.85 倍 EM）。此时应：
  - 止损距离按 1.25–1.5 倍 EM 预留；
  - “价格触及 1 倍 EM 边界”不再自动构成反转依据；
  - 趋势扩张分支的概率赋权上调。

#### Session EM与Remaining EM（v1.1）

盘中必须保留两套互不覆盖的波动尺度：

1. `session_em`：盘前生成计划时冻结，用于衡量当天从结算/计划锚点已经消耗多少波动；
2. `remaining_em`：盘中最新ATM Straddle，用于衡量当前时点之后的剩余风险预算。

```pseudo
session_em = premarket_atm_straddle
remaining_em = current_atm_straddle

consumed_up = max(0, session_high - session_anchor) / session_em
consumed_down = max(0, session_anchor - session_low) / session_em

remaining_upper = current_synthetic_forward + remaining_em
remaining_lower = current_synthetic_forward - remaining_em
```

更新 `remaining_em` 不得重置 `consumed_up/down`。已经消耗约1倍Session EM时，即使盘中Straddle仍显示较大剩余范围，也应先执行止盈/降追价等级；Remaining EM是风险预算，不是目标保证。

---

## 6. SPX价位映射为ES价位

SPX 是现金指数，ES 是指定到期月份的期货，两者不能一对一等同。

### 6.1 实时基差

在同一秒或至少同一分钟取得：

- 指定 ES 合约 bid/ask；
- SPX 实时指数值。

计算：

$$
ES_{mid}=\frac{ES_{bid}+ES_{ask}}{2}
$$

$$
Basis_t=ES_{mid,t}-SPX_t
$$

Basis是指定合约、指定时点的状态变量，不是跨日固定参数。任何 `+29～31`、`+26` 等数值只能标注为某一交易日样本，不能写入通用模型常量。

```pseudo
if abs(es_timestamp - spx_timestamp) <= 60_seconds:
    basis = es_mid - spx_value
    basis_status = SYNCHRONIZED
else:
    basis = None
    basis_status = ASYNCHRONOUS_REJECTED
```

若只有延迟且不同步的ES与SPX报价，报告Basis区间或暂缓映射，不用两条异步行情相减生成精确价位。盘前Basis与RTH同步Basis偏离超过2点时，所有纯SPX映射位使用新Basis整体重映射。

### 6.2 映射公式

对任一 SPX 派生价位 $L_{SPX}$：

$$
L_{ES}=
\operatorname{RoundToTick}
\left(L_{SPX}+Basis_t\right)
$$

ES 最小跳动为 0.25 点，因此：

```pseudo
function map_spx_to_es(level_spx, basis):
    return round_to_tick(level_spx + basis, 0.25)
```

### 6.3 重要约束

Basis 必须来自同步报价：

```text
正确：
同一分钟ESU26 Mid − 同一分钟SPX

错误：
周一盘前ES连续符号 − 上周五SPX收盘

错误：
先观察截图价位，再反推出一个能对齐整数strike的basis
```

后者只能用于提出候选假说，不能证明真实计算路径。

---

## 7. 与ES技术结构融合

期权节点不是唯一输入，应与 ES 自身结构做聚类。

### 7.1 候选池

```pseudo
candidate_levels = [
    mapped_call_nodes,
    mapped_put_nodes,
    mapped_gamma_flip,
    mapped_upper_expected_move,
    mapped_lower_expected_move,
    overnight_high,
    overnight_low,
    prior_day_high,
    prior_day_low,
    prior_close,
    session_vwap,
    anchored_vwap,
    profile_poc,
    profile_vah,
    profile_val,
    high_volume_nodes,
    low_volume_nodes,
    atr_bands,
    opening_range_levels
]
```

### 7.2 价位聚类

若多个候选位置相距较近，将其合并为一个反应区：

```pseudo
clusters = cluster_levels(
    candidate_levels,
    tolerance=max(1.0, 0.02 * expected_move_es)
)
```

例如：

```text
SPX Put-GEX节点映射位：7530.00
ES前日低点：7529.25
Volume Profile VAL：7531.00
```

可合并为：

```text
核心支撑区：7529–7531
```

这比使用伪精确的单点更加合理。

---

## 8. 候选价位评分

对每个聚类位置评分：

$$
Score(L)=
0.30\times OptionsStrength
+0.25\times TechnicalConfluence
+0.20\times Proximity
+0.15\times ExpectedMoveAlignment
+0.10\times HistoricalReaction
$$

各项含义：

| 项目 | 含义 |
|---|---|
| OptionsStrength | OI、GEX、Delta、成交量或Flow强度 |
| TechnicalConfluence | 是否与PDH/PDL、VWAP、Profile等重合 |
| Proximity | 距离当前价格是否适合成为当日目标 |
| ExpectedMoveAlignment | 是否接近隐含波动边界 |
| HistoricalReaction | 历史上该类位置的反应质量 |

实际权重应通过历史数据训练和样本外验证，不应固定照搬上述示例。

---

## 9. 关键价位生成

## 9.1 多空转换位

候选来源：

- Gamma Flip；
- Volatility Trigger；
- Session VWAP；
- Anchored VWAP；
- Volume Profile POC；
- 前日 Pivot/CPR；
- 多指标加权中轴。

候选算法：

```pseudo
pivot = weighted_median([
    mapped_gamma_flip,
    overnight_midpoint,      # 2026-08-04 实证新增
    session_vwap,
    anchored_vwap,
    profile_poc,
    prior_day_pivot
], weights)
```

**2026-08-04 实证修正**：原作者当日的多空转换位约 7,643.5，与当日隔夜区间中轴高度吻合：

$$
\frac{ONH+ONL}{2}=\frac{7657.75+7629.00}{2}=7643.4
$$

而当日 Gamma Flip 映射位约为 7,632。这提示：转换位构造中应**始终纳入隔夜中轴候选**；原作者的转换位可能更偏技术结构（ON Mid / VWAP 类），而目标位才来自期权结构。

定义缓冲：

$$
buffer=
\max(2.0,\ 0.05\times EM_{ES})
$$

方向判断：

```pseudo
if current_es > pivot + buffer:
    bias = BULLISH
elif current_es < pivot - buffer:
    bias = BEARISH
else:
    bias = NEUTRAL
```

---

## 9.2 上下目标

```pseudo
up_candidates = levels_above(current_es)
down_candidates = levels_below(current_es)

bull_targets = rank_by_score(up_candidates)[0:3]
bear_targets = rank_by_score(down_candidates)[0:3]
```

输出：

```text
多头目标1：最近的高权重上方节点
多头目标2：中层节点
多头目标3：上侧波动边界或远端Call节点

空头目标1：最近的高权重下方节点
空头目标2：核心下方结构位
空头目标3：下侧波动边界或Squeeze节点
```

“空头目标”表示价格向下运行时的目标，不代表该位置本身一定看空。例如，空头目标2也可能同时是高质量的反转做多观察区。

---

## 9.3 核心多头防守位

从下方候选中，选择同时具备以下特征的位置：

- Put/GEX 支撑较强；
- 与技术支撑重合；
- 距当前价格足够远，具有较好的反转风险收益比；
- 失守后市场结构会明显恶化。

```pseudo
major_long_support = select_level(
    down_candidates,
    conditions=[
        strong_options_support,
        strong_technical_confluence,
        favorable_long_risk_reward,
        clear_invalidation_below
    ]
)
```

“多头底裤”可标准化表达为：

- Bullish Floor；
- Major Long Support；
- Structural Bullish Invalidation Level。

其正确含义是：

```text
支撑上方：多头结构仍可保持；
支撑附近：若出现承接，可寻找反转多；
有效失守：取消逢低做多假设。
```

---

## 9.4 Squeeze Zone

从核心支撑下方选择：

- Lower Expected Move；
- 负 Gamma 区；
- Put Delta 快速变化区；
- 低成交量节点；
- 支撑失守后的下一个结构目标。

```pseudo
squeeze_lower = select_level(
    down_candidates,
    conditions=[
        below(major_long_support),
        negative_gamma_or_low_liquidity,
        expected_move_alignment,
        breakdown_acceleration_risk
    ]
)
```

Squeeze Zone 不表示 Dealer 一定卖出，而表示：

> 核心支撑一旦失守，止损、主动卖盘、流动性下降及潜在对冲流可能共同放大行情，价格更容易向该区域扩张。

---

## 10. 波动状态分类

方向与波动状态应分开判断。

可能的状态包括：

- `CHOPPING`：围绕中轴反复；
- `COMPRESSION`：实际波幅明显低于预期；
- `NORMAL`：正常双向波动；
- `EXPANSION`：实际波幅或隐含波动率快速扩张。

候选规则：

```pseudo
if abs(current_es - pivot) <= buffer:
    regime = CHOPPING

elif realized_range < 0.60 * expected_move_es:
    regime = COMPRESSION

elif realized_range > 1.20 * expected_move_es:
    regime = EXPANSION

else:
    regime = NORMAL
```

加入净 Gamma 和事件过滤：

```pseudo
if aggregate_gex > 0 \
and price_between_major_walls \
and event_risk_is_low:
    increase_probability(RANGE_OR_MEAN_REVERSION)

if aggregate_gex < 0 \
or price_breaks_volatility_trigger \
or short_term_iv_expands_fast:
    increase_probability(TREND_OR_VOL_EXPANSION)
```

### 10.1 ES–VIX背离与期限结构必须分开（v1.1）

“VIX倒挂”只能描述期限结构；ES上涨同时VIX上涨属于价格/波动背离，二者不得混用：

```pseudo
term_backwardation = near_term_vol > longer_term_vol
es_vix_divergence = es_higher_high and vix_rising

if es_vix_divergence:
    downgrade_breakout_quality()
    first_target_touch_is_takeprofit_or_observation()
    require_price_acceptance_before_next_target()

if vix_falls_after_event:
    remove_one_volatility_veto()
    # 仍需价格在关键位上方收盘并回踩守住
```

同时观察VIX1D、VIX9D和VIX3M：VIX1D快速回落只表示事件溢价衰减；若VIX9D仍显著偏高，中短期风险尚未完全消失。

### 方向与波动状态的组合

| Bias | Regime | 解释 |
|---|---|---|
| 偏多 | 震荡/压缩 | 回落做多，不追高 |
| 偏多 | 扩张 | 突破回踩后顺势多 |
| 中性 | 震荡 | 在区间边缘交易，中部不交易 |
| 偏空 | 震荡/压缩 | 反弹做空，不在低位追空 |
| 偏空 | 扩张 | 跌破确认后顺势空 |

因此，“偏多但更多看震荡”并不矛盾：

```text
方向上略偏多；
路径上更可能来回旋转；
执行上适合等回落，而不是追涨。
```

---

## 11. 条件式盘前文案生成

```pseudo
text = []

if bias == BULLISH and regime in [CHOPPING, COMPRESSION]:
    text.append("短线偏多，但更偏震荡，不宜在近端目标前追价。")

if bias == BEARISH and regime in [CHOPPING, COMPRESSION]:
    text.append("短线偏空，但更偏震荡，优先等待反弹后的确认。")

if current_es > pivot:
    text.append(f"价格位于多空转换位{pivot}上方，上方先看{bull_targets[0]}。")

if current_es < pivot:
    text.append(f"价格位于多空转换位{pivot}下方，下方先看{bear_targets[0]}。")

if major_long_support is not None:
    text.append(
        f"若回撤至{major_long_support}并出现承接，"
        "该区域可能提供更好的多头风险收益比。"
    )

if squeeze_lower is not None:
    text.append(
        f"若有效失守核心支撑，下方{squeeze_lower}"
        "是潜在的波动和流动性加速区。"
    )
```

---

## 12. 截图案例的候选解释

截图中的主要 ESU26 价位：

| 类型 | 价位 |
|---|---:|
| 多头目标3 | 7619.00 |
| 多头目标2 | 7589.25 |
| 多头目标1 | 7569.00 |
| 多空转换位 | 约7556.50 |
| 空头目标1 | 7539.00 |
| 空头目标2 | 7529.00 |
| 空头目标3/Squeeze Zone | 7509.00 |
| 截图现价 | 约7563.00 |

### 12.1 外层波动边界

$$
7619-7563=56
$$

$$
7563-7509=54
$$

上下边界中点：

$$
\frac{7619+7509}{2}=7564
$$

因此，外层结构近似为：

$$
7564\pm55
$$

它很像以下任一结果：

- 0DTE Expected Move；
- ATM Straddle 波动边界；
- ATR 扩展；
- 上下期权墙与波动边界的重合区域。

仅凭截图不能确定具体来源。

### 12.2 多空转换位

因为：

$$
7563>7556.5
$$

状态模型可能给出短线偏多。

但距离第一目标只有：

$$
7569-7563=6
$$

因此当前位置追多的风险收益比较差，可以同时得出：

```text
方向略偏多；
不建议追多；
市场更可能震荡；
回落至核心支撑后做多更有吸引力。
```

### 12.3 7530核心多头防守区

截图文字中的“7530附近”，与蓝线7529基本一致。

最可能的标准化解释为：

```text
7529–7530是核心多头支撑/防守区；
价格到达并出现承接时，Long风险收益比较好；
有效失守后，多头结构明显降级。
```

不能将其解释为“价格一碰7530就无条件做多”。

### 12.4 7509 Squeeze Zone

若7529失守，7509可能是：

- 下侧Expected Move；
- 负Gamma加速区；
- 低流动性目标；
- 多头止损与Dealer对冲可能共同放大的区域。

执行上应理解为：

```text
7529守住：仍可按震荡/反转处理；
7529有效失守：取消逢低做多；
7509成为下一个风险节点；
7509下方继续获得接受：按趋势扩张处理。
```

---

## 13. 基差逆向的两个候选模型

仅凭单张截图，存在不唯一性。

### 候选A：约 +29 点

若：

$$
ESU26-SPX\approx+29
$$

则：

| ESU26价位 | 候选SPX价位 |
|---:|---:|
| 7619 | 7590 |
| 7589.25 | 7560.25 |
| 7569 | 7540 |
| 7556.50 | 7527.50 |
| 7539 | 7510 |
| 7529 | 7500 |
| 7509 | 7480 |

该模型的经济解释较连贯：

- 7619可能对应SPX 7590 Call节点；
- 7529可能对应SPX 7500 Put节点；
- 7556.5可能对应连续GEX曲线的7527.5零交叉；
- 7509可能对应SPX 7480下侧波动边界。

但没有同步SPX/ESU26报价，因此不能确认该basis。

### 候选B：约 -21 点

只对六个非pivot目标做数值网格拟合，可得到：

$$
ESU26-SPX\approx-21
$$

| ESU26价位 | 候选SPX价位 |
|---:|---:|
| 7619 | 7640 |
| 7589.25 | 7610.25 |
| 7569 | 7590 |
| 7539 | 7560 |
| 7529 | 7550 |
| 7509 | 7530 |

除7589.25外，其余位置几乎精确落在10点网格上。

但这也可能只是因为截图多数目标以数字`9`结尾，选择`-21`平移后自然变成整数10点网格。它是很强的数学拟合，却不构成期权因果证据。

### 2026-08-04 裁决：候选A确认，候选B证伪

两个独立交易日的同步报价与原作者网格解码给出一致答案：

| 观测 | 数值 | 口径 |
|---|---:|---|
| 2026-08-03 16:10 ET | ES 7,632.00 − SPX 7,600.50 = **+31.4** | 同步报价 |
| 2026-08-04 10:15 ET | ES 7,701.75 − SPX 7,672.64 = **+29.1** | RTH 实时同步 |
| 2026-08-04 博主网格反推 | 6 个价位中 4 个以残差 ≤0.6 点落回 SPX 期权结构（basis +31.4） | 独立网格解码 |

解码示例（basis +31.4）：

| 博主 ES 价位 | 反推 SPX | 对应期权结构 |
|---:|---:|---|
| 空头目标1 7,631 | 7,599.6 | **Gamma Flip 7,600** |
| 空头目标2 7,621 | 7,589.6 | **最大正 OI 7,590** |
| 多头目标2 7,676 | 7,644.6 | **0DTE EM 上沿 7,644.37** |
| 多头目标3 7,688 | 7,656.6 | 当日 0DTE Call 墙 7,660（用 RTH 实时 basis +29.1 映射为 7,689，残差 1 点） |

结论：

```text
真实运行 Basis ≈ +29 至 +31（随时间小幅漂移）；
候选B（-21）证伪——它只对单日数字网格拟合，无法通过同步报价检验；
本裁决遵循“先用同步报价计算 Basis，再检验价位”的顺序，不构成循环论证。
```

**Basis 是时变量**：2026-08-04 结算口径（+27.75）与 16:10 盘口口径（+31.4）相差 3.65 点。所有映射位应携带 ±2–4 点的不确定带；开盘后前 15 分钟用实时报价校验，偏离 >2 点时全部价位整体平移。

---

## 14. 开盘后的执行状态机

期权价位应作为地图，价格行为作为触发器。

### 14.1 多头场景

```pseudo
if price > pivot \
and price > rth_vwap \
and breakout_above(target_1) is accepted:
    long_bias = ACTIVE
    next_target = target_2
```

确认条件可以包括：

- 突破后1–5分钟K线收在外侧；
- 回踩没有立即跌回；
- VWAP向上倾斜；
- OR15上沿同步突破；
- 近端IV没有出现与价格不一致的剧烈扩张。

### 14.2 回落做多场景

```pseudo
if price enters major_long_support_zone:
    wait_for_rejection_or_reclaim()

if reclaim_confirmed \
and downside_expansion_stalls:
    activate_reversal_long()
```

不能在核心支撑第一次触及时闭眼挂多。

### 14.3 空头场景

```pseudo
if price < pivot \
and price < rth_vwap \
and breakdown_below(support) is accepted:
    short_bias = ACTIVE
    next_target = lower_target
```

### 14.4 Squeeze场景

```pseudo
if major_long_support_breaks \
and failed_retest_occurs \
and volatility_expands:
    cancel_mean_reversion_long()
    activate_squeeze_scenario()
```

---

## 15. 模型不可识别的部分

仅凭一张截图，无法确定：

1. 使用的是 SPX、SPXW、ES Options，还是三者混合；
2. 使用0DTE、近周还是全期限GEX；
3. Call和Put的GEX符号如何定义；
4. Dealer仓位方向如何推断；
5. 是否包含实时Options Flow；
6. Gamma Flip是否来自0DTE或全期限；
7. ES–SPX真实basis；
8. 哪些水平来自期权，哪些来自技术指标；
9. 是否在算法输出后进行了人工微调；
10. “Squeeze Zone”的精确定义。

所以本文只能称为：

> 可复现的候选算法，而不是对原作者算法的唯一恢复。

### 15.1 2026-08-04 样本后的状态更新

已由实证部分回答：

- **(7) ES–SPX 真实 Basis**：≈ +29 至 +31（见第 13 章裁决）；
- **(8) 哪些水平来自期权、哪些来自技术**：当日目标位（多头/空头目标、Squeeze）均可解码回 SPX 期权结构（Flip、最大 OI、EM 边界、0DTE Call 墙）；多空转换位与隔夜区间中轴吻合，**更可能来自技术结构**；
- **(6) Gamma Flip 的期限**：RTH 重建后的 Flip 随当日 0DTE/近月混合结构整体移动（7,600.6 → 7,642.5）；盘前静态 Flip 不能用于盘中。

仍未解决：(1)(2)(3)(4)(5)(9)(10)。

新增教训：**路径叙事文案（如“先多后空”）属于第 5 层人工裁量**。2026-08-04 当日该剧本在开盘 45 分钟内被资金流翻转推翻（CVR −32.03K → +5.31K），而同一截图的价位全部有效。回测时应只检验价位，不检验叙事。

---

## 16. 历史样本验证方案

## 16.1 数据收集

每个交易日保存：

```text
盘前截图原件
截图发布时间与时区
OCR后的全部价位与文案
指定ES合约及同步报价
SPX同步报价
ES–SPX实时basis
ES前日和隔夜结构
SPX/SPXW逐strike期权链
ATM Straddle与Expected Move
VIX/VIX1D
当日经济事件
当日ES 1分钟走势
RTH重建后的Flip/墙位/CVR/GEX成交量符号（2026-08-04 新增）
博主路径叙事与实际路径的分离记录（2026-08-04 新增）
```

建议固定采样时间（2026-08-04 修订）：

```text
08:25–08:30 ET：第一份盘前模型
09:25–09:29 ET：开盘前最终模型
10:00–10:15 ET：RTH结构校验（0DTE重建后重采期权链，对比盘前Flip/墙位）
16:05 ET后：日终审计
```

同一天多张截图不能作为多个独立交易日样本。

## 16.2 待比较模型

- A：SPX整数strike + 同步basis；
- B：GEX extrema / Gamma Flip / Call-Put Wall；
- C：Expected Move / IV Bands；
- D：纯技术支撑阻力；
- E：期权与技术结构混合模型。

## 16.3 样本量

| 样本量 | 可以完成的目标 |
|---:|---|
| 20–30日 | 初步识别规律和候选参数 |
| 60日 | 排除明显不成立的模型 |
| 120–150日 | 较可靠地区分主要模型 |
| 200–250日 | 进行稳健的样本外比较 |

## 16.4 关键检验

### Basis假说

必须先使用同步报价计算：

$$
Basis_t=ES_t-SPX_t
$$

再检验：

$$
|L_{ES,i}-(K_{SPX,i}+Basis_t)|\le0.25
$$

不能使用截图水平反推basis后，再检验同一批水平。

### Expected Move假说

检验：

$$
\frac{Upper-S_0}{EM}
$$

和：

$$
\frac{S_0-Lower}{EM}
$$

是否跨日稳定接近某个固定比例。

### Gamma Flip假说

检验多空转换位是否稳定接近：

- 0DTE GEX零交叉；
- 近周GEX零交叉；
- 全期限GEX零交叉；
- VWAP、POC或前日Pivot。

### 价位有效性

不能只统计“当天是否碰到”。应统计：

1. 上方还是下方先触及；
2. 触及后15/30分钟反转概率；
3. 突破后延续概率；
4. 收盘位于转换位哪一侧；
5. 触及距离相对于Expected Move的标准化结果。

标准化距离：

$$
D_i=\frac{|L_i-S_0|}{EM}
$$

避免离现价更近的价位因天然容易触及而获得虚假优势。

---

## 17. 风险管理

### 17.1 先定义失效，再计算仓位

ES每指数点价值为50美元：

$$
Contracts=
\frac{MaximumRiskPerTrade}
{StopDistance\times50}
$$

### 17.2 期权位不是自动入场信号

交易触发优先级建议为：

1. 价格对关键位的接受或拒绝；
2. VWAP位置和斜率；
3. OR15突破与回踩；
4. 成交量和波动率确认；
5. 期权墙、GEX和OI位置。

### 17.3 模型失效条件

出现以下情况时，应降低或取消盘前模型权重：

- 重大突发新闻；
- 经济数据造成跳空；
- Basis快速变化；
- 0DTE成交显著改变盘中Gamma结构；
- VIX1D快速扩张；
- 价格突破外层边界并获得接受；
- 期权链报价错配或延迟。

### 17.4 EM透支分级与事件窗口（2026-08-04/05 实盘校准）

透支等级固定使用盘前冻结的 `session_em`；盘中 `remaining_em` 只用于当前剩余风险范围，不重置透支计数。

以 0DTE Session EM 为单位的日内仓位规则：

| 已消耗 EM 倍数 | 规则 |
|---:|---|
| <1.0× | 正常按状态机执行 |
| 1.0–1.25× | 不新增追价仓；持仓开始分批止盈 |
| 1.25–1.5× | 只减仓不加仓；新入场仅限回踩核心簇且出现承接 |
| >1.5× | 禁止追价；任何新方向仓需独立催化剂 + RTH 接受确认 |

事件窗口规则：

```text
数据发布前5分钟至发布后5分钟：不新开仓；
持仓者在数据前减仓至半仓以下或将止损移至参照结构中轴；
数据后5–10分钟：以数据后形成的新结构替代盘前OR作为日内参照。
```

负 Gamma 加速带内禁止左侧摸顶；趋势日在 VWAP 上方只多不空，跌破 VWAP 并获接受才允许翻空。

---

## 18. 最终算法伪代码

```pseudo
function build_premarket_plan(date, es_contract):
    # 1. 获取并审计数据
    spx_quote = get_synchronized_spx_quote()
    es_quote = get_synchronized_es_quote(es_contract)
    option_chain = get_spx_spxw_option_chain()
    market_structure = get_es_market_structure(es_contract)
    volatility_data = get_volatility_and_event_data(date)

    lineage = audit_data_lineage(
        analysis_date=date,
        spx_quote=spx_quote,
        es_quote=es_quote,
        option_chain=option_chain
    )
    assert lineage.today_snapshot_is_valid
    assert lineage.option_expiry_is_valid
    assert option_chain_passes_internal_parity_checks(option_chain)

    synthetic_forward_spx = robust_synthetic_forward(option_chain)

    # 2. 仅用同步报价计算basis；否则暂缓精确映射
    basis = calculate_basis_if_synchronized(
        es_quote,
        spx_quote,
        max_skew=60_seconds
    )

    # 3. 计算期权暴露
    gex_0dte = calculate_model_gex(
        option_chain.filter(expiry="0DTE"),
        effective_timestamp=lineage.effective_option_timestamp
    )

    gex_week = calculate_model_gex(
        option_chain.filter(expiry="near_week"),
        effective_timestamp=lineage.effective_option_timestamp
    )

    gex_all = calculate_model_gex(
        option_chain,
        effective_timestamp=lineage.effective_option_timestamp
    )

    hybrid_gex = weighted_sum(
        gex_0dte,
        gex_week,
        gex_all
    )
    flow_state = get_orderflow_indicators(option_chain)

    # 4. 提取SPX期权节点
    call_nodes_spx = find_call_gex_nodes(hybrid_gex)
    put_nodes_spx = find_put_gex_nodes(hybrid_gex)
    gamma_flip_spx = find_zero_crossing(hybrid_gex)

    session_em_spx = calculate_expected_move(
        option_chain,
        method="ATM_STRADDLE"
    )
    remaining_em_spx = session_em_spx

    upper_em_spx = synthetic_forward_spx + remaining_em_spx.up
    lower_em_spx = synthetic_forward_spx - remaining_em_spx.down

    if basis is None:
        return PendingBasisPlan(
            reason="ES/SPX报价不同步，暂缓精确ES映射",
            gamma_flip_spx=gamma_flip_spx,
            call_nodes_spx=call_nodes_spx,
            put_nodes_spx=put_nodes_spx,
            synthetic_forward_spx=synthetic_forward_spx,
            upper_em_spx=upper_em_spx,
            lower_em_spx=lower_em_spx,
            session_em=session_em_spx,
            remaining_em=remaining_em_spx,
            data_lineage=lineage
        )

    # 5. 映射到ES
    option_levels_es = map_all_spx_levels_to_es(
        levels=[
            call_nodes_spx,
            put_nodes_spx,
            gamma_flip_spx,
            upper_em_spx,
            lower_em_spx
        ],
        basis=basis,
        tick=0.25
    )

    # 6. 合并ES技术位
    candidate_levels = option_levels_es + [
        market_structure.ONH,
        market_structure.ONL,
        market_structure.PDH,
        market_structure.PDL,
        market_structure.VWAP,
        market_structure.POC,
        market_structure.VAH,
        market_structure.VAL,
        market_structure.ATR_BANDS
    ]

    clusters = cluster_nearby_levels(candidate_levels)
    scored_levels = score_level_clusters(clusters)

    # 7. 生成状态轴和目标
    pivot = choose_pivot(
        gamma_flip=map_to_es(gamma_flip_spx, basis),
        vwap=market_structure.VWAP,
        poc=market_structure.POC,
        prior_pivot=market_structure.PRIOR_PIVOT
    )

    bias = classify_direction(
        price=es_quote.mid,
        pivot=pivot,
        buffer=max(2.0, 0.05 * session_em_spx.total)
    )

    regime = classify_volatility_regime(
        aggregate_gex=hybrid_gex.current,
        expected_move=session_em_spx,
        realized_range=market_structure.overnight_range,
        volatility_data=volatility_data
    )

    bull_targets = select_top_levels_above(
        scored_levels,
        es_quote.mid,
        count=3
    )

    bear_targets = select_top_levels_below(
        scored_levels,
        es_quote.mid,
        count=3
    )

    major_long_support = choose_major_long_support(
        scored_levels,
        es_quote.mid
    )

    squeeze_zone = choose_lower_squeeze_zone(
        scored_levels,
        major_long_support,
        lower_expected_move=map_to_es(lower_em_spx, basis)
    )

    # 8. 生成条件式文字
    narrative = generate_conditional_narrative(
        price=es_quote.mid,
        pivot=pivot,
        bias=bias,
        regime=regime,
        bull_targets=bull_targets,
        bear_targets=bear_targets,
        major_long_support=major_long_support,
        squeeze_zone=squeeze_zone
    )

    return PremarketPlan(
        bias=bias,
        regime=regime,
        pivot=pivot,
        bull_targets=bull_targets,
        bear_targets=bear_targets,
        major_long_support=major_long_support,
        squeeze_zone=squeeze_zone,
        narrative=narrative,
        gamma_flip_spx=gamma_flip_spx,
        synthetic_forward_spx=synthetic_forward_spx,
        synchronized_basis=basis,
        session_em=session_em_spx,
        remaining_em=remaining_em_spx,
        dynamic_gamma_levels=[gamma_flip_spx, hybrid_gex.current],
        oi_walls=[call_nodes_spx, put_nodes_spx],
        cvr_0dte=flow_state.cvr_0dte,
        net_gex_volume=flow_state.net_gex_volume,
        data_lineage=lineage,
        data_timestamp=lineage.effective_market_timestamp,
        limitations=list_model_limitations()
    )

# 注意：盘前计划不是终稿。
# RTH 开盘后 30–60 分钟必须执行 rth_recalibration(plan)（见第 20.5 节）：
# 0DTE 重建可使 Flip/墙位移动数十点，资金流符号翻转会使路径叙事作废。
```

---

## 19. 结论

这套逆向算法最可能是：

> 以 SPX/SPXW 期权结构产生离散墙位和连续状态线，以 ATM Straddle 或短端 IV 形成波动边界，通过同步 ES–SPX basis 映射到指定 ES 合约，再与 ES 的 VWAP、前高低和 Volume Profile 聚类，最终生成多空转换位、上下三级目标、核心多头防守位和 Squeeze Zone，并由人工根据风险收益比形成盘前文字判断。

当前可以较高置信度恢复的部分：

- 分层价位状态机；
- 多空转换位的功能；
- 方向与波动状态分离；
- 核心支撑与Squeeze Zone的关系；
- 期权位必须转换为ES价位；
- 价格确认优先于静态OI；
- **ES–SPX Basis必须逐日、逐合约、逐时点从同步行情计算；+29～31仅为2026-08-04样本值**；
- **目标位来自期权结构、转换位偏技术结构（2026-08-04 解码）**；
- **盘前结构必须经 RTH 0DTE 重建校验（2026-08-04 实证，见第 20 章）**。

当前无法唯一恢复的部分：

- 原作者实际使用的数据供应商；
- Dealer仓位方向假设；
- GEX符号和期限权重；
- 实时Flow分类；
- 各条线的确切指标来源；
- 人工调整规则（含路径叙事文案的生成规则）。

因此，这份算法应被用作：

1. 建立可复现的候选模型；
2. 收集历史截图后进行逐日验证；
3. 比较期权模型、Expected Move和纯技术模型；
4. 最终形成经过样本外检验的ES盘前分析系统。

---

## 20. 2026-08-04 实战验证与算法修订

本章记录该算法第一次在完整交易日中的实战检验。当日为美东周二，SPX 前收 7,600.50（周一 +1.48% 大涨），ES 结算 7,628.25；当日 RTH 开盘后出现趋势扩张，上午 10:15 ET 已至 7,701.75（日高约 7,703.5，自结算 +73.5 点 ≈ 1.85 倍当日 0DTE EM）。

### 20.1 当日时间线与价位命中记录

| 时间 (ET) | 事件 | 价位行为 | 对应模型位 |
|---|---|---|---|
| 20:30–21:00（前晚） | 隔夜低 7,629.00 | 短暂跌破 Flip 映射 7,632 后收回 | 转换位首次测试 |
| 04:45–05:00 | 回踩 7,633–7,634 获承接 | Flip 映射上方确认 | 转换位二次确认 |
| 07:30–08:00 | 拉升至 ONH 7,657.75 | 越过 52 周高映射 7,652.25 | 决策区 |
| 09:26 | 盘前冲 7,662 | 进入 T1 阻力带 7,661.5–7,666 | 双方 T1 |
| 09:30–09:56 | RTH：7,660 → 7,686.25 | 26 分钟 +58 点 ≈ 1.45× EM；依次穿越 T1 带、双方 T2 7,676、延伸位 7,681.5；**首波日高停在博主 T3 7,688 下方 1.75 点** | 趋势扩张分支触发 |
| 10:00 | JOLTS（共识 7.44M） | 未构成利空，价格续拉 | 事件窗口 |
| 10:14–10:15 | 7,701.75，日高 ~7,703.5 | 停在 SPX 7,675（当日最大成交量行权价）映射 7,704 下方 0.5 点；CVD 顶背离（9,263 < 前峰 10,746） | 决策带 7,700–7,704 |

### 20.2 Basis 裁决摘要

候选 A（+29～31）在2026-08-04样本中经两次独立同步报价与博主网格解码三重确认，候选 B（−21）在该样本中证伪。详见第13章。**该结果验证了“同步Basis映射”这一方法，但+29～31不是跨日固定参数；每个交易日必须重新计算。**

### 20.3 RTH 0DTE 重建：盘前结构的失效案例

同一套看板在 8/3 16:00（盘前输入）与 8/4 10:14（RTH 实时）的对比：

| 指标 | 8/3 16:00 | 8/4 10:14 | 变化 |
|---|---:|---:|---|
| SPX 现价 | 7,600.61 | 7,672.64 | +72 |
| 零 Gamma | 7,600.61 | **7,642.50** | **上移 42 点** |
| 0DTE GEX | −26.19K | **+7.19K** | 翻正 |
| 0DTE CVR | −32.03K | **+5.31K** | 翻正 |
| 1DTE+ CVR | −10.10K | +665.25 | 翻正 |
| 净 GEX 成交量 | −1.68M | **+158.22K** | 翻正 |
| 0DTE 多/空 Gamma | 7.59K / 7.60K | **7.66K / 7.70K** | 上移 60–100 点 |
| 最大正 OI | 7,590 | **7,650** | 上移 60 点 |
| 最大正成交量 | 7,593.80 | **7,675** | 当日资金主战场 |

结论：

```text
盘前模型基于隔夜/昨收残留结构；
RTH 开盘 30–60 分钟内 0DTE 重建可使 Flip 和墙位移动数十点；
当 RTH Flip 与盘前 Flip 偏离 >10 SPX 点时，盘前动态Gamma状态作废，以新结构重算；未迁移的离散OI墙保留但重新评分。
```

### 20.4 资金流符号翻转 = 剧本失效器

原作者当日文案为“上行空间不大，空头在 76 附近介入，可能先多后空”。该剧本未兑现：开盘后 CVR 由 −32.03K 翻至 +5.31K、净 GEX 成交量由 −1.68M 翻至 +158.22K——**空头剧本所需的资金流燃料在开盘 45 分钟内被抽走**。

教训：

1. 价位（结构判断）与路径叙事（人工裁量）必须分开存档、分开评分；
2. 盘前剧本在 RTH 资金流符号翻转时自动作废，价位可保留；
3. “先 X 后 Y”类剧本的置信度应恒低于价位本身。

### 20.5 趋势日状态机校准

当日暴露的盘前模型偏差及修正：

- **隔夜压缩 + 大涨次日 + 贴近墙上**是扩张前兆：隔夜全幅仅 29 点（<0.75× EM），RTH 26 分钟释放 +58 点。当 `overnight_range < 0.75×EM` 且 `VIX1D < 10` 时，上调 EXPANSION 先验概率；
- **趋势日中阻力带可能被直接穿越**：T1 带 7,661.5–7,666 未产生任何停顿。趋势日里阻力位只作止盈参考，不作逆势入场；
- **角色转换**：双方共同天花板 7,676 被接受后转为当日最重要支撑（前阻力 → 新支撑）；
- **动能衰竭信号**（10:15 ET 实盘确认）：价格新高 7,701.75 而 CVD 更低高（9,263 < 10,746），且拉升段一路出现红色大单吸收气泡——上涨由被动限价承接维持而非攻击性买盘。此组合出现后禁止追多；
- **决策带定位**：日高 7,703.5 停在“整数关 7,700 + 当日最大成交量行权价映射 7,704”叠加带，误差 0.5 点——行权价成交量节点在 RTH 的优先级应上调。

伪代码补丁：

```pseudo
# RTH 开盘后30–60分钟执行；重大数据后5–15分钟可额外执行一次
function rth_recalibration(premarket_plan):
    new_chain = get_spx_spxw_option_chain()
    lineage = audit_data_lineage(new_chain)
    assert lineage.today_snapshot_is_valid
    assert new_chain_passes_internal_parity_checks(new_chain)

    new_flip_spx = find_zero_crossing(
        calculate_model_gex(
            new_chain,
            effective_timestamp=lineage.effective_option_timestamp
        )
    )
    flip_shift_spx = abs(
        new_flip_spx - premarket_plan.gamma_flip_spx
    )

    if flip_shift_spx > 10:
        invalidate(premarket_plan.dynamic_gamma_levels)
        rebuild_dynamic_gamma_state(new_chain)

    new_walls = find_discrete_oi_nodes(new_chain)
    if wall_rank_changed(new_walls, premarket_plan.oi_walls) \
    or wall_level_shift(new_walls, premarket_plan.oi_walls) > wall_tolerance:
        invalidate(premarket_plan.oi_walls)
        rebuild_discrete_nodes(new_walls)
    else:
        retain_and_rescore(premarket_plan.oi_walls)

    new_basis = calculate_basis_if_synchronized(
        current_es_quote(),
        current_spx_quote(),
        max_skew=60_seconds
    )
    if new_basis is not None:
        remap_all_spx_levels(new_basis)

    remaining_em = current_atm_straddle(new_chain)
    update_remaining_risk_envelope(remaining_em)
    preserve_session_em_consumption(premarket_plan.session_em)

    flow = get_orderflow_indicators(new_chain)
    if sign(flow.cvr_0dte) != sign(premarket_plan.cvr_0dte) \
    or sign(flow.net_gex_volume) != sign(premarket_plan.net_gex_volume):
        invalidate(premarket_plan.narrative_path)  # 仅路径叙事作废

# 趋势日补充规则
if realized_range_from_settle > 1.4 * em_0dte and time < "11:00":
    regime = EXPANSION_CONFIRMED
    resistance_zones_are_takeprofit_only()
    only_long_above_vwap()
    no_chase_beyond(1.5 * em_0dte)

if price > vwap and cvd_lower_high and price_higher_high:
    warn("吸收主导：禁止追多，等待回踩承接")

if broken_resistance_reclaimed_from_above:
    mark_role_reversal(level)                      # 前阻力 → 新支撑
```

### 20.6 命中/失误清单（诚实记账）

命中：

- Basis +29～31（两次同步校验 + 博主网格解码，残差 ≤1 点）；
- 博主 T2 7,676 = 本方审计修正后的上 EM 7,675.75；该位被突破后发生角色转换回测；
- 博主 T3 7,688：首波日高 7,686.25 停于其下 1.75 点；
- 博主 BT1 7,631 ≈ 本方 Flip 映射 7,632：隔夜低 7,629 附近承接；
- 博主 BT2 7,621 ≈ 本方下 T1 7,621.5（日内未触及，不记分）；
- 盘前审计将 EM 由 ±55 下修为 ±39–40，避免了上方目标系统性偏高；
- 盘前“以 RTH 新链为准”提醒被证实必要（Flip 上移 42 点）。

失误/教训：

- 盘前基准情形给“冲高被拒/消化”的主权重过高，实际开盘直接进入趋势扩张；Regime 先验应随“隔夜压缩 + VIX1D 极端低位”上调；
- 原作者“先多后空”剧本未兑现——再次确认叙事层不可回测、不可依赖；
- 零 Gamma 映射曾出现 0.5 点取整误差（7,631.5 vs 7,632.0）：映射报告应保留一位小数中间值，取整仅在展示层进行；
- VIX 换算 EM（±55）与当日 0DTE 定价（±39–40）偏差 38%：EM 必须以当日 0DTE 快照为准（已写入第 5.5 方法C）。


## 21. 2026-08-05 实战验证与v1.1修订

### 21.1 数据质量与事件窗口

当日用户补充的五张期权图均标注 `2026-08-04 15:59:59 EST / SPX 7735.60`，属于昨日已到期的0DTE快照；它们只能进入历史样本库。生产模型改用当日Cboe延迟链，并同时记录下载时间与有效标的时间。

09:45 ET S&P Global Services PMI终值 `53.6`；10:00 ET ISM Services为 `54.1`，Employment `47.4`、Prices `70.3`。这是“总量扩张、就业收缩、价格黏性增强”的混合事件，适合在数据后重新计算Remaining EM、Flip与波动过滤器，而不是沿用盘前路径叙事。

### 21.2 价位与路径的分离评分

| 对象 | 盘前值 | 盘中结果 | 状态标签 |
|---|---:|---:|---|
| Pivot | 7,786 | 截图时仍守住 | `HELD` |
| 第一多头目标 | 7,815.75 | 日高约7,820.25，随后回落至7,806.25 | `HIT_REJECTED` |
| 第二多头目标 | 7,831 | 截图时未触及 | `NOT_TOUCHED` |
| 盘前ATM Straddle | 44.65 | 前收至日高约46.5点，约1.04×EM | `SESSION_EM_CONSUMED` |

第一目标精度得到验证，但“触及”不等于“接受”。当价格在约1倍Session EM处命中目标、同时VIX仍高于17并与ES同涨时，应记录为止盈/观察事件，不直接升级下一目标。

### 21.3 10:00 ET后期权结构重建

Cboe抓取时间 `14:15:25 UTC`，延迟标的有效时间 `10:00:21 ET`。当日0DTE重算：

| 指标 | 盘前/较早值 | 10:00有效快照 | 结论 |
|---|---:|---:|---|
| 合成远期 | 约7,775 | 7,783.7 | 使用Parity合成远期更新 |
| ATM Straddle | 44.65 | 42.9 | 事件溢价衰减，更新Remaining EM |
| Gamma Flip | 7,707.1 | 7,731.9 | 上移24.7点，动态Gamma状态失效重建 |
| 主要Call OI墙 | 7,800/7,825/7,850 | 基本未变 | 保留并重新评分 |
| 主要Put节点 | 7,700/7,720/7,680 | 基本未变 | 保留并重新评分 |

该样本直接证明：**Flip大幅移动不等于全部OI墙失效**。连续Gamma状态和离散存量节点必须分开管理。

按当时约 `+26` 的开盘Basis仅作样本映射，新Flip约对应 `7,757.9 ES`，落入原计划 `7,757–7,763` 核心防守带。这个结果验证了价位聚类方法，但不把 `+26` 固化为未来参数。

### 21.4 v1.1正式状态标签

后续样本统一使用：

- `HIT_REJECTED`：触及后未获得5/15分钟接受；
- `HIT_ACCEPTED`：关键周期收于其外侧且回踩守住；
- `BROKEN_RETEST_HELD`：突破后完成角色转换；
- `NOT_TOUCHED`：样本窗口内未触及；
- `INVALIDATED_BEFORE_TOUCH`：触及前模型已因数据、Basis、Flip或资金流失效。

价位命中率、接受后延续率和路径叙事准确率必须分别统计。

---

## 免责声明

本文仅用于市场结构研究、算法逆向和教育讨论，不构成投资建议、交易建议或收益承诺。期权定位模型依赖大量不可直接观察的假设；任何盘前价位均应由实时价格行为、成交量、VWAP、Opening Range和风险管理规则确认。
