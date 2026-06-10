# Canonical prompt format (single source of truth)

The model is trained on the EXACT format below, and the viewer's
`src/viewer/assets/lib/nq_prompt.js` MUST send the same format at inference. If
these drift, accuracy collapses silently. Change them together.

The string constants live in `datagen/generate.py` (`SYSTEM`, `format_card`,
`build_user`). This document mirrors them for the runtime side.

## Messages (ChatML / Qwen chat template)

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
  <name> (<type>) — <description>          # no-label metric
  ...                                       # includes 1–3 distractors
Request: <natural language request>
```

**assistant** (the completion; loss is computed only here)
```
<PromQL>          # or exactly: NO_METRIC
```

## Worked example
```
system: Convert the request into ONE PromQL query using ONLY the listed metrics. …
user:
  Metrics:
    cpu_usage (counter; labels: id,state) — The amount of CPU time spent in each state
    tcp_packets (counter; labels: id,direction) — TCP packets
    cpu_frequency (gauge; labels: id) — core frequency        # distractor
  Request: cpu usage per tcp packet
assistant: sum(irate(cpu_usage[1s])) / sum(irate(tcp_packets[1s]))
```

## Invariants the runtime must uphold
1. **Same system string**, byte-for-byte.
2. **Same card format**: `name (type; labels: a,b) — description` (omit `; labels: …`
   when the metric has none; omit ` — description` when absent).
3. **Retrieval supplies the cards.** The model only uses names present in the
   prompt → it generalizes to metrics it never saw in training. For ratio/"per"
   requests the retriever MUST include *both* metrics, or even a perfect model
   can't answer.
4. **`NO_METRIC`** is the refusal token when no listed metric fits.
5. Node scoping is injected by the viewer *after* generation — never ask the
   model for `{node=…}`.
