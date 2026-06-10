#!/usr/bin/env python3
"""Phase 4: export the fine-tuned checkpoint to ONNX (q4) for transformers.js.

Runs on the CUDA host after training. Produces a model directory the in-browser
viewer can load via `@huggingface/transformers` (set device:'webgpu', dtype:'q4').

Recommended path uses the optimum CLI (most robust ONNX export for Qwen2):

  optimum-cli export onnx \
      --model checkpoints/nl-query-0.5b \
      --task text-generation-with-past \
      exports/nl-query-0.5b-onnx

Then quantize to q4 (onnxruntime or optimum quantization), and verify ONNX-vs-
torch parity by running eval/evaluate.py against both before shipping.

After export, point the viewer at the model:
  - host exports/nl-query-0.5b-onnx on HF (or self-host),
  - set the model id + `dtype: 'q4'` in src/viewer/assets/lib/nq_generate.js,
  - mirror PROMPT_FORMAT.md in src/viewer/assets/lib/nq_prompt.js.

This script wraps the CLI for convenience; review flags for your optimum version.
"""
from __future__ import annotations

import argparse
import subprocess
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", required=True, help="HF checkpoint dir from sft.py")
    ap.add_argument("--out", default="exports/nl-query-0.5b-onnx")
    ap.add_argument("--task", default="text-generation-with-past")
    args = ap.parse_args()

    cmd = [
        "optimum-cli", "export", "onnx",
        "--model", args.checkpoint,
        "--task", args.task,
        args.out,
    ]
    print("running:", " ".join(cmd))
    rc = subprocess.call(cmd)
    if rc != 0:
        sys.exit(rc)
    print(f"\nExported to {args.out}")
    print("Next: quantize to q4, verify parity with eval/evaluate.py, then wire into nq_generate.js.")


if __name__ == "__main__":
    main()
