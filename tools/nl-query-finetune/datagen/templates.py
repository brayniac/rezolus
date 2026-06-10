"""datagen/templates.py — deterministic intent → gold-PromQL templates.

This module IS the label generator: given a metric card (name, type, labels), it
emits (intent, canonical_nl, gold_promql, metrics_used) tuples whose PromQL is
*correct by construction*. The data generator validates each gold by executing it
(see rezolus_oracle) and then paraphrases only the NL side.

Conventions (project rules):
  counter   → irate(SEL[1s])         (instantaneous rate)
  gauge     → SEL
  histogram → histogram_quantile(q, SEL)
  aggregate → sum(...) / avg(...)
  breakout  → sum by (LABEL)(...)
  filter    → SEL{LABEL="value"}
  ratio     → sum(A) / sum(B)

Pure / stdlib-only and unit-testable.
"""
from __future__ import annotations

from dataclasses import dataclass

# Labels whose values index a resource (a CPU, a core …). These become the
# heatmap/Y dimension and what we sum across — never a categorical breakout.
INDEX_LABELS = {"id", "cpu", "core"}

# Cardinality ceilings so we don't enumerate huge label spaces.
MAX_BREAKOUT_VALUES = 8      # only break out / facet low-cardinality labels
MAX_FILTER_VALUES = 4        # at most this many filter examples per (metric,label)
COUNTER_WINDOW = "1s"
DEFAULT_QUANTILE = 0.99
QUANTILES = [0.5, 0.9, 0.99]


@dataclass
class Example:
    intent: str
    nl: str                 # canonical phrasing (paraphrased later)
    promql: str             # gold, validated downstream
    metrics: tuple          # metric names referenced (for selection-accuracy eval)


def humanize(name: str) -> str:
    """cpu_usage -> 'cpu usage'."""
    return name.replace("_", " ")


def _sel(metric: str, filters: dict | None = None) -> str:
    if not filters:
        return metric
    inner = ",".join(f'{k}="{v}"' for k, v in filters.items())
    return f"{metric}{{{inner}}}"


def base_expr(card: dict, filters: dict | None = None) -> str:
    """Type-appropriate base expression for a metric selector."""
    sel = _sel(card["name"], filters)
    t = card["type"]
    if t == "counter":
        return f"irate({sel}[{COUNTER_WINDOW}])"
    if t == "histogram":
        return f"histogram_quantile({DEFAULT_QUANTILE}, {sel})"
    return sel  # gauge / unknown


def _categories(card: dict):
    """Low-cardinality categorical labels suitable for breakout/filter."""
    out = []
    for key, values in card.get("labels", {}).items():
        if key in INDEX_LABELS:
            continue
        if 1 <= len(values) <= MAX_BREAKOUT_VALUES:
            out.append((key, values))
    return out


def _has_index(card: dict) -> bool:
    return any(k in INDEX_LABELS for k in card.get("labels", {}))


def examples_for_metric(card: dict):
    """Yield Example objects for a single metric card."""
    name = card["name"]
    phrase = humanize(name)
    t = card["type"]
    m = (name,)

    if t == "histogram":
        # Histograms: a percentile is the natural reduction. No rate/sum.
        yield Example("lookup", f"show me {phrase}", base_expr(card), m)
        for q in QUANTILES:
            pct = int(round(q * 100))
            yield Example("quantile", f"p{pct} {phrase}",
                          f"histogram_quantile({q}, {name})", m)
        return

    base = base_expr(card)

    # Single-metric lookups and reductions.
    yield Example("lookup", f"show me {phrase} over time", base, m)
    yield Example("total", f"total {phrase}", f"sum({base})", m)
    if _has_index(card):
        yield Example("average", f"average {phrase} across all",
                      f"avg({base})", m)

    # Categorical breakouts and filters.
    for key, values in _categories(card):
        yield Example("by_label", f"{phrase} by {key}",
                      f"sum by ({key}) ({base})", m)
        for v in sorted(values)[:MAX_FILTER_VALUES]:
            filt = base_expr(card, {key: v})
            yield Example("filter", f"{v} {phrase}", filt, m)

    # Top-k over a high-cardinality grouping label (e.g. cgroup `name`).
    for key, values in card.get("labels", {}).items():
        if key in INDEX_LABELS:
            continue
        if len(values) > MAX_BREAKOUT_VALUES:
            yield Example("topk", f"top 5 {key} by {phrase}",
                          f"topk(5, sum by ({key}) ({base}))", m)
            break


def ratio_example(a: dict, b: dict) -> Example:
    """A composition example: 'A per B' -> sum(base_a) / sum(base_b)."""
    return Example(
        intent="ratio",
        nl=f"{humanize(a['name'])} per {humanize(b['name'])}",
        promql=f"sum({base_expr(a)}) / sum({base_expr(b)})",
        metrics=(a["name"], b["name"]),
    )
