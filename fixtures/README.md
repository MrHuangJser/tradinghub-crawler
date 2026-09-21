# fixtures

**此目录不提交任何真实抓取数据**——TradingHub 数据版权归其所有方
（见主 README 注意事项），本仓库公开，不能 redistribution。

引擎测试策略：
- **合成数据单测**（`thc-engine` 内 `#[cfg(test)]`）：平价自洽的合成期权链、
  合成期权结构，覆盖 build_plan 端到端 + parity 接受/拒绝 + EM 优先级
- **真实数据归档**：`thc run` 每日自动落盘 `archive/YYYY-MM-DD/`（已 gitignore），
  本地跑对拍/校准用，不入库

若未来需要跨机器共享测试语料，用脱敏/合成生成器，不用真实快照。
