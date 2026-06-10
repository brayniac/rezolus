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
import sys


def prompt_messages(rec):
    """Drop the trailing assistant (gold) turn; keep system+user as the prompt."""
    msgs = rec["messages"]
    if msgs and msgs[-1].get("role") == "assistant":
        return msgs[:-1]
    return msgs


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
        for seq in tok.batch_decode(gen, skip_special_tokens=True):
            preds.append(first_line(seq))
        print(f"  {min(i + args.batch_size, len(prompts))}/{len(prompts)}",
              file=sys.stderr)

    with open(args.out, "w") as f:
        for p in preds:
            f.write(json.dumps({"pred": p}) + "\n")
    print(json.dumps({"out": args.out, "n": len(preds), "backend": args.backend}))


if __name__ == "__main__":
    main()
