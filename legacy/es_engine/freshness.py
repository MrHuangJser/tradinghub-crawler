# -*- coding: utf-8 -*-
"""快照时效门禁。关键：所有"分钟旧"用 Unix 纪元算（与时区无关）；
所有"交易日/盘中"判断在 America/New_York 内做，不用本地时区。

captured_at 是美东(EDT/EST)。用户机器可能任意时区，故禁止用本地 date 与 captured_at 比较。
"""

from __future__ import annotations
import time
from datetime import datetime, timezone

try:
    from zoneinfo import ZoneInfo
    ET = ZoneInfo("America/New_York")          # 自动处理 EDT/EST
except Exception:  # pragma: no cover
    ET = timezone.utc

RTH_OPEN = 9 * 60 + 30      # 09:30 ET
RTH_CLOSE = 16 * 60         # 16:00 ET
FRESH_MIN = 15              # 同日且 <15 分钟视为新鲜


def _et(epoch: float) -> datetime:
    return datetime.fromtimestamp(epoch, tz=timezone.utc).astimezone(ET)


def assess(snapshot_epoch: float | None, now_epoch: float | None = None) -> dict:
    """评估期权快照时效。返回 status/staleness_min/会话/美东时间/中文说明。

    status:
      FRESH            今日、<15 分钟（盘中地图可直接用）
      STALE_TODAY      今日但滞后>15 分钟（TradingHub 可能没在刷新）
      PRIOR_CLOSE_OK   非今日、但当前是盘前 → 昨结数据本就是盘前正解
      STALE_PRIOR_DAY  非今日、且当前不在盘前（盘中/隔夜后）→ 危险，慎用
      UNKNOWN          无 timestamp
    """
    if not snapshot_epoch:
        return {"status": "UNKNOWN", "message": "无 timestamp，无法判断时效"}

    now = now_epoch or time.time()
    snap_dt = _et(snapshot_epoch)
    now_dt = _et(now)
    staleness_min = (now - snapshot_epoch) / 60.0
    snap_date, now_date = snap_dt.date(), now_dt.date()
    is_trading_day = now_dt.weekday() < 5            # 周一~周五（未处理美股假日）
    now_min = now_dt.hour * 60 + now_dt.minute
    in_rth = is_trading_day and RTH_OPEN <= now_min < RTH_CLOSE
    pre_market = is_trading_day and now_min < RTH_OPEN
    same_day = snap_date == now_date

    if same_day and staleness_min <= FRESH_MIN:
        status, msg = "FRESH", f"今日数据，约 {staleness_min:.0f} 分钟前"
    elif same_day:
        status, msg = "STALE_TODAY", f"今日数据但已滞后 {staleness_min:.0f} 分钟（TradingHub 可能未刷新）"
    elif pre_market:
        status, msg = "PRIOR_CLOSE_OK", f"昨结数据（{snap_date} ET），盘前可用"
    else:
        phase = "盘中" if in_rth else "当前"
        status, msg = "STALE_PRIOR_DAY", f"⚠️ 数据为 {snap_date} ET（非今日），{phase}慎用——flip 可能偏数十点"

    return {
        "status": status,
        "staleness_min": round(staleness_min, 1),
        "snapshot_et": snap_dt.strftime("%Y-%m-%d %H:%M %Z"),
        "now_et": now_dt.strftime("%Y-%m-%d %H:%M %Z"),
        "session": "RTH" if in_rth else ("PRE" if pre_market else "OFF"),
        "same_day": same_day,
        "message": msg,
    }


def emoji(status: str) -> str:
    return {"FRESH": "🟢", "STALE_TODAY": "🟡", "PRIOR_CLOSE_OK": "🟡",
            "STALE_PRIOR_DAY": "🔴", "UNKNOWN": "⚪"}.get(status, "⚪")


# 可安全用作"地图"的状态（其余应警告或拒绝）
USABLE = {"FRESH", "PRIOR_CLOSE_OK"}
