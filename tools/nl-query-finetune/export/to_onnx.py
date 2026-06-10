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
import shutil
import subprocess
import sys
from pathlib import Path


def quantize_q4(fp32_onnx: Path, out_onnx: Path, block_size: int = 32):
    """Block-wise 4-bit weight quantization (MatMul → MatMulNBits) for the browser.

    transformers.js loads this with dtype:'q4'. We keep activations/embeddings in
    fp32 and 4-bit only the big MatMul weights — the standard q4 recipe and the
    one that preserves accuracy best at 0.5B.
    """
    import onnx
    from onnxruntime.quantization.matmul_nbits_quantizer import MatMulNBitsQuantizer

    model = onnx.load(str(fp32_onnx), load_external_data=True)
    quant = MatMulNBitsQuantizer(model, bits=4, block_size=block_size, is_symmetric=True)
    quant.process()
    out_onnx.parent.mkdir(parents=True, exist_ok=True)
    quant.model.save_model_to_file(str(out_onnx), use_external_data_format=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", required=True, help="HF checkpoint dir from sft.py")
    ap.add_argument("--out", default="exports/nl-query-0.5b-onnx")
    ap.add_argument("--task", default="text-generation-with-past")
    ap.add_argument("--q4", action="store_true", help="also emit a 4-bit quantized onnx/model_q4.onnx")
    args = ap.parse_args()

    out = Path(args.out)
    cmd = ["optimum-cli", "export", "onnx",
           "--model", args.checkpoint, "--task", args.task, str(out)]
    print("running:", " ".join(cmd))
    rc = subprocess.call(cmd)
    if rc != 0:
        sys.exit(rc)
    print(f"\nExported (fp32) to {out}")

    if args.q4:
        # transformers.js convention: dtype-specific files live under onnx/.
        onnx_dir = out / "onnx"
        onnx_dir.mkdir(exist_ok=True)
        # Keep an fp32 copy under onnx/ too (dtype:'fp32' fallback).
        if (out / "model.onnx").exists() and not (onnx_dir / "model.onnx").exists():
            shutil.copy(out / "model.onnx", onnx_dir / "model.onnx")
            for ext in ("model.onnx_data", "model.onnx.data"):
                if (out / ext).exists():
                    shutil.copy(out / ext, onnx_dir / ext)
        q4_path = onnx_dir / "model_q4.onnx"
        print(f"quantizing q4 → {q4_path}")
        quantize_q4(out / "model.onnx", q4_path)
        print(f"q4 written: {q4_path}")

    print("Next: verify parity with eval/predict.py --backend onnx, then wire into nq_generate.js.")


if __name__ == "__main__":
    main()
