---
license: apache-2.0
base_model: Qwen/Qwen2.5-Coder-0.5B
library_name: transformers.js
pipeline_tag: text-generation
tags:
  - promql
  - onnx
  - webgpu
  - text-generation
---

# promql-0.5b-onnx

A specialist fine-tune of **Qwen2.5-Coder-0.5B** that turns natural-language
questions into **PromQL** over a systems-performance metrics schema (CPU,
scheduler, block IO, network, syscalls, …), for **in-browser** inference
(transformers.js + WebGPU, ONNX/q4) — a natural-language query box for metrics.

It is **schema-agnostic**: the prompt supplies candidate metric "cards", and the
model answers using only those names — so it generalizes to metrics it never saw
in training. Gold PromQL in the training data is **execution-validated, never
hand-written**.

## Usage (transformers.js)

```js
import { pipeline } from '@huggingface/transformers';
const gen = await pipeline('text-generation', 'brayniac/promql-0.5b-onnx',
                           { device: 'webgpu', dtype: 'q4' });
```

Decode greedily and stop on **both** `<|im_end|>` and `<|endoftext|>` (the base
eos differs from the ChatML turn terminator). Take the first non-empty line of the
completion. Ground the output to the provided card names (snap any out-of-vocab
metric identifier to the nearest provided name).

## Prompt format (must match exactly)

**system**
```
Convert the request into ONE PromQL query using ONLY the listed metrics.
counter -> irate(x[1s]); gauge -> x; histogram -> histogram_quantile(q, x); aggregate with sum()/avg(); filter with {label="value"}; ratio "A per B" -> sum(A_expr)/sum(B_expr).
If no listed metric answers the request, output exactly: NO_METRIC. Output only PromQL, nothing else.
```

**user**
```
Metrics:
  <name> (<type>; labels: <k1>,<k2>) — <description>
  ...                                       # includes 1–3 distractors
Request: <natural language request>
```

**assistant** = one PromQL query, or exactly `NO_METRIC`.

## Evaluation (execution-based semantic equivalence)

| split | semantic-equiv |
|---|---|
| in-vocab held-out phrasings (n=293) | **100.0%** |
| held-out *metrics* never seen in training (n=466) | **98.7%** |
| ONNX q4 vs torch (in-vocab) | 98.6% (torch/fp32 = 100%) |

Intents covered: lookup, total, average, by_label, filter, share (part-of-whole),
peak/min, by_index (per-CPU → heatmap), quantile, topk, ratio, derived-efficiency
KPIs (IPC/CPI, cache hit/miss rate, branch-miss rate, avg IO/packet size, frequency
scaling, cost-per-traffic), and `NO_METRIC` refusal.

## Two things the runtime must uphold

1. **Retrieval supplies the cards** — for "A per B" / efficiency ratios the
   retriever MUST include *both* metrics, or even a perfect model can't answer.
2. **Chart type is chosen by the viewer**, not the model — from `metric_type` +
   result shape (histogram→percentiles/heatmap; `sum by (id)` multi-series→heatmap;
   single series→line). The model emits PromQL only.

Files: `onnx/model_q4.onnx` (int4, shipped) + fp32 `model.onnx`. A q4f16 (~half the
size) is recommended via the transformers.js conversion script.
