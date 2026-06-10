"""datagen/paraphrase.py — diversify the NL side of an example.

The gold PromQL is fixed; only the natural-language request varies. Two backends:

  * default  — a deterministic, stdlib-only synonym/template augmenter. Runs
               offline with no dependencies, good enough to exercise the whole
               pipeline and to bootstrap.
  * teacher  — when REZOLUS_TEACHER is set, paraphrasing is delegated to a
               stronger model (you wire this up on the training host). The teacher
               MUST paraphrase the NL only and never emit PromQL.

Determinism: variants are produced with a seeded RNG derived from the input text,
so regenerating the dataset is reproducible. The teacher backend seeds torch from
the input text for the same reason.

Teacher backend
---------------
No hosted API key was available on the training host, so the teacher is a LOCAL
instruct model run on the GPU (the README's "generate a paraphrase bank yourself"
path). Select it with, e.g.:

    export REZOLUS_TEACHER=hf:Qwen/Qwen2.5-3B-Instruct   # or local:<model_id>

The value may also be a bare HF model id containing "/". The model is loaded once
and cached. Every candidate it produces is passed through a hard PromQL filter:
anything that looks like a query (parens/brackets/braces, a slash, or a PromQL
keyword like irate/sum/histogram_quantile) is rejected, so the teacher can never
poison the gold label. Slots the teacher fails to fill are topped up from the
offline augmenter, and any load/generation failure falls back to it entirely —
the data pipeline never crashes because the teacher is unhappy.
"""
from __future__ import annotations

import os
import random
import re
import sys

# term -> alternatives (the term itself is always an option).
SYNONYMS = {
    "show me": ["show me", "display", "graph", "plot", "what is", "give me"],
    "usage": ["usage", "utilization", "consumption", "use"],
    "cpu": ["cpu", "processor", "CPU"],
    "memory": ["memory", "mem", "RAM"],
    "over time": ["over time", "as a time series", "trend", ""],
    "total": ["total", "overall", "aggregate", "summed"],
    "average": ["average", "avg", "mean"],
    "per": ["per", "for each", "divided by", "normalized by"],
    "by": ["by", "broken down by", "grouped by", "per"],
    "latency": ["latency", "delay", "response time"],
    "top 5": ["top 5", "highest 5", "top five"],
    "across all": ["across all", "across every", "summed over"],
}

PREFIXES = ["", "", "can you ", "please ", "i want to see "]
SUFFIXES = ["", "", "", "?", " please"]


def _apply_synonyms(text: str, rng: random.Random) -> str:
    out = text
    # Longest phrases first so "show me" wins over "show". Word-boundary matching
    # so "by" does not match inside "bytes" (would corrupt the request).
    for term in sorted(SYNONYMS, key=len, reverse=True):
        pat = re.compile(r"\b" + re.escape(term) + r"\b")
        if pat.search(out):
            out = pat.sub(lambda _m: rng.choice(SYNONYMS[term]), out, count=1)
    return out


def _normalize(s: str) -> str:
    return " ".join(s.split()).strip()


def _offline(nl: str, n: int, seed: int) -> list:
    """Deterministic, stdlib-only augmenter. Always includes the canonical form."""
    rng = random.Random(f"{seed}:{nl}")
    variants = {_normalize(nl)}
    attempts = 0
    while len(variants) < n and attempts < n * 8:
        attempts += 1
        body = _apply_synonyms(nl, rng)
        cand = _normalize(rng.choice(PREFIXES) + body + rng.choice(SUFFIXES))
        if cand:
            variants.add(cand)
    return list(variants)[:n]


def paraphrase(nl: str, n: int = 4, seed: int = 0) -> list:
    """Return up to `n` distinct phrasings of `nl` (includes the canonical form)."""
    teacher = os.environ.get("REZOLUS_TEACHER")
    if teacher:
        return _teacher_paraphrase(nl, n, teacher, seed)
    return _offline(nl, n, seed)


# ── teacher backend (local instruct model) ──────────────────────────────────

# A candidate is rejected as "looks like a query" if it contains any of these.
# This is the safety net that guarantees the teacher only ever touches the NL:
# even if it ignores the instruction and emits PromQL, the candidate is dropped.
_QUERYISH = re.compile(
    r"[(){}\[\]/=]"                                  # query punctuation
    r"|\b(?:irate|rate|sum|avg|min|max|count|topk|bottomk|quantile"
    r"|histogram_quantile|by|without|group_left|group_right|offset)\s*\("
)
# Common chat-model preamble we strip if it leaks through.
_PREAMBLE = re.compile(r"^\s*(?:sure|here(?:'s| are| is)?|okay|ok|certainly)\b.*?:\s*", re.I)
_LIST_NUM = re.compile(r"^\s*(?:[-*•]|\d+[.)])\s*")

# Cache: model_id -> (tokenizer, model, torch_module) or the string "FAILED".
_TEACHER_CACHE: dict = {}


