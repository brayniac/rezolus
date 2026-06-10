# nl-query-finetune

Fine-tune a small (~0.5B) code LLM to translate natural-language questions into
**PromQL over the Rezolus metrics schema**, for fully **in-browser** inference in
the viewer's Natural Query tab (transformers.js, WebGPU, ONNX/q4).

## Why fine-tune

A general 0.5B in-browser model can't do this: e.g. *"cpu usage per tcp packet"*
→ `rate(tcp_bytes[1h])` (wrong metric, no composition, hallucinated window). But
the domain is **narrow** (a known metric vocabulary) and the output is a **small
formal language**, so a fine-tuned specialist is a strong fit — and the key
enabler is that **gold PromQL is generated, never hand-written**: the deterministic
intent→query templates (`datagen/templates.py`) are a correct-PromQL generator,
validated by executing every query against a real parquet. A teacher model only
paraphrases the NL side and never writes PromQL, so it can't poison labels.

## Decisions

- **Base model:** `Qwen2.5-Coder-0.5B` (full fine-tune). Fallback `…-1.5B` if eval
  shows capacity-bound failures on composition.
- **Full fine-tuning** (not LoRA): cheap at 0.5B, maximal specialization, clean
  ONNX export. Train on a CUDA host (RTX 4090 24GB) — see hardware notes below.
- **Schema-agnostic training:** every example injects retrieved metric cards into
  the prompt; the model learns to use *provided* names, so it generalizes to
  metrics added in future Rezolus versions. Includes distractors + `NO_METRIC`
  refusal negatives.
- **`irate(x[1s])`** for counters (instantaneous), not `rate`.
- **Execution-based eval**, never string match.

The exact training/inference prompt format is in **`PROMPT_FORMAT.md`** — the
single source of truth that `src/viewer/assets/lib/nq_prompt.js` must mirror.

## Layout

```
rezolus_oracle.py     wraps the rezolus binary: schema dump + PromQL execute/validate
schema/dump_metrics.py  Phase 0 — metric cards → data/metrics.json
datagen/templates.py    deterministic intent → gold PromQL (the label generator)
datagen/paraphrase.py   NL diversification (offline augmenter; pluggable teacher)
datagen/generate.py     Phase 1 — validate golds, build chat records, split
datagen/harvest.py      Phase 1b — harvest real dashboard queries as seed data
eval/evaluate.py        Phase 3 — execution-based eval (parse/exec/equiv/selection)
train/{config.yaml,sft.py}   Phase 2 — SFT (CUDA host)
export/to_onnx.py       Phase 4 — ONNX/q4 export for transformers.js (CUDA host)
```

`data/`, `checkpoints/`, `exports/` are gitignored — regenerate from the scripts.

## Prerequisites

Build the rezolus binary (provides the schema + a PromQL oracle):
```
cargo build --release          # from the repo root
export REZOLUS_BIN=$PWD/target/release/rezolus
```
**Phases 0–1 and eval are pure Python stdlib** — no installs. Training/export deps
(CUDA host) are in `requirements.txt`.

## Run it

```bash
cd tools/nl-query-finetune
PQ=../../site/viewer/data/demo.parquet     # any single-source parquet

# Phase 0 — metric cards (single parquet → label values are co-present for validation)
python3 schema/dump_metrics.py $PQ --out data/metrics.json

# Phase 1 — generate + validate the dataset (executes every gold)
python3 datagen/generate.py --schema data/metrics.json --parquet $PQ \
    --out data --paraphrases 4 --ratios 40
#   add --max-metrics N for a quick slice; set REZOLUS_TEACHER to use a real
#   paraphraser instead of the offline augmenter (see datagen/paraphrase.py).

# Phase 1b — harvest REAL dashboard queries as high-value seed + eval data.
# Each plot's title/description is an NL seed; the query is expert gold. Windows
# are normalized to irate(...[1s]); every query is execution-validated.
python3 datagen/harvest.py --schema data/metrics.json --parquet $PQ --dash-dir /tmp/dash --dump
cat data/harvested.jsonl >> data/train.jsonl          # fold into training
# data/dashboard_eval.jsonl = held-out "reproduce the real dashboards" benchmark
python3 eval/evaluate.py --data data/dashboard_eval.jsonl --parquet $PQ --mode gold

# Phase 3 — sanity-check the eval harness (gold should be ~100%, corrupt collapses)
python3 eval/evaluate.py --data data/val.jsonl --parquet $PQ --mode gold
python3 eval/evaluate.py --data data/val.jsonl --parquet $PQ --mode corrupt

# Phase 2 — train (CUDA host)
pip install -r requirements.txt
python3 train/sft.py --config train/config.yaml

# Evaluate the trained model: emit predictions as JSONL ({"pred": "..."} aligned
# to data/test.jsonl), then:
python3 eval/evaluate.py --data data/test.jsonl --parquet $PQ --mode file --pred preds.jsonl

# Phase 4 — export for the browser (CUDA host)
python3 export/to_onnx.py --checkpoint checkpoints/nl-query-0.5b --out exports/nl-query-0.5b-onnx
```

Larger coverage: run Phase 0/1 per additional single-source parquet and
concatenate the JSONL — each metric's label values stay co-present for validation.

## Status (validated locally on demo.parquet)

- Phase 0 oracle: 9/9 query/validation checks pass (incl. rejecting bad label
  values and parse errors); clean schema (57 metrics, pseudo-labels excluded).
- Phase 1: golds executed + validated; records carry context + distractors +
  `NO_METRIC` negatives; held-out-metric split for generalization.
- Phase 1b harvest: 237 dashboard plots → 78 distinct execution-validated golds
  (windows normalized to irate[1s]) → ~500 train records + 15 held-out
  dashboard-eval. Real idiomatic queries (IPC ratios, unit conversions, per-id
  breakouts) the synthetic templates don't produce.
- Phase 3: `--mode gold` = 100% across parse/exec/equiv/selection/NO_METRIC;
  `--mode corrupt` collapses semantic-equiv — the harness discriminates.
- Phases 2/4 are documented, runnable on the CUDA host (not exercised here).

## Seed data from the viewer's own queries

`datagen/harvest.py` mines the viewer's dashboards (and could be extended to the
service-KPI templates and Query Explorer examples) for expert PromQL. The plot
title/subgroup description is the NL seed; the query is gold. This anchors the
dataset in the team's real intent distribution and idioms. Caveat: terse/shared
dashboard labels are sometimes ambiguous (one description → several filtered
golds), so treat `dashboard_eval` as directional and lean on a real teacher
(`REZOLUS_TEACHER`) for clean NL — the offline augmenter only does lexical
variety.

## Two things to get right at integration time

1. **Retrieval is the other half.** The model only uses metrics in its prompt. The
   viewer's embedding search must be **intent-aware and multi-metric** — for
   "A per B" it must surface *both* metrics, or even a perfect model fails. Train
   data forces both into context; runtime must match.
2. **Prompt-format drift is fatal.** Keep `nq_prompt.js` byte-for-byte in sync with
   `PROMPT_FORMAT.md`.

## Hardware

Train on the **RTX 4090** (or any CUDA GPU/cloud): the HF + flash-attn + optimum
stack is CUDA-first and full-FT of 0.5B fits in ~8GB. An M3 Ultra works only via
MLX (+LoRA); its big unified memory is wasted on a 0.5B target. Use the Mac for
the data/eval pipeline (stdlib, binary-bound) regardless.
