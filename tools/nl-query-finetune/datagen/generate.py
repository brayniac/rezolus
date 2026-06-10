#!/usr/bin/env python3
"""Phase 1: generate the SFT dataset.

For each metric (and sampled metric pairs) we expand the deterministic intent
templates, EXECUTE every gold query to validate it (dropping any that error or
return empty), paraphrase the NL side, and assemble chat records in the canonical
runtime format with retrieval context (gold metric cards + distractors) plus
NO_METRIC negatives. Records are split into train / val / test, holding out a
fraction of *metrics* entirely to measure generalization to unseen metrics.

Usage:
  python datagen/generate.py --schema data/metrics_demo.json \
      --parquet ../../site/viewer/data/demo.parquet --out data \
      --paraphrases 4 --ratios 30 [--max-metrics N]

The canonical prompt format produced here is documented in PROMPT_FORMAT.md and
must be mirrored by the viewer's nq_prompt.js at runtime.
"""
from __future__ import annotations

import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from rezolus_oracle import Oracle  # noqa: E402
from datagen.templates import examples_for_metric, ratio_example  # noqa: E402
from datagen.efficiency import efficiency_examples  # noqa: E402
from datagen.paraphrase import paraphrase  # noqa: E402

SYSTEM = (
    "Convert the request into ONE PromQL query using ONLY the listed metrics.\n"
    "counter -> irate(x[1s]); gauge -> x; histogram -> histogram_quantile(q, x); "
    "aggregate with sum()/avg(); filter with {label=\"value\"}; "
    'ratio "A per B" -> sum(A_expr)/sum(B_expr).\n'
    "If no listed metric answers the request, output exactly: NO_METRIC. "
    "Output only PromQL, nothing else."
)

NO_METRIC = "NO_METRIC"


def format_card(card: dict) -> str:
    labels = ",".join(card.get("labels", {}).keys())
    head = f"{card['name']} ({card['type']}"
    head += f"; labels: {labels})" if labels else ")"
    desc = card.get("description", "")
    return f"  {head} — {desc}" if desc else f"  {head}"


def build_user(request: str, context_cards: list) -> str:
    lines = ["Metrics:"] + [format_card(c) for c in context_cards]
    lines.append(f"Request: {request}")
    return "\n".join(lines)


def pick_distractors(gold_names, cards_by_name, rng, k):
    """Prefer name-prefix-similar distractors (hard negatives), then random."""
    pool = [n for n in cards_by_name if n not in gold_names]
    prefixes = {n.split("_")[0] for n in gold_names}
    similar = [n for n in pool if n.split("_")[0] in prefixes]
    rng.shuffle(similar)
    rng.shuffle(pool)
    chosen, seen = [], set()
    for n in similar + pool:
        if n in seen:
            continue
        seen.add(n)
        chosen.append(n)
        if len(chosen) >= k:
            break
    return chosen


