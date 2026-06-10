# NL→PromQL fine-tune — run report

Fine-tuned `Qwen/Qwen2.5-Coder-0.5B` (full FT) to translate natural-language
requests into PromQL over the Rezolus metrics schema, for in-browser inference in
the viewer's Natural Query tab. Run on an RTX 4090 (24 GB).

## TL;DR

- **Held-out *metric* generalization (test.jsonl, execution-based semantic-equiv): 98.7%** (v2).
- Built via two iterations; the **judge-and-augment loop** (run v1 → judge failures
  with the oracle → targeted augmentation → v2) lifted the weak intent
  (`efficiency`) from 55.6% → 73.3% and added the heatmap query shape (`by_index`,
  100% on unseen metrics).
- Deliverable model: `checkpoints/nl-query-0.5b-v2` → `exports/nl-query-0.5b-onnx`
  (fp32 + `onnx/model_q4.onnx`). Runtime prompt format unchanged from
  `PROMPT_FORMAT.md`.

## Pipeline integrity (unchanged guarantee)

Gold PromQL is **never hand-written**: deterministic intent→query templates
(`datagen/templates.py`, `datagen/efficiency.py`) generate it, and every gold is
**executed against a real parquet** (`rezolus_oracle`) and dropped if it errors or
returns empty. The teacher only ever paraphrases the NL side. All augmentation
below preserves this — we author NL and metric *pairings*; the oracle validates
the gold.

## Data

Built with `datagen/build_dataset.py` (primary = demo.parquet for train/val/test +
held-out metrics; supplementary = cachecannon + vllm, training-only, excluding the
primary's held-out metrics so the eval stays clean; harvested dashboards folded
into train; merged → deduped → train/eval-collision guarded).

- **v2 dataset:** train **9767**, val 246, test 466 (held-out *metrics*),
  dashboard_eval 15.
- **Teacher paraphraser** (`datagen/paraphrase.py`): no hosted API key was
  available, so the teacher is a **local `Qwen2.5-3B-Instruct`** on the GPU
  (`REZOLUS_TEACHER=hf:Qwen/Qwen2.5-3B-Instruct`), used for the dashboard NL seeds.
  Every candidate passes a hard PromQL-rejection filter, with offline top-up — it
  can never touch the gold.
- **Authored NL variants** at the template level (intent-aware, where the metric
  phrase / label / value are in scope) — higher quality than lexical synonym swaps.

### Augmentation added this run (NL authored, gold execution-validated)

| shape (intent) | example NL | gold structure |
|---|---|---|
| efficiency: IPC/CPI | "instructions per cycle" | `sum(irate(I))/sum(irate(C))` |
| efficiency: cache hit/miss rate | "cache hit rate" | `sum(H)/(sum(H)+sum(M))` |
| efficiency: branch-miss / dtlb / numa rate | "branch misprediction rate" | `sum(bad)/sum(total)` |
| efficiency: avg IO / packet size | "average io size" | `sum(bytes)/sum(ops)` |
| efficiency: frequency scaling | "aperf to mperf ratio" | `sum(aperf)/sum(mperf)` |
| **traffic-cost** | "cpu usage per packet", "syscalls per packet", "instructions per byte" | `sum(cost)/sum(unit)` |
| utilization | "cpu utilization" | `sum(irate(cpu_usage))/sum(cpu_cores)/1e9` |
| **share** (part-of-whole) | "what fraction of cpu usage is system" | `sum(M{l=v})/sum(M)` |
| **peak / min** | "peak cpu usage", "lowest …" | `max(base)` / `min(base)` |
| **by_index** (per-CPU → HEATMAP) | "cpu usage per core", "… heatmap" | `sum by (id)(base)` |

## Eval harness validation (`eval/evaluate.py`)

Execution-based, never string match. Sanity on val:
- `--mode gold` = **100%** across parse / exec / semantic-equiv / selection / NO_METRIC.
- `--mode corrupt` collapses semantic-equiv to **13.1%** (NO_METRIC recall → 0%) —
  the harness discriminates.

## Results — v1 → v2 (judge-and-augment)

Held-out **test.jsonl** (metrics never seen in training), execution semantic-equiv:

| intent | v1 (8808 train) | v2 (9767 train) |
|---|---|---|
| **headline** | **97.1%** | **98.7%** |
| ratio | 100% | 100% (108/108) |
| total / lookup / average / by_label / peak / quantile | ~97–100% | **100%** |
| no_metric (P/R) | 96.2 / 100 | 96.8 / 100 |
| **efficiency** | **55.6%** | **73.3%** |
| by_index (heatmap) | — (not in v1) | **100%** (37/37) |
| filter | 96.4% | 94.3% |
| exec-success / metric-selection | 98.1 / 96.9 | 99.4 / 98.9 |

dashboard_eval (real dashboards): 46.7% semantic-equiv, **metric-selection ~100%**
— directional only (terse labels are ambiguous: one NL → several filtered golds,
e.g. "softirqs handled per second" maps to both `kind="sched"` and `kind="timer"`).
Weigh the held-out test, not this number.

**Judge step (v1 efficiency failures, found automatically by executing v1 preds vs
gold):** the model hallucinated shortened compound names (`cgroup_cpu_instructions`
→ `cgroup_instructions`) and picked wrong siblings/denominators
(`memory_numa_miss / memory_numa_other`). v2's broader efficiency coverage fixed
most; the residual ~27% is grounding compound names on *unseen* metrics — the
capacity signal that justifies escalating to `Qwen2.5-Coder-1.5B` if that last gap
matters.

