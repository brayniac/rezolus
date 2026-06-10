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

    Saved as a SINGLE self-contained .onnx (no external .data file): onnxruntime-web
    can't mount an external-data sibling in the browser ("Module.MountedFiles is not
    available"). A 0.5B q4 model (~733 MB) is well under the 2 GB protobuf limit, so
    inlining is safe; for models that would exceed 2 GB you must use external data and
    a runtime that mounts it.
    """
    import onnx
    from onnxruntime.quantization.matmul_nbits_quantizer import MatMulNBitsQuantizer

    model = onnx.load(str(fp32_onnx), load_external_data=True)
    quant = MatMulNBitsQuantizer(model, bits=4, block_size=block_size, is_symmetric=True)
    quant.process()
    out_onnx.parent.mkdir(parents=True, exist_ok=True)
    # save_model_to_file may force external data; re-load and save inline to be sure.
    quant.model.save_model_to_file(str(out_onnx), use_external_data_format=True)
    inlined = onnx.load(str(out_onnx), load_external_data=True)
    onnx.save_model(inlined, str(out_onnx), save_as_external_data=False)
    for sib in (out_onnx.with_suffix(out_onnx.suffix + ".data"),
                out_onnx.with_suffix(out_onnx.suffix + "_data")):
        sib.unlink(missing_ok=True)


def quantize_q4f16(q4_onnx: Path, out_onnx: Path):
    """q4f16: take the int4 model and store the LEFTOVER fp32 tensors (the big
    embedding/lm_head table) in fp16 — roughly halving size (~733MB -> ~450MB at
    0.5B). The MatMulNBits int4 weights are opaque to the fp16 pass. transformers.js
    loads this with dtype:'q4f16'; IO types stay fp32 so the int64 input_ids /
    logits interface is unchanged.

    Order matters: int4 FIRST (so MatMul weights aren't fp16'd and skipped), then
    fp16. We onnxslim first to fuse redundant Cast/layernorm topology.

    CAVEAT: onnxconverter_common's fp16 pass mis-types some pass-through nodes on
    Qwen's attention graph (Cast/Unsqueeze), producing a model onnxruntime rejects.
    main() validates the output loads and removes it if not — in that case produce
    q4f16 with the transformers.js conversion script or onnxruntime-genai, which
    handle this op-set. q4 (fp32 embeddings) is the validated fallback.
    """
    import onnx
    from onnxconverter_common import float16
    try:
        import onnxslim
        loaded = onnxslim.slim(onnx.load(str(q4_onnx), load_external_data=True))
    except Exception:
        loaded = onnx.load(str(q4_onnx), load_external_data=True)
    model16 = float16.convert_float_to_float16(loaded, keep_io_types=True)
    out_onnx.parent.mkdir(parents=True, exist_ok=True)
    onnx.save_model(model16, str(out_onnx), save_as_external_data=True,
                    all_tensors_to_one_file=True,
                    location=out_onnx.name + ".data", convert_attribute=True)


def _onnx_loads(path: Path) -> bool:
    """True iff onnxruntime can build an inference session for the model."""
    try:
        import onnxruntime as ort
        ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
        return True
    except Exception as e:  # noqa: BLE001
        print(f"  [q4f16] model does not load: {str(e)[:160]}")
        return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", required=True, help="HF checkpoint dir from sft.py")
    ap.add_argument("--out", default="exports/nl-query-0.5b-onnx")
    ap.add_argument("--task", default="text-generation-with-past")
    ap.add_argument("--q4", action="store_true", help="also emit a 4-bit quantized onnx/model_q4.onnx")
    ap.add_argument("--q4f16", action="store_true",
                    help="also emit onnx/model_q4f16.onnx (fp16 model + int4 MatMuls; ~half the q4 size)")
    args = ap.parse_args()

    out = Path(args.out)
    cmd = ["optimum-cli", "export", "onnx",
           "--model", args.checkpoint, "--task", args.task, str(out)]
    print("running:", " ".join(cmd))
    rc = subprocess.call(cmd)
    if rc != 0:
        sys.exit(rc)
    print(f"\nExported (fp32) to {out}")

    if args.q4 or args.q4f16:
        # transformers.js convention: dtype-specific files live under onnx/.
        onnx_dir = out / "onnx"
        onnx_dir.mkdir(exist_ok=True)
        # Keep an fp32 copy under onnx/ too (dtype:'fp32' fallback).
        if (out / "model.onnx").exists() and not (onnx_dir / "model.onnx").exists():
            shutil.copy(out / "model.onnx", onnx_dir / "model.onnx")
            for ext in ("model.onnx_data", "model.onnx.data"):
                if (out / ext).exists():
                    shutil.copy(out / ext, onnx_dir / ext)
    if args.q4:
        q4_path = onnx_dir / "model_q4.onnx"
        print(f"quantizing q4 → {q4_path}")
        quantize_q4(out / "model.onnx", q4_path)
        print(f"q4 written: {q4_path}")
    if args.q4f16:
        # Needs the int4 model as input (int4 first, then fp16). Build it if absent.
        q4_path = onnx_dir / "model_q4.onnx"
        if not q4_path.exists():
            print(f"quantizing q4 (prereq) → {q4_path}")
            quantize_q4(out / "model.onnx", q4_path)
        q4f16_path = onnx_dir / "model_q4f16.onnx"
        print(f"quantizing q4f16 → {q4f16_path}")
        quantize_q4f16(q4_path, q4f16_path)
        if _onnx_loads(q4f16_path):
            print(f"q4f16 written + validated: {q4f16_path}")
        else:
            q4f16_path.unlink(missing_ok=True)
            (q4f16_path.parent / (q4f16_path.name + ".data")).unlink(missing_ok=True)
            print("q4f16 REMOVED (failed to load) — produce it with the transformers.js "
                  "conversion script / onnxruntime-genai; ship q4 meanwhile.")

    print("Next: verify parity with eval/predict.py --backend onnx, then wire into nq_generate.js.")


if __name__ == "__main__":
    main()