def make_record(request, gold, gold_metrics, context_names, cards_by_name, rng, intent):
    ctx = list(context_names)
    rng.shuffle(ctx)
    user = build_user(request, [cards_by_name[n] for n in ctx])
    return {
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": user},
            {"role": "assistant", "content": gold},
        ],
        "gold": gold,
        "intent": intent,
        "metrics": list(gold_metrics),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--schema", required=True)
    ap.add_argument("--parquet", required=True, help="parquet to validate gold against (must match schema)")
    ap.add_argument("--out", default="data")
    ap.add_argument("--paraphrases", type=int, default=4)
    ap.add_argument("--ratios", type=int, default=30, help="number of validated ratio pairs to sample")
    ap.add_argument("--distractors", type=int, default=2)
    ap.add_argument("--no-metric-rate", type=float, default=0.12, help="fraction of records turned into NO_METRIC negatives")
    ap.add_argument("--heldout-metric-frac", type=float, default=0.15)
    ap.add_argument("--max-metrics", type=int, default=0, help="cap metrics processed (0 = all); for quick local runs")
    ap.add_argument("--exclude-metrics", default=None,
                    help="JSON list of metric names to drop entirely (e.g. the primary run's "
                         "held-out set, so supplementary parquets never train on them)")
    ap.add_argument("--heldout-out", default=None,
                    help="write the chosen held-out metric names to this JSON file")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--binary", default=None)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    oracle = Oracle(args.binary)
    cards = json.load(open(args.schema))
    cards_by_name = {c["name"]: c for c in cards}
    names = sorted(cards_by_name)
    if args.exclude_metrics:
        drop = set(json.load(open(args.exclude_metrics)))
        names = [n for n in names if n not in drop]
        print(f"excluded          : {len(drop)} metric names (kept {len(names)})")
    if args.max_metrics:
        names = names[: args.max_metrics]
    # Restrict the working card set to the kept names so excluded metrics never
    # appear anywhere — not as a gold, a composition operand, OR a distractor.
    cards_by_name = {n: cards_by_name[n] for n in names}

    # Held-out metrics → test only (generalization to unseen metrics).
    heldout = set(rng.sample(names, max(1, int(len(names) * args.heldout_metric_frac))))
    if args.heldout_out:
        json.dump(sorted(heldout), open(args.heldout_out, "w"))

    valid_cache: dict = {}

    def is_valid(promql):
        if promql not in valid_cache:
            valid_cache[promql] = oracle.valid(args.parquet, promql)
        return valid_cache[promql]

    # 1) Single-metric examples.
    examples = []  # (Example, is_heldout_metric)
    n_dropped = 0
    for name in names:
        for ex in examples_for_metric(cards_by_name[name]):
            if is_valid(ex.promql):
                examples.append(ex)
            else:
                n_dropped += 1

    # 2) Ratio (composition) examples from sampled counter/gauge pairs.
    rateable = [n for n in names if cards_by_name[n]["type"] in ("counter", "gauge")]
    tried, made = 0, 0
    while made < args.ratios and tried < args.ratios * 12 and len(rateable) >= 2:
        tried += 1
        a, b = rng.sample(rateable, 2)
        ex = ratio_example(cards_by_name[a], cards_by_name[b])
        if is_valid(ex.promql):
            examples.append(ex)
            made += 1

    # 2b) Curated derived-efficiency KPIs (IPC, cache hit rate, branch-miss rate,
    #     average IO size, frequency scaling, …) — meaningful compositions the
    #     random pairs and the dashboards under-cover. Only those whose metrics
    #     are present here are emitted; each gold is still execution-validated.
    eff_made = 0
    for ex in efficiency_examples({n: cards_by_name[n] for n in names}):
        if is_valid(ex.promql):
            examples.append(ex)
            eff_made += 1

    # 3) Expand to records: paraphrase NL, attach context, add NO_METRIC negatives.
    records = []
    for ex in examples:
        gold_metrics = set(ex.metrics)
        held = bool(gold_metrics & heldout)
        # NL pool: authored intent-aware variants (from templates) first, then
        # top up with the paraphraser (offline augmenter or, if REZOLUS_TEACHER
        # is set, the teacher model). Use every authored phrasing.
        pool = list(ex.variants)
        for p in paraphrase(ex.nl, args.paraphrases, seed=args.seed):
            if p not in pool:
                pool.append(p)
        target = max(args.paraphrases, len(ex.variants))
        for i, nl in enumerate(pool[:target]):
            distract = pick_distractors(gold_metrics, cards_by_name, rng, args.distractors)
            # NO_METRIC negative: drop the gold metric(s) from context.
            if rng.random() < args.no_metric_rate:
                ctx = pick_distractors(gold_metrics, cards_by_name, rng, max(2, args.distractors + 1))
                rec = make_record(nl, NO_METRIC, set(), ctx, cards_by_name, rng, "no_metric")
            else:
                ctx = list(gold_metrics) + distract
                rec = make_record(nl, ex.promql, gold_metrics, ctx, cards_by_name, rng, ex.intent)
            rec["_held"] = held
            records.append(rec)

    # 4) Split.
    train, val, test = [], [], []
    for rec in records:
        held = rec.pop("_held")
        if held:
            test.append(rec)
        else:
            (val if rng.random() < 0.1 else train).append(rec)

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for split, recs in (("train", train), ("val", val), ("test", test)):
        with open(out / f"{split}.jsonl", "w") as f:
            for r in recs:
                f.write(json.dumps(r) + "\n")

    from collections import Counter
    intents = Counter(r["intent"] for r in records)
    print(f"metrics processed : {len(names)} ({len(heldout)} held out → test)")
    print(f"valid examples    : {len(examples)}  (dropped {n_dropped} invalid golds; {made} ratios; {eff_made} efficiency)")
    print(f"records           : {len(records)}  train={len(train)} val={len(val)} test={len(test)}")
    print(f"intent mix        : {dict(intents)}")
    print(f"wrote             : {out}/train.jsonl, val.jsonl, test.jsonl")


if __name__ == "__main__":
    main()
