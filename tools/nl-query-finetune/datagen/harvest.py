#!/usr/bin/env python3
"""Harvest expert PromQL from the viewer's dashboards as high-value seed data.

Every dashboard plot is a real, idiomatic query written by the people who designed
the metrics, and its subgroup description / plot title are free NL labels. We:

  1. dump the dashboard JSON (`cargo run -p dashboard`),
  2. walk groups → subgroups → plots, taking (nl seeds, promql),
  3. normalize counter windows to irate(...[1s])  (project convention),
  4. drop templated/placeholder queries,
  5. VALIDATE every normalized query by executing it (rezolus_oracle),
  6. paraphrase the NL seeds and emit chat records (reusing datagen.generate),
  7. hold out a slice as a "reproduce the real dashboards" eval set.

These records complement the synthetic templates: harvested = realism/idiom +
the team's true intent distribution; synthetic = breadth, filters, NO_METRIC,
composition.

Usage:
  python datagen/harvest.py --schema data/metrics.json \
      --parquet ../../site/viewer/data/demo.parquet --dash-dir /tmp/dash \
      --out data
  # add --dump to regenerate the dashboard JSON via cargo first.
"""
from __future__ import annotations

import argparse
import json
import random
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from rezolus_oracle import Oracle  # noqa: E402
from datagen.generate import SYSTEM, build_user, pick_distractors  # noqa: E402
from datagen.paraphrase import paraphrase  # noqa: E402

REPO = Path(__file__).resolve().parents[3]

# Queries we can't validate/teach as-is: regex matches, dashboard variables,
# service/cgroup placeholder substitutions.
_PLACEHOLDER = re.compile(r"=~|\$\{|\$[A-Za-z]|__[A-Z]|\{\{|%\{")

# rate(SEL[dur]) or irate(SEL[dur]) -> irate(SEL[1s]). SEL has no parens/brackets
# of its own (label filters use {}), so [^()\[\]]*? is safe and non-greedy.
_RATE = re.compile(r"\b(?:i?rate)\(([^()\[\]]*?)\[[^\]]*\]\)")


def normalize_window(q: str) -> str:
    return _RATE.sub(r"irate(\1[1s])", q)


def dump_dashboard(dash_dir: Path):
    dash_dir.mkdir(parents=True, exist_ok=True)
    subprocess.check_call(
        ["cargo", "run", "-q", "-p", "dashboard", "--", str(dash_dir)], cwd=str(REPO))


def clean_nl(s: str) -> str:
    s = s.replace("%", " percentage").replace("/", " per ")
    words = s.split()
    # Drop consecutive/repeated words so "timer" + "timer rate" -> "timer rate".
    deduped, seen = [], set()
    for w in words:
        lw = w.lower()
        if lw in seen:
            continue
        seen.add(lw)
        deduped.append(w)
    return " ".join(deduped).strip().lower()


def walk_plots(dash_dir: Path):
    """Yield dicts: {section, subsection, desc, title, query}."""
    for jf in sorted(dash_dir.glob("*.json")):
        d = json.loads(jf.read_text())
        if not isinstance(d, dict) or "groups" not in d:
            continue  # e.g. sections.json is an index, not a dashboard
        for group in d.get("groups", []):
            section = group.get("name", "")
            for sub in group.get("subgroups", []):
                subsection = sub.get("name", "")
                desc = sub.get("description", "")
                for plot in sub.get("plots", []):
                    q = plot.get("promql_query")
                    if not q:
                        continue
                    yield {
                        "section": section,
                        "subsection": subsection,
                        "desc": desc,
                        "title": plot.get("opts", {}).get("title", ""),
                        "query": q,
                    }


def extract_metrics(query: str, names: list) -> list:
    """Schema metric names that appear as whole identifiers (longest-first wins)."""
    toks = set(re.findall(r"[A-Za-z_]\w*", query))
    return sorted((n for n in names if n in toks), key=len, reverse=True)


