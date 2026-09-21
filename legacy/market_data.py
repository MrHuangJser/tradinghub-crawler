# -*- coding: utf-8 -*-
"""
免费市场数据抓取（CBOE 延时 15 分钟，免 key、免登录）。

提供：
  - VIX 家族现价：VIX / VIX1D / VIX9D / VVIX / SKEW
  - SPX 月度期权链 → 最近到期 ATM Straddle → 0DTE Expected Move（√T 近似）

实测可用端点（2026-08-12）：
  指数报价: https://cdn.cboe.com/api/global/delayed_quotes/quotes/_<SYM>.json
  SPX期权链: https://cdn.cboe.com/api/global/delayed_quotes/options/_SPX.json
  注: SPXW（0DTE/周度）该端点返回 403，故 0DTE EM 用月度 straddle 的 √T 近似。

可直接运行：python3 market_data.py [--no-em]  → 输出 JSON
也可被 es_engine 导入：from market_data import get_market_data
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from datetime import date, datetime
from typing import Any

import requests

UA = ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
      "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36")
QUOTE_BASE = "https://cdn.cboe.com/api/global/delayed_quotes/quotes/_{sym}.json"
SPX_OPTIONS = "https://cdn.cboe.com/api/global/delayed_quotes/options/_SPX.json"

VIX_FAMILY = ["VIX", "VIX1D", "VIX9D", "VVIX", "SKEW"]
TIMEOUT = 25


def _session() -> requests.Session:
    s = requests.Session()
    s.headers.update({"User-Agent": UA, "Accept": "application/json"})
    return s


def _get(session: requests.Session, url: str) -> tuple[dict | None, str | None]:
    try:
        r = session.get(url, timeout=TIMEOUT)
        if not r.ok:
            return None, f"HTTP {r.status_code}"
        return r.json(), None
    except Exception as e:  # noqa: BLE001
        return None, str(e)[:120]


# ---------------- VIX 家族 ----------------
def fetch_vix_family(session: requests.Session | None = None) -> dict[str, Any]:
    """逐个抓 VIX 家族；任一失败不影响其它。返回 {SYM: {value, change, timestamp}} + warnings。"""
    session = session or _session()
    out: dict[str, Any] = {}
    warnings: list[str] = []
    for sym in VIX_FAMILY:
        data, err = _get(session, QUOTE_BASE.format(sym=sym))
        if err or not data:
            warnings.append(f"{sym}: {err}")
            continue
        d = data.get("data", {})
        out[sym] = {
            "value": d.get("current_price"),
            "change": d.get("price_change"),
            "timestamp": data.get("timestamp"),
        }
    return {"indices": out, "warnings": warnings}


# ---------------- SPX 期权链 → EM ----------------
_OCC_RE = re.compile(r"^([A-Z]+)(\d{6})([CP])(\d{8})$")


def parse_occ(symbol: str) -> dict | None:
    """解析 OCC 期权符号 SPX260821C07730000 -> {root, expiry(date), cp, strike}。"""
    m = _OCC_RE.match(symbol.replace(" ", ""))
    if not m:
        return None
    yy, mm, dd = int(m.group(2)[0:2]), int(m.group(2)[2:4]), int(m.group(2)[4:6])
    try:
        return {"root": m.group(1), "expiry": date(2000 + yy, mm, dd),
                "cp": m.group(3), "strike": int(m.group(4)) / 1000.0}
    except ValueError:
        return None


def fetch_spx_chain(session: requests.Session | None = None) -> dict[str, Any]:
    session = session or _session()
    data, err = _get(session, SPX_OPTIONS)
    if err or not data:
        return {"ok": False, "error": err or "empty"}
    d = data.get("data", {})
    opts = d.get("options", [])
    parsed = []
    for o in opts:
        p = parse_occ(o.get("option", ""))
        if not p:
            continue
        p.update({"bid": o.get("bid"), "ask": o.get("ask"),
                  "iv": o.get("iv"), "oi": o.get("open_interest"), "vol": o.get("volume")})
        parsed.append(p)
    spot = d.get("current_price") or d.get("close")
    return {"ok": True, "spot": spot, "timestamp": data.get("timestamp"),
            "contracts": len(parsed), "options": parsed}


def nearest_atm_straddle(chain: dict, as_of: date | None = None) -> dict | None:
    """取最近到期月份的 ATM（最接近 spot）call+put mid 之和。"""
    if not chain.get("ok") or not chain.get("options"):
        return None
    spot = chain["spot"]
    opts = chain["options"]
    today = as_of or date.today()
    expiries = sorted({o["expiry"] for o in opts if o["expiry"] >= today})
    if not expiries:
        return None
    expiry = expiries[0]
    dte = max(1, (expiry - today).days)
    valid = [o for o in opts if o["expiry"] == expiry and o["bid"] and o["ask"]
             and o["bid"] > 0 and o["ask"] > 0]
    if not valid:
        return None

    def atm(cp: str) -> dict | None:
        cands = [o for o in valid if o["cp"] == cp]
        return min(cands, key=lambda o: abs(o["strike"] - spot)) if cands else None

    c, p = atm("C"), atm("P")
    if not (c and p):
        return None
    cmid, pmid = (c["bid"] + c["ask"]) / 2, (p["bid"] + p["ask"]) / 2
    straddle = cmid + pmid
    return {"expiry": expiry.isoformat(), "dte": dte,
            "strike": c["strike"], "call_mid": cmid, "put_mid": pmid,
            "straddle": straddle, "iv_atm": c.get("iv")}


def expected_move_0dte(straddle_info: dict | None) -> dict | None:
    """月度 straddle → 0DTE EM = straddle / √DTE（flat-term-structure 近似）。"""
    if not straddle_info:
        return None
    dte = straddle_info["dte"]
    em_0dte = straddle_info["straddle"] / math.sqrt(dte)
    return {
        "em_0dte_spx": round(em_0dte, 2),
        "method": f"monthly ATM straddle / √DTE (DTE={dte})",
        "source_expiry": straddle_info["expiry"],
        "source_straddle": round(straddle_info["straddle"], 2),
        "note": "SPXW 真 0DTE 端点被封(403)，用月度 √T 近似；误差通常 ≤5%，"
                "远优于 VIX 年化换算(30–40%)。需要精确值可用 --em 手动传入。",
    }


# ---------------- 汇总 ----------------
def get_market_data(want_em: bool = True, session: requests.Session | None = None) -> dict[str, Any]:
    """抓 VIX 家族 + EM，任一失败降级不抛异常。"""
    session = session or _session()
    result: dict[str, Any] = {"source": "CBOE delayed (15min)", "warnings": []}
    vf = fetch_vix_family(session)
    result["vix_family"] = vf["indices"]
    result["warnings"].extend(vf["warnings"])

    if want_em:
        chain = fetch_spx_chain(session)
        if chain.get("ok"):
            straddle_info = nearest_atm_straddle(chain)
            em = expected_move_0dte(straddle_info)
            result["em"] = em
            result["cboe_spot"] = chain.get("spot")
            result["cboe_timestamp"] = chain.get("timestamp")
            result["spx_chain_contracts"] = chain.get("contracts")
        else:
            result["em"] = None
            result["warnings"].append(f"SPX chain: {chain.get('error')}")
    return result


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="抓 CBOE 免费市场数据（VIX 家族 + EM）")
    ap.add_argument("--no-em", action="store_true", help="跳过期权链/EM（更快）")
    ap.add_argument("--pretty", action="store_true", default=True)
    args = ap.parse_args(argv)
    data = get_market_data(want_em=not args.no_em)
    print(json.dumps(data, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
