"""Unit tests for Fear & Greed join helpers (stdlib unittest)."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

from _market_sentiment_common import (  # noqa: E402
    analysis_regime,
    fng_rows_in_window,
    gross_to_net_ratio,
    parse_fng_api_payload,
    summarize_window_fng,
    WindowSpec,
)


class TestFearGreedParse(unittest.TestCase):
    def test_fixture(self) -> None:
        payload = json.loads((ROOT / "tests/fixtures/fear_greed_api_sample.json").read_text())
        rows = parse_fng_api_payload(payload, source="test")
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0].date_utc, date(2024, 5, 1))
        self.assertEqual(rows[0].value, 25)
        self.assertEqual(rows[1].date_utc, date(2024, 5, 2))


class TestWindowJoin(unittest.TestCase):
    def test_exclusive_end(self) -> None:
        rows = parse_fng_api_payload(
            json.loads((ROOT / "tests/fixtures/fear_greed_api_sample.json").read_text())
        )
        win = [
            r
            for r in fng_rows_in_window(
                rows, "2024-05-01T00:00:00Z", "2024-05-02T00:00:00Z"
            )
        ]
        self.assertEqual(len(win), 1)
        self.assertEqual(win[0].value, 25)  # 2024-05-01 only

    def test_summarize(self) -> None:
        rows = parse_fng_api_payload(
            json.loads((ROOT / "tests/fixtures/fear_greed_api_sample.json").read_text())
        )
        w = WindowSpec(
            "test_win",
            "USDC",
            "test_win",
            "2024-05-01T00:00:00Z",
            "2024-05-03T00:00:00Z",
            ROOT,
            "",
        )
        s = summarize_window_fng(w, rows)
        self.assertEqual(s["fng_day_count"], 2)
        self.assertEqual(s["analysis_regime"], analysis_regime(s["dominant_regime"]))


class TestMetrics(unittest.TestCase):
    def test_gross_to_net(self) -> None:
        r = gross_to_net_ratio("1000", "900", "-100")
        self.assertEqual(r, "19.0000")

    def test_gross_to_net_zero_delta(self) -> None:
        self.assertEqual(gross_to_net_ratio("1", "1", "0"), "")


if __name__ == "__main__":
    unittest.main()
