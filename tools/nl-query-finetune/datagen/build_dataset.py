#!/usr/bin/env python3
"""Build the full SFT dataset from a primary parquet + supplementary parquets.

Why an orchestrator: broad metric coverage comes from generating over several
single-source parquets, but the eval splits must stay clean. This script:

  1. PRIMARY parquet (e.g. demo): run generate.py → data/{train,val,test}.jsonl
     and record its held-out metric set (data/heldout.json). val/test are the
     ONLY in-distribution eval sets.
  2. SUPPLEMENTARY parquets (e.g. cachecannon, vllm): run generate.py with
     --exclude-metrics data/heldout.json so the primary's held-out metrics never
     leak into training, then fold ALL their splits into train (pure augmentation
     — we never evaluate on supplementary metrics).
  3. Harvested dashboard queries (data/harvested.jsonl, from harvest.py) are
     folded into train as well.
  4. Merge → dedupe identical (prompt, gold) → drop any train row that collides
     with val / test / dashboard_eval (train/eval leakage guard) → write train.

Schema dumps are produced per parquet on the fly. Re-run harvest.py separately
(it needs the dashboards + optionally REZOLUS_TEACHER); this script reuses its
output if present.

  python datagen/build_dataset.py --parquet-dir ../../site/viewer/data \
      --primary demo.parquet --supplementary cachecannon.parquet vllm.parquet \
      --out data --paraphrases 6 --ratios 60
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent  # tools/nl-query-finetune


def sh(cmd):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.check_call(cmd)


def dump_schema(parquet: str, out: str):
    sh([sys.executable, str(HERE.parent / "schema/dump_metrics.py"), parquet, "--out", out])


def gen(schema, parquet, out, paraphrases, ratios, extra=None):
    cmd = [sys.executable, str(HERE / "generate.py"), "--schema", schema,
           "--parquet", parquet, "--out", out,
           "--paraphrases", str(paraphrases), "--ratios", str(ratios)]
    if extra:
        cmd += extra
    sh(cmd)


def load(p):
    p = Path(p)
    return [json.loads(l) for l in p.open()] if p.exists() else []


def key(rec):
    """Identity for dedupe/leakage: the user request + the gold completion."""
    user = next((m["content"] for m in rec["messages"] if m["role"] == "user"), "")
    return (user, rec["gold"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--parquet-dir", default="../../site/viewer/data")
    ap.add_argument("--primary", default="demo.parquet")
    ap.add_argument("--supplementary", nargs="*", default=["cachecannon.parquet", "vllm.parquet"])
    ap.add_argument("--out", default="data")
    ap.add_argument("--paraphrases", type=int, default=6)
    ap.add_argument("--ratios", type=int, default=60)
    ap.add_argument("--primary-heldout-frac", type=float, default=0.15,
                    help="fraction of primary metrics held out to test (0 = ship: train on all, "
                         "eval split becomes example-level)")
    args = ap.parse_args()

    pdir = Path(args.parquet_dir)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    heldout = str(out / "heldout.json")

    # 1) primary
    prim_schema = str(out / "metrics.json")
    dump_schema(str(pdir / args.primary), prim_schema)
    gen(prim_schema, str(pdir / args.primary), str(out), args.paraphrases, args.ratios,
        extra=["--heldout-out", heldout,
               "--heldout-metric-frac", str(args.primary_heldout_frac)])

    train = load(out / "train.jsonl")
    val = load(out / "val.jsonl")
    test = load(out / "test.jsonl")
    dash = load(out / "dashboard_eval.jsonl")
    harvested = load(out / "harvested.jsonl")
    train += harvested
    print(f"[build] primary: train={len(train)-len(harvested)} (+{len(harvested)} harvested) "
          f"val={len(val)} test={len(test)} dashboard_eval={len(dash)}")

    # 2) supplementary (training-only, excluding the primary's held-out metrics)
    for i, sup in enumerate(args.supplementary):
        sschema = str(out / f"metrics_sup{i}.json")
        sdir = str(out / f"_sup{i}")
        dump_schema(str(pdir / sup), sschema)
        gen(sschema, str(pdir / sup), sdir, args.paraphrases, max(30, args.ratios // 2),
            extra=["--exclude-metrics", heldout, "--heldout-metric-frac", "0.0", "--seed", str(100 + i)])
        added = load(f"{sdir}/train.jsonl") + load(f"{sdir}/val.jsonl") + load(f"{sdir}/test.jsonl")
        train += added
        print(f"[build] supplementary {sup}: +{len(added)} train rows")

    # 3) dedupe + leakage guard
    eval_keys = {key(r) for r in (val + test + dash)}
    seen, merged, dropped_dup, dropped_leak = set(), [], 0, 0
    for r in train:
        k = key(r)
        if k in eval_keys:
            dropped_leak += 1
            continue
        if k in seen:
            dropped_dup += 1
            continue
        seen.add(k)
        merged.append(r)

    with (out / "train.jsonl").open("w") as f:
        for r in merged:
            f.write(json.dumps(r) + "\n")

    from collections import Counter
    print(f"\n[build] FINAL: train={len(merged)} (deduped {dropped_dup}, "
          f"dropped {dropped_leak} eval-colliding) | val={len(val)} test={len(test)} "
          f"dashboard_eval={len(dash)}")
    print(f"[build] train intent mix: {dict(Counter(r['intent'] for r in merged))}")


if __name__ == "__main__":
    main()
