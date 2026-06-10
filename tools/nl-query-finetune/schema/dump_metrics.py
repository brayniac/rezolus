#!/usr/bin/env python3
"""Phase 0: build the metric catalog (metric cards) from rezolus parquet files.

Usage:
  python schema/dump_metrics.py [PARQUET ...] [--out data/metrics.json]

With no PARQUET args, defaults to every parquet under site/viewer/data/ in the repo.
Writes a JSON list of metric cards (name, type, unit, description, labels+values,
source parquets) that the data generator consumes.
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from rezolus_oracle import Oracle  # noqa: E402

REPO = Path(__file__).resolve().parents[3]


def default_parquets():
    d = REPO / "site/viewer/data"
    return sorted(str(p) for p in d.glob("*.parquet")) if d.exists() else []


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("parquets", nargs="*", help="parquet files (default: site/viewer/data/*.parquet)")
    ap.add_argument("--out", default=str(Path(__file__).resolve().parents[1] / "data/metrics.json"))
    ap.add_argument("--binary", default=None)
    args = ap.parse_args()

    parquets = args.parquets or default_parquets()
    if not parquets:
        ap.error("no parquet files given and none found under site/viewer/data/")

    oracle = Oracle(args.binary)
    cards = oracle.dump_schema(parquets)

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    payload = [c.to_json() for c in sorted(cards.values(), key=lambda c: c.name)]
    out.write_text(json.dumps(payload, indent=2))

    by_type = {}
    labelled = 0
    for c in cards.values():
        by_type[c.type] = by_type.get(c.type, 0) + 1
        if c.labels:
            labelled += 1
    print(f"parquets : {len(parquets)}")
    print(f"metrics  : {len(cards)}  {by_type}")
    print(f"labelled : {labelled} metrics carry >=1 label")
    print(f"wrote    : {out}")


if __name__ == "__main__":
    main()
