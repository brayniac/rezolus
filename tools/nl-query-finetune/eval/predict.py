#!/usr/bin/env python3
"""Generate predictions from a trained checkpoint (torch or ONNX) for eval.

The eval harness (eval/evaluate.py --mode file) wants a JSONL of {"pred": "..."}
aligned 1:1 to a data file. This script produces that: it renders each record's
prompt (system+user, WITHOUT the gold assistant turn) with the model's chat
template + an assistant generation prompt — byte-for-byte the runtime format in
PROMPT_FORMAT.md — greedily decodes, and writes the first line of the completion.

Backends:
  --backend torch   AutoModelForCausalLM (the HF checkpoint)            [default]
  --backend onnx    optimum ORTModelForCausalLM (the exported ONNX dir)

Used for both trained-model eval and ONNX-vs-torch parity (run twice, diff).

Usage:
  python eval/predict.py --checkpoint checkpoints/nl-query-0.5b \
      --data data/test.jsonl --out preds_test.jsonl
  python eval/predict.py --backend onnx --checkpoint exports/nl-query-0.5b-onnx \
      --data data/test.jsonl --out preds_test_onnx.jsonl
"""
from __future__ import annotations

import argparse
import json
import re
import sys


def prompt_messages(rec):
    """Drop the trailing assistant (gold) turn; keep system+user as the prompt."""
    msgs = rec["messages"]
    if msgs and msgs[-1].get("role") == "assistant":
        return msgs[:-1]
    return msgs


# PromQL functions / keywords that are NOT metric names (never snapped).
_PROMQL_KW = {
    "sum", "avg", "min", "max", "count", "count_values", "stddev", "stdvar",
    "group", "topk", "bottomk", "quantile", "rate", "irate", "increase", "delta",
    "idelta", "deriv", "predict_linear", "histogram_quantile", "abs", "ceil",
    "floor", "round", "clamp", "clamp_min", "clamp_max", "exp", "ln", "log2",
    "log10", "sqrt", "sgn", "vector", "scalar", "time", "timestamp", "by",
    "without", "on", "ignoring", "group_left", "group_right", "offset", "and",
    "or", "unless", "le", "inf", "nan", "bool",
}
_IDENT = re.compile(r"[A-Za-z_]\w*")
_BRACES = re.compile(r"\{[^}]*\}")          # label filters — keys/values, not metrics


def parse_allowed(messages):
    """Metric names offered in the prompt's `Metrics:` block (the only legal names)."""
    user = next((m["content"] for m in messages if m["role"] == "user"), "")
    allowed = set()
    for line in user.splitlines():
        m = re.match(r"\s*([A-Za-z_]\w*)\s*\(", line)   # "  name (type; ...)"
        if m:
            allowed.add(m.group(1))
    return allowed


def ground_names(pred: str, allowed: set, cutoff: float = 0.8) -> str:
    """Snap any metric identifier not in `allowed` to the nearest provided name.

    This is the deployable form of constrained decoding: the model only sees a
    fixed set of card names, so any identifier outside it (and outside the PromQL
    keyword set, and not a label key/value) is a hallucination — replace it with
    the closest legal name. Fixes copy-faithfulness errors like
    cgroup_instructions -> cgroup_cpu_instructions. Conservative (cutoff 0.8) so
    it never snaps a genuinely-different name.
    """
    import difflib
    if not allowed or pred.strip() == "NO_METRIC":
        return pred
    masked = _BRACES.sub("{}", pred)            # ignore identifiers inside {label="v"}
    spans = [(m.group(0), m.start(), m.end()) for m in _IDENT.finditer(masked)]
    repl = {}
    for tok, _s, e in spans:
        if tok in _PROMQL_KW or tok in allowed or tok.isdigit():
            continue
        if e < len(masked) and masked[e] == "(":   # function call → not a metric
            continue
        near = difflib.get_close_matches(tok, allowed, n=1, cutoff=cutoff)
        if near:
            repl[tok] = near[0]
    out = pred
    for bad, good in repl.items():
        out = re.sub(rf"\b{re.escape(bad)}\b", good, out)
    return out


