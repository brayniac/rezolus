#!/usr/bin/env python3
"""Phase 2: supervised fine-tuning (runs on the CUDA host, NOT locally).

Full fine-tune of a ~0.5B code model on the chat records from datagen. Uses the
model's chat template and computes loss on the ASSISTANT completion only.

This is a documented starting point — verify versions/flags against your stack.

  pip install -r ../requirements.txt
  python train/sft.py --config train/config.yaml

Outputs a HF checkpoint at config.output_dir; feed it to export/to_onnx.py.
"""
from __future__ import annotations

import argparse
import json
import sys


def load_config(path):
    try:
        import yaml
    except ImportError:
        sys.exit("pip install pyyaml (or inline the config as JSON)")
    return yaml.safe_load(open(path))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", default="train/config.yaml")
    args = ap.parse_args()
    cfg = load_config(args.config)

    # Imported lazily so Phases 0–1 + eval need none of this installed.
    import torch
    from datasets import load_dataset
    from transformers import AutoModelForCausalLM, AutoTokenizer
    from trl import SFTConfig, SFTTrainer, DataCollatorForCompletionOnlyLM

    tok = AutoTokenizer.from_pretrained(cfg["base_model"])
    if tok.pad_token is None:
        tok.pad_token = tok.eos_token

    # Records are {"messages": [...]} → render with the chat template into a
    # single "text" field. The DataCollatorForCompletionOnlyLM below masks every
    # token before the assistant header, so loss is computed ONLY on the assistant
    # completion (PROMPT_FORMAT.md). add_generation_prompt MUST be False here so
    # the gold completion is included in the rendered text.
    ds = load_dataset("json", data_files={
        "train": cfg["train_file"], "eval": cfg["eval_file"]})

    def to_text(ex):
        return {"text": tok.apply_chat_template(ex["messages"], tokenize=False)}

    ds = ds.map(to_text, remove_columns=ds["train"].column_names)

    model = AutoModelForCausalLM.from_pretrained(
        cfg["base_model"],
        torch_dtype=torch.bfloat16 if cfg.get("bf16") else None,
        attn_implementation=cfg.get("attn_implementation", "sdpa"),
    )

    # Completion-only loss is incompatible with packing — force packing off.
    if cfg.get("packing"):
        print("[sft] completion_only_loss → forcing packing=False")
    collator = DataCollatorForCompletionOnlyLM(
        response_template=cfg.get("response_template", "<|im_start|>assistant\n"),
        tokenizer=tok,
    )

    sft = SFTConfig(
        output_dir=cfg["output_dir"],
        learning_rate=cfg["learning_rate"],
        lr_scheduler_type=cfg["lr_scheduler_type"],
        warmup_ratio=cfg["warmup_ratio"],
        num_train_epochs=cfg["num_train_epochs"],
        per_device_train_batch_size=cfg["per_device_train_batch_size"],
        gradient_accumulation_steps=cfg["gradient_accumulation_steps"],
        weight_decay=cfg["weight_decay"],
        max_grad_norm=cfg["max_grad_norm"],
        bf16=cfg.get("bf16", True),
        packing=False,                       # completion-only collator needs unpacked rows
        max_seq_length=cfg.get("max_seq_length", 1024),
        gradient_checkpointing=cfg.get("gradient_checkpointing", True),
        eval_strategy=cfg.get("eval_strategy", "steps"),
        eval_steps=cfg.get("eval_steps", 100),
        save_steps=cfg.get("save_steps", 100),
        save_total_limit=cfg.get("save_total_limit", 3),
        logging_steps=cfg.get("logging_steps", 20),
        load_best_model_at_end=cfg.get("load_best_model_at_end", True),
        metric_for_best_model=cfg.get("metric_for_best_model", "eval_loss"),
        optim=cfg.get("optim", "adamw_torch"),  # e.g. adafactor to fit 1.5B full-FT in 24GB
        report_to=[],                        # no W&B/etc. on the training host
    )

    trainer = SFTTrainer(
        model=model, args=sft, processing_class=tok,
        train_dataset=ds["train"], eval_dataset=ds["eval"],
        data_collator=collator,
    )
    trainer.train()
    trainer.save_model(cfg["output_dir"])
    tok.save_pretrained(cfg["output_dir"])
    print(json.dumps({"saved": cfg["output_dir"]}))


if __name__ == "__main__":
    main()
