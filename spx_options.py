#!/usr/bin/env python3
"""
TradingHub OptionsDataViewer 数据抓取工具

每次运行：登录 tradinghubs.org -> 拉取两个期权数据接口 -> 合并并抽出指定标的
（默认 SPX）的全部期权数据 -> 输出 JSON 到 stdout。

数据来源（已逆向，详见 ANALYSIS.md）：
  GET /beta-test/api/gex/live-data       关键价位 / Gamma 阶梯 / 订单流 / exposure
  GET /beta-test/api/options-data/exposure  希腊值分布（Greeks Profile）图表数据
鉴权：POST /api/auth/login 拿到 tradinghub_user_session Cookie。

凭据来源（优先级从高到低）：
  1) CLI:   --email / --password
  2) 环境变量: TRADINGHUB_EMAIL / TRADINGHUB_PASSWORD
  3) 配置文件: ./config.json 或 ~/.config/tradinghub/config.json
     { "email": "...", "password": "..." }

用法示例：
  python3 spx_options.py                      # SPX 全量，pretty JSON 到 stdout
  python3 spx_options.py --ticker SPX         # 指定标的
  python3 spx_options.py --sections levels,orderflow,gamma_ladder
  python3 spx_options.py --output spx.json    # 写入文件
  python3 spx_options.py --raw                # 输出两接口合并后的原始 payload
  python3 spx_options.py --list-tickers       # 只列出可用标的
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests

try:  # Python 3.9+
    from zoneinfo import ZoneInfo
    _ET = ZoneInfo("America/New_York")
except Exception:  # pragma: no cover
    _ET = None

BASE_URL = "https://tradinghubs.org"
LOGIN_URL = f"{BASE_URL}/api/auth/login"
GEX_URL = f"{BASE_URL}/beta-test/api/gex/live-data"
EXPOSURE_URL = f"{BASE_URL}/beta-test/api/options-data/exposure"

UA = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36"
)

# 与页面一致的数据分区（对应页面上你曾经截图的各个板块）
ALL_SECTIONS = [
    "levels_summary",   # 关键价位概览（现价/零Gamma/最大正负OI/净GEX...）
    "gamma_ladder",     # 前列 Gamma 行权价（含 metrics + ladder）
    "classic_chain",    # 经典期权链摘要（major pos/neg vol/oi、sum_gex...）
    "state_greeks",     # 状态希腊值（major positive/negative、mini_contracts）
    "orderflow",        # 订单流看板（0DTE/1DTE+ 的 GEX/Vanna/Charm/CVR/DEX...）
    "exposure",         # 希腊值分布图表数据（oi/gex/dex/vex/chex 按行权价）
    "dte_exposure",     # 按 DTE 模式聚合的 exposure（gex/dex/vex/chex × zero/one/net）
]

DTE_METRICS = ["gex", "dex", "vex", "chex"]
DTE_MODES = ["zero", "one", "net"]


class AuthError(RuntimeError):
    pass


def load_credentials(cli_email: str | None, cli_password: str | None) -> tuple[str, str]:
    """按优先级解析凭据。"""
    email = cli_email or os.environ.get("TRADINGHUB_EMAIL")
    password = cli_password or os.environ.get("TRADINGHUB_PASSWORD")
    if email and password:
        return email, password

    for path in (Path("config.json"), Path.home() / ".config" / "tradinghub" / "config.json"):
        try:
            if path.is_file():
                cfg = json.loads(path.read_text(encoding="utf-8"))
                email = email or cfg.get("email")
                password = password or cfg.get("password")
                if email and password:
                    return email, password
        except (OSError, ValueError):
            continue

    raise AuthError(
        "未找到凭据。请用 --email/--password，或设置环境变量 "
        "TRADINGHUB_EMAIL/TRADINGHUB_PASSWORD，或在 ./config.json 放 "
        '{"email":"...","password":"..."}'
    )


def login(session: requests.Session, email: str, password: str) -> None:
    """登录并让 session 自动保存 tradinghub_user_session Cookie。"""
    resp = session.post(
        LOGIN_URL,
        json={"email": email.strip(), "password": password},
        headers={
            "User-Agent": UA,
            "Content-Type": "application/json",
            "Referer": f"{BASE_URL}/account/login",
            "Origin": BASE_URL,
        },
        timeout=30,
    )
    payload: Any = {}
    try:
        payload = resp.json()
    except ValueError:
        payload = {"ok": False, "message": resp.text[:200]}

    if not payload.get("ok"):
        err = payload.get("error") or payload.get("message") or f"HTTP {resp.status_code}"
        raise AuthError(f"登录失败：{err}")

    # 确认会话 Cookie 已下发
    if not any(c.name == "tradinghub_user_session" for c in session.cookies):
        # 某些部署可能用别的会话名；只要登录 ok 且后续接口可访问即可
        if not session.cookies:
            raise AuthError("登录返回 ok 但未下发任何 Cookie，无法继续。")


def _cache_bust(url: str) -> str:
    sep = "&" if "?" in url else "?"
    return f"{url}{sep}v={int(time.time() * 1000)}"


def fetch_json(session: requests.Session, url: str) -> dict:
    resp = session.get(
        _cache_bust(url),
        headers={
            "User-Agent": UA,
            "Accept": "*/*",
            "Referer": f"{BASE_URL}/beta-test/OptionsDataViewer",
            "Cache-Control": "no-cache",
            "Pragma": "no-cache",
        },
        timeout=30,
    )
    resp.raise_for_status()
    data = resp.json()
    if isinstance(data, dict) and data.get("ok") is False:
        raise RuntimeError(f"接口返回错误：{data.get('error') or data.get('message')}")
    return data


def _iso_et(ts: int | float | None) -> str | None:
    if not ts:
        return None
    dt = datetime.fromtimestamp(float(ts), tz=timezone.utc).astimezone(_ET) if _ET \
        else datetime.fromtimestamp(float(ts), tz=timezone.utc)
    return dt.strftime("%Y-%m-%d %H:%M:%S %Z")


def extract_ticker(merged: dict, ticker: str, sections: list[str]) -> dict:
    """从合并后的两接口 payload 中抽出指定标的的各分区。"""
    gex = (merged.get("gex") or {}).get("primary") or {}
    exp = (merged.get("exposure") or {}).get("primary") or {}
    ticker_u = ticker.upper()

    def want(name: str) -> bool:
        return not sections or name in sections

    out: dict[str, Any] = {
        "ticker": ticker_u,
        "generated_at": (merged.get("gex") or {}).get("generated_at"),
        "last_updated_at": (merged.get("gex") or {}).get("last_updated_at"),
        "stale": (merged.get("gex") or {}).get("stale"),
    }

    levels = gex.get("levels", {}).get(ticker_u)
    if levels:
        out["spot"] = levels.get("spot")
        out["captured_at"] = _iso_et(levels.get("timestamp"))

    if want("levels_summary"):
        out["levels_summary"] = levels

    if want("gamma_ladder"):
        proxy = gex.get("gex_proxy", {}).get(ticker_u)
        if proxy:
            out["gamma_ladder"] = {
                "metrics": proxy.get("metrics"),
                "ladder": proxy.get("ladder"),
            }

    if want("classic_chain"):
        cc = gex.get("classic_chain", {}).get(ticker_u)
        if cc:
            out["classic_chain"] = cc

    if want("state_greeks"):
        sg = gex.get("state_greeks", {}).get(ticker_u)
        if sg:
            out["state_greeks"] = sg

    if want("orderflow"):
        of = gex.get("orderflow", {}).get(ticker_u)
        if of:
            out["orderflow"] = of

    if want("exposure"):
        # 希腊值分布：优先用 /api/options-data/exposure 的权威数据，回退到 gex 内联的
        detail = (exp.get("exposure") or {}).get(ticker_u) \
            or (gex.get("exposure") or {}).get(ticker_u)
        if detail:
            out["exposure"] = detail

    if want("dte_exposure"):
        dte: dict[str, dict[str, Any]] = {}
        for metric in DTE_METRICS:
            dte[metric] = {}
            for mode in DTE_MODES:
                bucket = gex.get(f"{metric}_{mode}", {})
                if isinstance(bucket, dict):
                    val = bucket.get(ticker_u)
                    if val is not None:
                        dte[metric][mode] = val
        # 只在有数据时才挂上
        if any(dte[m].get(mode) is not None for m in DTE_METRICS for mode in DTE_MODES):
            out["dte_exposure"] = dte

    return out


def list_tickers(merged: dict) -> list[str]:
    gex = (merged.get("gex") or {}).get("primary") or {}
    tickers = gex.get("tickers") or []
    if not tickers:
        tickers = sorted((gex.get("levels") or {}).keys())
    return tickers


# ---------------------------------------------------------------------------
# 拆分输出：按页面板块把单个大 JSON 拆成多个小文件，每个配同名 .schema.json
# ---------------------------------------------------------------------------
def _resolve_split_schema(schema_id: str, metric: str | None = None) -> dict:
    """解析某个拆分文件的 schema；处理 dte_exposure 的 inherit + {zero,one,net} 包装。"""
    import schemas as S
    s = S.render_schema(schema_id, metric)
    if "inherit" not in s:
        return s
    base = S.render_schema(s["inherit"], metric)
    mode_fields = dict(base.get("fields", {}))
    if "strikes_override" in s:  # gex 的 strikes 与 classic_chain 略有差别
        mode_fields["strikes"] = s["strikes_override"]
    wrapped = {
        "schema_version": s.get("schema_version"),
        "title": s.get("title"),
        "page_section": s.get("page_section"),
        "data_source": s.get("data_source"),
        "notes": s.get("notes"),
        "root": {
            "type": "object",
            "desc": "本文件根对象有三个键 zero / one / net（三种 DTE 模式），"
                    "每个键的值都是下面 mode_schema 描述的结构",
            "modes": {"zero": "0DTE 当日到期", "one": "1DTE+ 次日及以后", "net": "约 90 天累计"},
        },
        "mode_schema": {
            "desc": f"单个 DTE 模式对象的结构（同 {s['inherit']} 形状）",
            "fields": mode_fields,
        },
    }
    if "value_meaning" in s:
        wrapped["mode_schema"]["value_meaning"] = s["value_meaning"]
    return wrapped


def split_output(out: dict, out_dir: str, flat: bool = False) -> list[str]:
    """把 extract_ticker 的结果按页面板块拆成目录，每个数据文件配同名 .schema.json。"""
    import schemas as S
    root = Path(out_dir)
    root.mkdir(parents=True, exist_ok=True)
    ticker = out.get("ticker", "UNKNOWN")

    # (相对路径, 数据, schema_id, metric) —— 数据为 None 的留到后面填（meta）
    plan: list[tuple[str, Any, str, str | None]] = []
    ex = out.get("exposure") or {}
    metrics = ex.get("metrics") or {}
    dte = out.get("dte_exposure") or {}

    plan.append(("01_levels_summary.json", out.get("levels_summary"), "levels_summary", None))
    plan.append(("02_key_levels/_meta.json",
                 {k: v for k, v in ex.items() if k != "metrics"}, "key_levels_meta", None))
    for m in ("oi", "gex", "dex", "vex", "chex"):
        if metrics.get(m) is None:
            continue
        sid = "key_levels_oi" if m == "oi" else "key_levels_exposure_metric"
        plan.append((f"02_key_levels/{m}.json", metrics.get(m), sid, m if m != "oi" else None))
    if out.get("gamma_ladder"):
        plan.append(("03_gamma_ladder.json", out.get("gamma_ladder"), "gamma_ladder", None))
    if out.get("orderflow"):
        plan.append(("04_orderflow.json", out.get("orderflow"), "orderflow", None))
    if out.get("classic_chain"):
        plan.append(("05_classic_chain.json", out.get("classic_chain"), "classic_chain", None))
    if out.get("state_greeks"):
        plan.append(("06_state_greeks.json", out.get("state_greeks"), "state_greeks", None))
    if dte.get("gex"):
        plan.append(("07_dte_exposure/gex.json", dte.get("gex"), "dte_exposure_gex", "gex"))
    for m in ("dex", "vex", "chex"):
        if dte.get(m):
            plan.append((f"07_dte_exposure/{m}.json", dte.get(m), "dte_exposure_state", m))

    def flat_name(rel: str) -> str:
        return f"{ticker}__{rel.replace('/', '__')}"

    written: list[str] = []  # 相对路径（数据文件）
    manifest: dict[str, dict] = {}

    for rel, data, sid, metric in plan:
        path = root / (flat_name(rel) if flat else rel)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        # 同名 .schema.json
        schema = (_resolve_split_schema(sid, metric) if sid.startswith("dte_exposure")
                  else S.render_schema(sid, metric))
        spath = path.with_suffix(".schema.json")
        spath.write_text(json.dumps(schema, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        key = flat_name(rel) if flat else rel
        manifest[key] = {
            "bytes": path.stat().st_size,
            "title": schema.get("title", sid),
            "page_section": schema.get("page_section"),
            "data_source": schema.get("data_source"),
        }
        written.append(key)

    # meta.json（含文件清单）+ meta.schema.json
    meta = {
        "ticker": ticker,
        "spot": out.get("spot"),
        "captured_at": out.get("captured_at"),
        "generated_at": out.get("generated_at"),
        "last_updated_at": out.get("last_updated_at"),
        "stale": out.get("stale"),
        "schema_note": "每个数据文件都有同名 .schema.json 说明各字段含义；建议先读 schema 再读数据。",
        "files": manifest,
    }
    mpath = root / (flat_name("meta.json") if flat else "meta.json")
    mpath.write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    mspath = mpath.with_suffix(".schema.json")
    mspath.write_text(json.dumps(S.render_schema("meta"), ensure_ascii=False, indent=2) + "\n",
                      encoding="utf-8")
    written.insert(0, mpath.name)
    return written


def parse_sections(raw: str | None) -> list[str]:
    if not raw:
        return []
    parts = [p.strip() for p in raw.split(",") if p.strip()]
    bad = [p for p in parts if p not in ALL_SECTIONS]
    if bad:
        raise SystemExit(f"未知分区：{bad}。可用分区：{ALL_SECTIONS}")
    return parts


def build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="抓取 TradingHub OptionsDataViewer 的期权数据并输出 JSON。",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="可用分区：\n  " + "\n  ".join(ALL_SECTIONS),
    )
    p.add_argument("--ticker", default="SPX", help="标的代码（默认 SPX）")
    p.add_argument("--sections", default=None,
                   help="只输出指定分区，逗号分隔；默认全部")
    p.add_argument("--pretty", action="store_true", default=True,
                   help="美化输出（默认开启）")
    p.add_argument("--compact", action="store_true",
                   help="压缩为一行 JSON")
    p.add_argument("--output", "-o", default=None,
                   help="写入文件（默认输出到 stdout）")
    p.add_argument("--raw", action="store_true",
                   help="输出两接口合并后的原始 payload（不做标的抽取）")
    p.add_argument("--split", metavar="DIR", default=None,
                   help="按页面板块拆分到目录：每个板块一个 .json + 同名 .schema.json（字段中文释义），并生成 meta.json 索引")
    p.add_argument("--split-flat", action="store_true",
                   help="配合 --split：不建子目录，文件名扁平化为 TICKER__板块__子项.json")
    p.add_argument("--list-tickers", action="store_true",
                   help="只列出可用标的后退出")
    p.add_argument("--email", default=None, help="账号邮箱")
    p.add_argument("--password", default=None, help="账号密码")
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)
    sections = parse_sections(args.sections)

    session = requests.Session()
    try:
        email, password = load_credentials(args.email, args.password)
        login(session, email, password)
        gex_payload = fetch_json(session, GEX_URL)
        exp_payload = fetch_json(session, EXPOSURE_URL)
    except AuthError as e:
        print(f"❌ {e}", file=sys.stderr)
        return 2
    except requests.HTTPError as e:
        print(f"❌ 接口请求失败：{e}", file=sys.stderr)
        return 3
    except requests.RequestException as e:
        print(f"❌ 网络请求失败：{e}", file=sys.stderr)
        return 3

    merged = {"gex": gex_payload, "exposure": exp_payload}

    if args.list_tickers:
        print(json.dumps(list_tickers(merged), indent=2, ensure_ascii=False))
        return 0

    if args.raw and not args.split:
        result: Any = merged
    else:
        result = extract_ticker(merged, args.ticker, sections)
        if not result.get("levels_summary") and not result.get("exposure"):
            available = list_tickers(merged)
            print(f"⚠️  标的 {args.ticker!r} 没有数据。可用标的：{available}",
                  file=sys.stderr)
            return 4

    if args.split:
        written = split_output(result, args.split, flat=args.split_flat)
        total = sum((Path(args.split) / w).stat().st_size for w in written)
        print(f"✅ 已拆分到 {args.split}/：{len(written)} 个文件，共 {total:,} 字节",
              file=sys.stderr)
        print("   数据文件（每个都有同名 .schema.json）：", file=sys.stderr)
        for w in written:
            print(f"     {w}", file=sys.stderr)
        return 0

    indent = None if args.compact else 2
    text = json.dumps(result, indent=indent, ensure_ascii=False)

    if args.output:
        Path(args.output).write_text(text + "\n", encoding="utf-8")
        print(f"✅ 已写入 {args.output}（{len(text)} 字节）", file=sys.stderr)
    else:
        sys.stdout.write(text + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
