#!/usr/bin/env bash
# Publish the exported ONNX model to the Hugging Face Hub for in-browser use.
#
# The viewer loads by model id via transformers.js (nq_generate.js DEFAULT_MODEL),
# so once this repo exists, point DEFAULT_MODEL at it with dtype:'q4'.
#
# Prereqs:  pip install -U huggingface_hub  &&  huggingface-cli login   (write token)
# Usage:    publish/publish_hf.sh [repo_id] [export_dir]
set -euo pipefail

REPO="${1:-iopsystems/nl-query-promql-0.5b-onnx}"
EXPORT_DIR="${2:-exports/nl-query-0.5b-onnx}"
HERE="$(cd "$(dirname "$0")" && pwd)"

if [ ! -f "$EXPORT_DIR/onnx/model_q4.onnx" ]; then
  echo "missing $EXPORT_DIR/onnx/model_q4.onnx — run export/to_onnx.py --q4 first" >&2
  exit 1
fi

# Drop the model card in as the repo README (transformers.js doesn't need it, but
# the Hub renders it).
cp "$HERE/MODEL_CARD.md" "$EXPORT_DIR/README.md"

echo "Creating repo $REPO (no-op if it exists)…"
huggingface-cli repo create "$REPO" --type model -y || true

echo "Uploading $EXPORT_DIR → $REPO …"
# Uploads tokenizer/config + onnx/. The fp32 model.onnx(.data) is large and
# optional for the browser (dtype:'q4' uses onnx/model_q4.onnx); exclude it to
# keep the repo lean, or drop --exclude to ship fp32 too.
huggingface-cli upload "$REPO" "$EXPORT_DIR" . \
  --exclude "model.onnx" --exclude "model.onnx_data"

cat <<EOF

Done. Next:
  - set DEFAULT_MODEL = '$REPO' in src/viewer/assets/lib/nq_generate.js (keep dtype:'q4')
  - mirror PROMPT_FORMAT.md in nq_prompt.js (already aligned)
EOF