def nl_seeds(plot: dict) -> list:
    seeds = []
    if plot["desc"]:
        seeds.append(clean_nl(plot["desc"]))
    combo = clean_nl(f"{plot['subsection']} {plot['title']}")
    if combo and combo not in seeds:
        seeds.append(combo)
    return seeds


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--schema", required=True)
    ap.add_argument("--parquet", required=True)
    ap.add_argument("--dash-dir", default="/tmp/dash")
    ap.add_argument("--dump", action="store_true", help="regenerate dashboard JSON via cargo")
    ap.add_argument("--out", default="data")
    ap.add_argument("--paraphrases", type=int, default=4)
    ap.add_argument("--distractors", type=int, default=2)
    ap.add_argument("--eval-frac", type=float, default=0.2, help="distinct queries held out as the dashboard eval set")
    ap.add_argument("--seed", type=int, default=11)
    ap.add_argument("--binary", default=None)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    oracle = Oracle(args.binary)
    cards = {c["name"]: c for c in json.load(open(args.schema))}
    names = sorted(cards)

    dash_dir = Path(args.dash_dir)
    if args.dump or not dash_dir.exists():
        dump_dashboard(dash_dir)

    # 1) collect distinct, normalized, validated queries with their NL seeds.
    seen = {}
    stats = {"plots": 0, "placeholder": 0, "invalid": 0, "no_metric": 0, "kept": 0}
    for plot in walk_plots(dash_dir):
        stats["plots"] += 1
        raw = plot["query"]
        if _PLACEHOLDER.search(raw):
            stats["placeholder"] += 1
            continue
        q = normalize_window(raw)
        metrics = extract_metrics(q, names)
        if not metrics:
            stats["no_metric"] += 1
            continue
        if q in seen:
            for s in nl_seeds(plot):           # ordered, description-first
                if s not in seen[q]["seeds"]:
                    seen[q]["seeds"].append(s)
            continue
        if not oracle.valid(args.parquet, q):
            stats["invalid"] += 1
            continue
        seen[q] = {"query": q, "metrics": metrics, "seeds": nl_seeds(plot),
                   "section": plot["section"]}
        stats["kept"] += 1

    queries = list(seen.values())
    rng.shuffle(queries)

    # 2) hold out distinct queries for the "reproduce real dashboards" eval.
    n_eval = max(1, int(len(queries) * args.eval_frac))
    eval_q, train_q = queries[:n_eval], queries[n_eval:]

    def to_record(nl, q, metrics):
        ctx = list(metrics) + pick_distractors(set(metrics), cards, rng, args.distractors)
        rng.shuffle(ctx)
        user = build_user(nl, [cards[n] for n in ctx])
        return {
            "messages": [
                {"role": "system", "content": SYSTEM},
                {"role": "user", "content": user},
                {"role": "assistant", "content": q},
            ],
            "gold": q, "intent": "dashboard", "metrics": list(metrics),
        }

    train_recs = []
    for item in train_q:
        seeds = item["seeds"] or [item["query"]]
        for seed in seeds:
            for nl in paraphrase(seed, args.paraphrases, seed=args.seed):
                train_recs.append(to_record(nl, item["query"], item["metrics"]))

    # Eval: one record per distinct query, using the canonical (description-first) seed.
    eval_recs = [to_record(it["seeds"][0] if it["seeds"] else it["query"],
                           it["query"], it["metrics"]) for it in eval_q]

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    with open(out / "harvested.jsonl", "w") as f:
        for r in train_recs:
            f.write(json.dumps(r) + "\n")
    with open(out / "dashboard_eval.jsonl", "w") as f:
        for r in eval_recs:
            f.write(json.dumps(r) + "\n")

    print(f"plots scanned   : {stats['plots']}")
    print(f"  dropped       : {stats['placeholder']} templated, {stats['no_metric']} no-schema-metric, {stats['invalid']} failed validation")
    print(f"distinct queries: {len(queries)}  (train {len(train_q)} / eval {len(eval_q)})")
    print(f"records         : {len(train_recs)} train  +  {len(eval_recs)} dashboard-eval")
    print(f"wrote           : {out}/harvested.jsonl, {out}/dashboard_eval.jsonl")
    print("merge into training:  cat data/harvested.jsonl >> data/train.jsonl")


if __name__ == "__main__":
    main()