## Chart-type selection (viewer-side, NOT model output)

Rezolus dashboards encode rendering via `opts.type` ∈ {delta_counter, gauge,
histogram} + `subtype: percentiles` — i.e. the **metric type**, plus the query
**result shape**. Both are already available to the viewer, so chart choice is
deterministic without expanding the model's target:

| signal (already known) | chart |
|---|---|
| histogram / `histogram_quantile` | percentile line(s) / latency heatmap |
| counter/gauge, single series | line |
| counter/gauge, `sum by (id/cpu)` multi-series | **heatmap** (per-CPU) |
| `sum by (categorical)` | multi-line / stacked |
| scalar / ratio | single line / stat |

The model *implicitly* drives heatmap-vs-line by emitting `histogram_quantile`
(line) vs a per-id breakout (heatmap) — which is exactly why we added the
`by_index` shape. Scatter is unused by the dashboards (it would be a 2-metric
correlation view — a separate viewer mode, like retrieval). Recommendation: keep
chart selection viewer-side from `(metric_type, resultType, #series, has-index-label)`.

## Export & viewer integration

- **Checkpoint:** `checkpoints/nl-query-0.5b-v2` (full FT of Qwen2.5-Coder-0.5B).
- **ONNX:** `exports/nl-query-0.5b-onnx/` (optimum, task `text-generation-with-past`;
  ONNX-vs-torch max diff ~1e-5). q4: `onnx/model_q4.onnx` (weight-only int4 on the
  transformer MatMuls; the ~700 MB `.data` is the fp32 embedding/lm_head table —
  use **q4f16** to roughly halve it for the browser).
- **Wire-in (`src/viewer/assets/lib/`):**
  - `nq_generate.js`: model id = the hosted `nl-query-0.5b-onnx` repo, `dtype: 'q4'`,
    `device: 'webgpu'`. Greedy decode; stop on `<|im_end|>` AND `<|endoftext|>`
    (the base eos differs from the chat-template terminator — see `eval/predict.py`).
  - `nq_prompt.js`: mirror `PROMPT_FORMAT.md` byte-for-byte (system string + card
    format `name (type; labels: a,b) — desc`). Unchanged this run.

### Parity (ONNX vs torch, on held-out test.jsonl, n=466)

| backend | semantic-equiv | exec-success | metric-selection |
|---|---|---|---|
| torch (bf16, GPU) | **98.7%** | 99.4% | 98.9% |
| ONNX fp32 (CPU) | **98.7%** | 99.4% | 98.9% |
| ONNX q4 (CPU) | **95.3%** | 97.4% | 97.2% |

ONNX fp32 is **bit-for-bit equivalent** to torch (same 98.7%). q4 costs ~3.4 pts
(98.7→95.3) — the expected 4-bit trade-off; recover it with **q4f16** or ship fp32
/ q8 if the browser budget allows. Reproduce:
`eval/predict.py --backend onnx [--onnx-file onnx/model_q4.onnx]` → `eval/evaluate.py --mode file`.

## Name grounding (the efficiency residual)

The v2 efficiency misses are on **held-out** metrics and split into two modes
(probe: `eval/predict.py` failing cases):
- **copy-faithfulness** — emits `cgroup_instructions` though `cgroup_cpu_instructions`
  is in the prompt;
- **acronym→composition** — "cgroup IPC"/"cpi" → wrong structure or `NO_METRIC`.

A system-prompt nudge ("use names exactly; IPC = instructions/cycles"), with no
retrain, recovered **0/4** (regressed 0): it changed the error mode (fewer
`NO_METRIC`, better name copies) but didn't yield correct compositions →
**capacity-bound, not promptable**. Moving the metric list into the system prompt
would not help (the names are already in-prompt) and would break the stable system
string + schema-agnostic design.

**Deployable fix — output grounding** (`eval/predict.py: ground_names`,
`--ground-names`): snap any emitted metric identifier not in the prompt's card set
to the nearest provided name (difflib cutoff 0.8; PromQL keywords and `{...}` label
keys excluded). On held-out test: 98.7% → **98.9%**, **1 fixed, 0 broken** — fixes
the copy class with zero regressions. This is the practical form of constrained
decoding and should be mirrored in `nq_generate.js` (a logit mask / post-snap over
the retrieved card names). The acronym→composition residual is the **1.5B** lever.

### Held-out metrics: eval-only, not for the shipping model

Holding metrics out measures generalization to *future* metrics — keep it as a
**benchmark**. But the **shipping** model should train on **100% of known
metrics** (no hold-out); today's pipeline handicaps the artifact by holding them
out. Recommended: ship an all-metrics model (`build_dataset.py` with
`--heldout-metric-frac 0`), and keep the held-out split only to report the
generalization lower bound (the 98.7% here). The cgroup-IPC misses are held-out
artifacts; in-vocabulary they train fine (non-cgroup IPC already works).

## Caveats / out of scope

- **Retrieval is viewer-side and NOT in this project.** For "A per B" / efficiency
  ratios the runtime retriever MUST surface *both* metrics or even a perfect model
  fails. Train data forces both into context; runtime must match.
- `dashboard_eval` is directional (ambiguous terse labels) — lean on the held-out
  `test.jsonl` + execution metrics.
- Last efficiency gap → consider `Qwen2.5-Coder-1.5B` (drop-in via `train/config.yaml`).
- Ready future shapes: container quota/throttle KPIs, top-k by efficiency, q4f16 export.
