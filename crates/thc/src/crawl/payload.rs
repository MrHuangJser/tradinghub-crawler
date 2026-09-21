//! 上游 payload 的强类型模型（Q14=A：全量 serde 类型化）。
//! 关键字段缺失/类型不符 → 反序列化失败 → 明确报"接口疑似变更"，
//! `--raw` dump 保留现场便于逆向更新。
//! 结构依据 `docs/ANALYSIS.md` 字段清单。TODO(Phase 2) 实现。
