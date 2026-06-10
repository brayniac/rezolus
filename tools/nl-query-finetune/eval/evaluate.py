#!/usr/bin/env python3
"""Phase 3: execution-based evaluation.

PromQL has many equivalent spellings, so we never score by string match. Instead
we EXECUTE gold and predicted queries against a fixture parquet and compare
behaviour:

  * parse-validity     — does the prediction parse as PromQL?
  * exec-success       — does it run and return a non-empty result?
  * semantic-equiv     — same series (by labels) with matching point-count + stats
                         (this is the headline metric)
  * metric-selection   — does the prediction reference the gold metric(s)?
  * NO_METRIC P/R      — refusal precision/recall on out-of-vocab requests

Results are broken down per intent class to show exactly where to add data.

Predictions come from one of:
  --mode gold      pred = gold            (sanity: should score ~100%)
  --mode corrupt   pred = mutated gold    (sanity: equiv should collapse)
  --mode file --pred preds.jsonl          (JSONL with a "pred" field, aligned to input)

Usage:
  python eval/evaluate.py --data data/test.jsonl \
      --parquet ../../site/viewer/data/demo.parquet --mode gold
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from rezolus_oracle import Oracle  # noqa: E402

NO_METRIC = "NO_METRIC"


def _close(a, b, rel=1e-6, abs_=1e-6):
    if a is None or b is None:
        return a is None and b is None
    return abs(a - b) <= max(abs_, rel * max(abs(a), abs(b)))


def semantic_equiv(oracle, parquet, gold, pred):
    if gold.strip() == NO_METRIC:
        return pred.strip() == NO_METRIC
    if pred.strip() == NO_METRIC:
        return False
    rg = oracle.query(parquet, gold)
    rp = oracle.query(parquet, pred)
    if not rg.ok or not rp.ok or rg.empty or rp.empty:
        return False
    gmap = {frozenset(s.labels.items()): s for s in rg.series}
    pmap = {frozenset(s.labels.items()): s for s in rp.series}
    if set(gmap) != set(pmap):
        return False
    for k in gmap:
        a, b = gmap[k], pmap[k]
        if a.points != b.points:
            return False
        if not (_close(a.mean, b.mean) and _close(a.vmin, b.vmin) and _close(a.vmax, b.vmax)):
            return False
    return True


def parses(oracle, parquet, pred):
    if pred.strip() == NO_METRIC:
        return True
    r = oracle.query(parquet, pred)
    # Parsed iff it executed OR failed for a non-syntax reason.
    return r.ok or not r.error.startswith("Parse error")


def exec_ok(oracle, parquet, pred):
    if pred.strip() == NO_METRIC:
        return True
    return oracle.valid(parquet, pred)


def selection_ok(gold_metrics, pred):
    toks = set(re.findall(r"[a-zA-Z_]\w*", pred))
    if not gold_metrics:
        return pred.strip() == NO_METRIC
    return all(m in toks for m in gold_metrics)


def corrupt(gold):
    """Deliberately wrong prediction to confirm the harness detects failure."""
    if gold.strip() == NO_METRIC:
        return "irate(cpu_usage[1s])"          # claim a metric where none applies
    if "irate(" in gold:
        return gold.replace("irate(", "rate(").replace("[1s]", "[1h]")  # wrong window/fn
    return f"2 * ({gold})"                      # scale the value (always beyond tolerance)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--parquet", required=True)
    ap.add_argument("--mode", choices=["gold", "corrupt", "file"], default="gold")
    ap.add_argument("--pred", help="predictions JSONL (mode=file); needs a 'pred' field aligned to --data")
    ap.add_argument("--binary", default=None)
    args = ap.parse_args()

    oracle = Oracle(args.binary)
    records = [json.loads(l) for l in open(args.data)]
    if args.mode == "file":
        preds = [json.loads(l)["pred"] for l in open(args.pred)]
        if len(preds) != len(records):
            ap.error(f"pred count {len(preds)} != data count {len(records)}")
    else:
        preds = [corrupt(r["gold"]) if args.mode == "corrupt" else r["gold"] for r in records]

    agg = defaultdict(int)
    per_intent = defaultdict(lambda: [0, 0])  # intent -> [equiv, total]
    nm_tp = nm_fp = nm_fn = 0

    for rec, pred in zip(records, preds):
        gold = rec["gold"]
        intent = rec.get("intent", "?")
        gold_metrics = rec.get("metrics", [])
        agg["n"] += 1
        agg["parse"] += parses(oracle, args.parquet, pred)
        agg["exec"] += exec_ok(oracle, args.parquet, pred)
        eq = semantic_equiv(oracle, args.parquet, gold, pred)
        agg["equiv"] += eq
        agg["select"] += selection_ok(gold_metrics, pred)
        per_intent[intent][0] += eq
        per_intent[intent][1] += 1
        # NO_METRIC precision/recall
        g_nm, p_nm = gold.strip() == NO_METRIC, pred.strip() == NO_METRIC
        nm_tp += g_nm and p_nm
        nm_fp += (not g_nm) and p_nm
        nm_fn += g_nm and (not p_nm)

    n = agg["n"] or 1
    print(f"mode={args.mode}  n={agg['n']}")
    print(f"  parse-validity   : {agg['parse']/n:6.1%}")
    print(f"  exec-success     : {agg['exec']/n:6.1%}")
    print(f"  semantic-equiv   : {agg['equiv']/n:6.1%}   <- headline")
    print(f"  metric-selection : {agg['select']/n:6.1%}")
    prec = nm_tp / (nm_tp + nm_fp) if (nm_tp + nm_fp) else 1.0
    rec_ = nm_tp / (nm_tp + nm_fn) if (nm_tp + nm_fn) else 1.0
    print(f"  NO_METRIC P/R    : {prec:6.1%} / {rec_:6.1%}")
    print("  per-intent semantic-equiv:")
    for intent in sorted(per_intent):
        eq, tot = per_intent[intent]
        print(f"    {intent:10s}: {eq/tot:6.1%}  ({eq}/{tot})")


if __name__ == "__main__":
    main()
