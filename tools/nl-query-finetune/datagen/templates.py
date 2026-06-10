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
    # Authored, intent-aware NL phrasings of the SAME request (gold unchanged).
    # Emitted here, where the metric phrase / label key / filter value are in
    # scope, so they are higher quality than lexical synonym-swapping a single
    # string. generate.py expands the example over these (+ offline top-up).
    variants: tuple = ()


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


def _uniq(seq):
    """Order-preserving dedupe; drops empties."""
    out, seen = [], set()
    for s in seq:
        s = " ".join(s.split()).strip()
        if s and s not in seen:
            seen.add(s)
            out.append(s)
    return tuple(out)


# ── authored, intent-aware NL phrasings (NL only; gold is unchanged) ─────────
# These read like questions a real engineer types. The canonical phrasing is
# always included first so the dataset still contains it.

def _v_lookup(phrase):
    return _uniq([
        f"show me {phrase} over time", f"{phrase} over time", f"graph {phrase}",
        f"plot {phrase}", f"chart {phrase} as a time series",
        f"how is {phrase} changing over time", f"{phrase} trend",
        f"track {phrase} over time",
    ])


def _v_hist_lookup(phrase):
    return _uniq([
        f"show me {phrase}", f"{phrase}", f"what is the {phrase}",
        f"display {phrase}", f"give me {phrase}",
    ])


def _v_total(phrase):
    return _uniq([
        f"total {phrase}", f"overall {phrase}", f"aggregate {phrase}",
        f"{phrase} across the whole system", f"system-wide {phrase}",
        f"sum of {phrase}", f"combined {phrase}", f"total {phrase} across all",
    ])


def _v_average(phrase):
    return _uniq([
        f"average {phrase} across all", f"average {phrase}", f"avg {phrase}",
        f"mean {phrase}", f"typical {phrase}", f"average {phrase} across everything",
    ])


def _v_by_label(phrase, key):
    return _uniq([
        f"{phrase} by {key}", f"{phrase} broken down by {key}",
        f"{phrase} grouped by {key}", f"{phrase} per {key}",
        f"break down {phrase} by {key}", f"{phrase} split by {key}",
        f"show {phrase} for each {key}",
    ])


def _v_filter(phrase, key, value):
    return _uniq([
        f"{value} {phrase}", f"{phrase} for {value}",
        f"{phrase} where {key} is {value}", f"{phrase} with {key} {value}",
        f"just the {value} {phrase}", f"{phrase} filtered to {value}",
    ])


def _v_quantile(phrase, pct):
    out = [
        f"p{pct} {phrase}", f"{pct}th percentile {phrase}", f"p{pct} of {phrase}",
        f"{phrase} at the {pct}th percentile", f"{phrase} p{pct}",
    ]
    if pct == 50:
        out += [f"median {phrase}", f"typical {phrase}"]
    elif pct >= 99:
        out += [f"tail {phrase}", f"worst-case {phrase}", f"{phrase} tail latency"]
    return _uniq(out)


def _v_topk(phrase, key):
    return _uniq([
        f"top 5 {key} by {phrase}", f"top 5 {key} with the most {phrase}",
        f"which {key} have the highest {phrase}", f"5 busiest {key} by {phrase}",
        f"top five {key} for {phrase}", f"highest {phrase}, by {key}",
    ])


def _v_peak(phrase):
    return _uniq([
        f"peak {phrase}", f"max {phrase}", f"maximum {phrase}",
        f"highest {phrase} on any core", f"the hottest {phrase}",
        f"busiest core's {phrase}",
    ])


def _v_low(phrase):
    return _uniq([
        f"lowest {phrase}", f"min {phrase}", f"minimum {phrase}",
        f"least {phrase} on any core", f"the quietest {phrase}",
    ])


def _v_by_index(phrase, noun):
    # Per-CPU/core breakout — the multi-series shape the viewer renders as a
    # HEATMAP. "heatmap"/"per core"/"by cpu" all map to sum by (<index>)(...).
    out = []
    for n in (noun, "cpu" if noun != "cpu" else "core"):
        out += [f"{phrase} per {n}", f"{phrase} by {n}", f"{phrase} for each {n}",
                f"{phrase} across each {n}", f"per-{n} {phrase}",
                f"{phrase} broken out by {n}"]
    out += [f"{phrase} heatmap", f"{phrase} as a heatmap", f"{phrase} per-cpu heatmap"]
    return _uniq(out)