def _resolve_model_id(backend: str) -> str:
    b = backend.strip()
    for prefix in ("hf:", "local:", "huggingface:"):
        if b.lower().startswith(prefix):
            return b[len(prefix):].strip()
    return b  # bare model id (e.g. "Qwen/Qwen2.5-3B-Instruct")


def _load_teacher(model_id: str):
    """Lazy-load + cache the instruct model. Returns None on any failure."""
    cached = _TEACHER_CACHE.get(model_id)
    if cached == "FAILED":
        return None
    if cached is not None:
        return cached
    try:
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer

        tok = AutoTokenizer.from_pretrained(model_id)
        model = AutoModelForCausalLM.from_pretrained(
            model_id,
            torch_dtype=torch.bfloat16,
            device_map="cuda" if torch.cuda.is_available() else None,
        )
        model.eval()
        _TEACHER_CACHE[model_id] = (tok, model, torch)
        print(f"[paraphrase] teacher loaded: {model_id} "
              f"(cuda={torch.cuda.is_available()})", file=sys.stderr)
        return _TEACHER_CACHE[model_id]
    except Exception as e:  # noqa: BLE001 — never let the teacher crash datagen
        print(f"[paraphrase] teacher load FAILED ({model_id}): {e!r}; "
              "falling back to offline augmenter", file=sys.stderr)
        _TEACHER_CACHE[model_id] = "FAILED"
        return None


def _clean_candidate(line: str) -> str:
    line = _PREAMBLE.sub("", line)
    line = _LIST_NUM.sub("", line)
    line = line.strip().strip('"').strip("'").strip()
    return _normalize(line)


def _is_ok(cand: str, nl: str) -> bool:
    if not cand or len(cand) > 200:
        return False
    if _QUERYISH.search(cand):           # looks like PromQL → reject
        return False
    # must be wordy NL, not a token dump
    return len(cand.split()) >= 2


_SYS = (
    "You rewrite short natural-language questions about systems performance "
    "metrics (CPU, memory, network, scheduler, syscalls, latency, etc.). "
    "Rephrase the user's request in varied, natural ways a real engineer might "
    "type it. Preserve the exact meaning, metrics, and any 'per'/'by'/'top' "
    "relationships. Do NOT answer it, do NOT write any query, code, PromQL, "
    "metric names with underscores, or punctuation like ()[]{}/. Output ONLY the "
    "rephrasings, one per line, nothing else."
)


def _teacher_paraphrase(nl: str, n: int, backend: str, seed: int = 0) -> list:
    """Strong-model paraphraser (local instruct model on the GPU).

    Contract: return up to `n` natural phrasings of `nl`, including the canonical
    form. NEVER produce PromQL — every candidate is filtered. Falls back to the
    offline augmenter to top up missing slots or on any failure.
    """
    model_id = _resolve_model_id(backend)
    loaded = _load_teacher(model_id)
    if loaded is None:
        return _offline(nl, n, seed)

    tok, model, torch = loaded
    canonical = _normalize(nl)
    variants = {canonical}                      # always keep the exact intent
    want = max(0, n - 1)                         # extra phrasings to ask for

    if want > 0:
        try:
            msgs = [
                {"role": "system", "content": _SYS},
                {"role": "user", "content": f"Request: {nl}\n\n"
                                            f"Give {want} different rephrasings, one per line."},
            ]
            prompt = tok.apply_chat_template(
                msgs, tokenize=False, add_generation_prompt=True)
            inputs = tok(prompt, return_tensors="pt").to(model.device)

            # Deterministic per-input sampling so regeneration is reproducible.
            torch.manual_seed((hash((seed, nl)) & 0x7FFFFFFF))
            with torch.no_grad():
                out = model.generate(
                    **inputs,
                    do_sample=True,
                    temperature=0.9,
                    top_p=0.95,
                    max_new_tokens=160,
                    num_return_sequences=2,      # two drafts → more usable lines
                    pad_token_id=tok.pad_token_id or tok.eos_token_id,
                )
            gen = out[:, inputs["input_ids"].shape[1]:]
            for seq in tok.batch_decode(gen, skip_special_tokens=True):
                for raw in seq.splitlines():
                    cand = _clean_candidate(raw)
                    if _is_ok(cand, nl) and cand.lower() != canonical.lower():
                        variants.add(cand)
                    if len(variants) >= n:
                        break
                if len(variants) >= n:
                    break
        except Exception as e:  # noqa: BLE001
            print(f"[paraphrase] teacher generate failed for {nl!r}: {e!r}; "
                  "topping up offline", file=sys.stderr)

    out_list = list(variants)[:n]
    if len(out_list) < n:                        # top up from the offline augmenter
        for cand in _offline(nl, n, seed):
            if cand not in out_list:
                out_list.append(cand)
            if len(out_list) >= n:
                break
    return out_list[:n]
