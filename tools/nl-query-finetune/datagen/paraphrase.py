"""datagen/paraphrase.py — diversify the NL side of an example.

The gold PromQL is fixed; only the natural-language request varies. Two backends:

  * default  — a deterministic, stdlib-only synonym/template augmenter. Runs
               offline with no dependencies, good enough to exercise the whole
               pipeline and to bootstrap.
  * teacher  — when REZOLUS_TEACHER is set, paraphrasing is delegated to a
               stronger model (you wire this up on the training host). The teacher
               MUST paraphrase the NL only and never emit PromQL.

Determinism: variants are produced with a seeded RNG derived from the input text,
so regenerating the dataset is reproducible.
"""
from __future__ import annotations

import os
import random
import re

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


def paraphrase(nl: str, n: int = 4, seed: int = 0) -> list:
    """Return up to `n` distinct phrasings of `nl` (includes the canonical form)."""
    teacher = os.environ.get("REZOLUS_TEACHER")
    if teacher:
        return _teacher_paraphrase(nl, n, teacher)

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


def _teacher_paraphrase(nl: str, n: int, backend: str) -> list:
    """Hook for a strong-model paraphraser. Implement on the training host.

    Contract: return `n` natural phrasings of `nl`. NEVER produce PromQL. The
    gold query is supplied separately and must not change.
    """
    raise NotImplementedError(
        f"REZOLUS_TEACHER={backend!r} set but no teacher backend is wired up. "
        "Implement _teacher_paraphrase (e.g. call an API to rephrase the NL only), "
        "or unset REZOLUS_TEACHER to use the offline augmenter."
    )