def _v_share(phrase, key, value):
    return _uniq([
        f"fraction of {phrase} that is {value}",
        f"share of {phrase} that is {value}",
        f"what fraction of {phrase} is {value}",
        f"percentage of {phrase} that is {value}",
        f"{value} as a share of total {phrase}",
        f"how much of {phrase} is {value}",
    ])


def _v_ratio(a, b):
    return _uniq([
        f"{a} per {b}", f"{a} for each {b}", f"{a} divided by {b}",
        f"ratio of {a} to {b}", f"{a} normalized by {b}", f"how much {a} per {b}",
        f"{a} relative to {b}", f"{a} over {b}",
    ])


def examples_for_metric(card: dict):
    """Yield Example objects for a single metric card."""
    name = card["name"]
    phrase = humanize(name)
    t = card["type"]
    m = (name,)

    if t == "histogram":
        # Histograms: a percentile is the natural reduction. No rate/sum.
        yield Example("lookup", f"show me {phrase}", base_expr(card), m,
                      _v_hist_lookup(phrase))
        for q in QUANTILES:
            pct = int(round(q * 100))
            yield Example("quantile", f"p{pct} {phrase}",
                          f"histogram_quantile({q}, {name})", m,
                          _v_quantile(phrase, pct))
        return

    base = base_expr(card)

    # Single-metric lookups and reductions.
    yield Example("lookup", f"show me {phrase} over time", base, m,
                  _v_lookup(phrase))
    yield Example("total", f"total {phrase}", f"sum({base})", m, _v_total(phrase))
    if _has_index(card):
        yield Example("average", f"average {phrase} across all",
                      f"avg({base})", m, _v_average(phrase))
        # Peak / min across the index dimension (busiest/quietest core, etc.).
        yield Example("peak", f"peak {phrase}", f"max({base})", m, _v_peak(phrase))
        yield Example("peak", f"lowest {phrase}", f"min({base})", m, _v_low(phrase))
        # Per-index (per-CPU/core) breakout → multi-series, rendered as a HEATMAP.
        ikey = next((k for k in card.get("labels", {}) if k in INDEX_LABELS), None)
        if ikey:
            noun = ikey if ikey in ("cpu", "core") else "core"
            yield Example("by_index", f"{phrase} per {noun}",
                          f"sum by ({ikey}) ({base})", m, _v_by_index(phrase, noun))

    # Categorical breakouts, filters, and part-of-whole shares.
    for key, values in _categories(card):
        yield Example("by_label", f"{phrase} by {key}",
                      f"sum by ({key}) ({base})", m, _v_by_label(phrase, key))
        for v in sorted(values)[:MAX_FILTER_VALUES]:
            filt = base_expr(card, {key: v})
            yield Example("filter", f"{v} {phrase}", filt, m,
                          _v_filter(phrase, key, v))
            # Share: this value as a fraction of the whole (only meaningful when
            # the label actually partitions the metric, i.e. >= 2 values).
            if len(values) >= 2:
                yield Example("share", f"fraction of {phrase} that is {v}",
                              f"sum({base_expr(card, {key: v})}) / sum({base})",
                              m, _v_share(phrase, key, v))

    # Top-k over a high-cardinality grouping label (e.g. cgroup `name`).
    for key, values in card.get("labels", {}).items():
        if key in INDEX_LABELS:
            continue
        if len(values) > MAX_BREAKOUT_VALUES:
            yield Example("topk", f"top 5 {key} by {phrase}",
                          f"topk(5, sum by ({key}) ({base}))", m,
                          _v_topk(phrase, key))
            break


def ratio_example(a: dict, b: dict) -> Example:
    """A composition example: 'A per B' -> sum(base_a) / sum(base_b)."""
    pa, pb = humanize(a["name"]), humanize(b["name"])
    return Example(
        intent="ratio",
        nl=f"{pa} per {pb}",
        promql=f"sum({base_expr(a)}) / sum({base_expr(b)})",
        metrics=(a["name"], b["name"]),
        variants=_v_ratio(pa, pb),
    )