def first_line(text: str) -> str:
    # The model is trained to emit a single line (one PromQL query or NO_METRIC).
    # Keep only the first non-empty line and strip stray code fences/quotes.
    for line in text.splitlines():
        s = line.strip().strip("`").strip()
        if s:
            return s
    return text.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--backend", choices=["torch", "onnx"], default="torch")
    ap.add_argument("--onnx-file", default=None,
                    help="ONNX backend: load this file (e.g. onnx/model_q4.onnx) for q4 parity")
    ap.add_argument("--max-new-tokens", type=int, default=64)
    ap.add_argument("--batch-size", type=int, default=16)
    ap.add_argument("--ground-names", action="store_true",
                    help="snap out-of-vocab metric identifiers to the nearest provided card name")
    args = ap.parse_args()

    import torch
    from transformers import AutoTokenizer

    tok = AutoTokenizer.from_pretrained(args.checkpoint)
    if tok.pad_token is None:
        tok.pad_token = tok.eos_token
    tok.padding_side = "left"  # decoder-only batched generation needs left pad

    # The Qwen chat template ends the assistant turn with <|im_end|>, but the base
    # tokenizer's eos_token is <|endoftext|>. Stop on either so generation halts at
    # the end of the completion (correctness + speed) instead of running to the cap.
    eos_ids = {tok.eos_token_id}
    im_end = tok.convert_tokens_to_ids("<|im_end|>")
    if isinstance(im_end, int) and im_end >= 0:
        eos_ids.add(im_end)
    eos_ids = [i for i in eos_ids if i is not None]

    if args.backend == "onnx":
        from optimum.onnxruntime import ORTModelForCausalLM
        kw = {}
        if args.onnx_file:  # optimum wants (subfolder, basename), not a slashed path
            parts = args.onnx_file.rsplit("/", 1)
            kw["file_name"] = parts[-1]
            if len(parts) == 2:
                kw["subfolder"] = parts[0]
        model = ORTModelForCausalLM.from_pretrained(args.checkpoint, **kw)
        device = "cpu"
    else:
        from transformers import AutoModelForCausalLM
        device = "cuda" if torch.cuda.is_available() else "cpu"
        model = AutoModelForCausalLM.from_pretrained(
            args.checkpoint,
            torch_dtype=torch.bfloat16 if device == "cuda" else torch.float32,
        ).to(device)
        model.eval()  # ORTModelForCausalLM has no .eval(); only torch needs it

    records = [json.loads(l) for l in open(args.data)]
    prompts = [
        tok.apply_chat_template(prompt_messages(r), tokenize=False,
                                add_generation_prompt=True)
        for r in records
    ]

    preds = []
    for i in range(0, len(prompts), args.batch_size):
        batch = prompts[i:i + args.batch_size]
        enc = tok(batch, return_tensors="pt", padding=True).to(model.device)
        with torch.no_grad():
            out = model.generate(
                **enc,
                max_new_tokens=args.max_new_tokens,
                do_sample=False,                 # greedy → deterministic
                num_beams=1,
                pad_token_id=tok.pad_token_id,
                eos_token_id=eos_ids,
            )
        gen = out[:, enc["input_ids"].shape[1]:]
        for r, seq in zip(records[i:i + args.batch_size],
                          tok.batch_decode(gen, skip_special_tokens=True)):
            p = first_line(seq)
            if args.ground_names:
                p = ground_names(p, parse_allowed(r["messages"]))
            preds.append(p)
        print(f"  {min(i + args.batch_size, len(prompts))}/{len(prompts)}",
              file=sys.stderr)

    with open(args.out, "w") as f:
        for p in preds:
            f.write(json.dumps({"pred": p}) + "\n")
    print(json.dumps({"out": args.out, "n": len(preds), "backend": args.backend}))


if __name__ == "__main__":
    main()
