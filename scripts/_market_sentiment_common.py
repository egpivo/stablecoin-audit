"""Shared helpers for Fear & Greed ingest and benchmark joins (stdlib only)."""
from __future__ import annotations

import csv
import json
from collections import Counter
from dataclasses import dataclass
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any, Iterable

SOURCE_DEFAULT = "alternative.me/crypto-fear-and-greed-index"

FEAR_LABELS = frozenset({"Extreme Fear", "Fear"})
NEUTRAL_LABELS = frozenset({"Neutral"})
GREED_LABELS = frozenset({"Greed", "Extreme Greed"})


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def parse_iso_utc(s: str) -> datetime:
    s = s.strip()
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    dt = datetime.fromisoformat(s)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def iso_to_date_utc(s: str) -> date:
    return parse_iso_utc(s).date()


def classification_from_value(value: int) -> str:
    """Map index value to standard bucket when API label missing."""
    if value <= 24:
        return "Extreme Fear"
    if value <= 44:
        return "Fear"
    if value <= 55:
        return "Neutral"
    if value <= 74:
        return "Greed"
    return "Extreme Greed"


def analysis_regime(value_classification: str) -> str:
    if value_classification in FEAR_LABELS:
        return "fear"
    if value_classification in NEUTRAL_LABELS:
        return "neutral"
    if value_classification in GREED_LABELS:
        return "greed"
    return "neutral"


def dominant_classification(classifications: Iterable[str], mean_value: float) -> str:
    counts = Counter(classifications)
    if not counts:
        return classification_from_value(int(round(mean_value)))
    top_n = counts.most_common()
    if len(top_n) == 1 or top_n[0][1] > top_n[1][1]:
        return top_n[0][0]
    return classification_from_value(int(round(mean_value)))


@dataclass(frozen=True)
class FearGreedRow:
    date_utc: date
    value: int
    value_classification: str
    source: str


def parse_fng_api_payload(payload: dict[str, Any], source: str = SOURCE_DEFAULT) -> list[FearGreedRow]:
    rows: list[FearGreedRow] = []
    for item in payload.get("data", []):
        ts = int(item["timestamp"])
        d = datetime.fromtimestamp(ts, tz=timezone.utc).date()
        value = int(item["value"])
        vc = item.get("value_classification") or classification_from_value(value)
        rows.append(FearGreedRow(d, value, vc, source))
    rows.sort(key=lambda r: r.date_utc)
    return rows


def write_fear_greed_csv(path: Path, rows: list[FearGreedRow]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["date_utc", "value", "value_classification", "source"])
        for r in rows:
            w.writerow([r.date_utc.isoformat(), r.value, r.value_classification, r.source])


def read_fear_greed_csv(path: Path) -> list[FearGreedRow]:
    out: list[FearGreedRow] = []
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            out.append(
                FearGreedRow(
                    date.fromisoformat(row["date_utc"]),
                    int(row["value"]),
                    row["value_classification"],
                    row["source"],
                )
            )
    out.sort(key=lambda r: r.date_utc)
    return out


@dataclass(frozen=True)
class WindowSpec:
    window_id: str
    asset: str
    run_id: str
    from_utc: str
    to_utc: str
    benchmark_dir: Path
    chains: str


def read_windows_csv(path: Path) -> list[WindowSpec]:
    root = repo_root()
    specs: list[WindowSpec] = []
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            bdir = row["benchmark_dir"].strip()
            specs.append(
                WindowSpec(
                    window_id=row["window_id"].strip(),
                    asset=row["asset"].strip(),
                    run_id=row["run_id"].strip(),
                    from_utc=row["from_utc"].strip(),
                    to_utc=row["to_utc"].strip(),
                    benchmark_dir=(root / bdir).resolve(),
                    chains=row.get("chains", "").strip(),
                )
            )
    return specs


def fng_rows_in_window(
    rows: list[FearGreedRow], from_utc: str, to_utc: str
) -> list[FearGreedRow]:
    """Include calendar dates where from_date <= date < to_date (to_utc exclusive)."""
    start = iso_to_date_utc(from_utc)
    end = iso_to_date_utc(to_utc)
    return [r for r in rows if start <= r.date_utc < end]


def summarize_window_fng(window: WindowSpec, rows: list[FearGreedRow]) -> dict[str, Any]:
    in_win = fng_rows_in_window(rows, window.from_utc, window.to_utc)
    if not in_win:
        raise ValueError(
            f"no Fear & Greed rows for {window.window_id} "
            f"({window.from_utc} .. {window.to_utc} exclusive)"
        )
    values = [r.value for r in in_win]
    mean_v = sum(values) / len(values)
    dom = dominant_classification((r.value_classification for r in in_win), mean_v)
    return {
        "window_id": window.window_id,
        "from_utc": window.from_utc,
        "to_utc": window.to_utc,
        "fng_day_count": len(in_win),
        "mean_fng": round(mean_v, 2),
        "min_fng": min(values),
        "max_fng": max(values),
        "dominant_regime": dom,
        "analysis_regime": analysis_regime(dom),
    }


def supply_invariant_status(row: dict[str, str]) -> str:
    v = row.get("supply_invariant_pass", "").strip().lower()
    if v in ("true", "1", "yes"):
        return "PASS"
    if v in ("false", "0", "no"):
        return "FAIL"
    return "UNKNOWN"


def gross_to_net_ratio(sum_mints_raw: str, sum_burns_raw: str, onchain_delta_raw: str) -> str:
    if not sum_mints_raw.strip() or not sum_burns_raw.strip():
        return ""
    mint = int(sum_mints_raw)
    burn = int(sum_burns_raw)
    delta = int(onchain_delta_raw)
    if delta == 0:
        return ""
    return f"{(mint + burn) / abs(delta):.4f}"


def write_csv_dicts(path: Path, fieldnames: list[str], rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)


def write_meta_json(path: Path, meta: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
