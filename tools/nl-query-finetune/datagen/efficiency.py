"""datagen/efficiency.py — curated derived-efficiency (KPI) composition queries.

The random `ratio_example` pairs in templates.py mostly produce meaningless
ratios, and the dashboard harvest only covers whatever plots happen to exist —
so the genuinely useful *efficiency* metrics an engineer asks for (IPC, cache hit
rate, branch-miss rate, average IO size, frequency scaling, …) are under-covered.

This module fills that gap with a CURATED catalog of well-known derived KPIs.
Integrity is preserved exactly as elsewhere: we author only the metric pairing
and the natural-language phrasings; the gold PromQL is built from the SAME
type-aware conventions as templates.py (`irate(counter[1s])`, `sum(...)`), so it
is correct-by-construction and is still execution-validated downstream by
generate.py (any KPI whose metrics are absent, or whose query returns empty on
the fixture, is dropped). Nothing here is a hand-written, unvalidated label.

A KPI only emits when ALL of its base metrics are present in the supplied schema,
so the same catalog adapts to demo / cachecannon / vllm / future parquets.
"""
from __future__ import annotations

from datagen.templates import Example, base_expr


def _u(seq):
    out, seen = [], set()
    for s in seq:
        s = " ".join(s.split()).strip()
        if s and s not in seen:
            seen.add(s)
            out.append(s)
    return tuple(out)


def _present(cards: dict, *names) -> bool:
    return all(n in cards for n in names)


def _ratio(cards, num, den) -> str:
    """sum(<base_expr num>) / sum(<base_expr den>) — counters become irate[1s]."""
    return f"sum({base_expr(cards[num])}) / sum({base_expr(cards[den])})"


def _share(cards, num, *terms) -> str:
    """num / (term0 + term1 + …) — e.g. hits / (hits + misses)."""
    den = " + ".join(f"sum({base_expr(cards[t])})" for t in terms)
    return f"sum({base_expr(cards[num])}) / ({den})"


def efficiency_examples(cards: dict):
    """Yield Example objects for every KPI whose base metrics exist in `cards`."""
    out = []

    def add(metrics, gold, variants):
        v = _u(variants)
        out.append(Example("efficiency", v[0], gold, tuple(metrics), v))

    # IPC / CPI — try non-cgroup and cgroup-scoped instruction/cycle counters.
    for ins, cyc, scope in (
        ("cpu_instructions", "cpu_cycles", ""),
        ("cgroup_cpu_instructions", "cgroup_cpu_cycles", "cgroup "),
    ):
        if _present(cards, ins, cyc):
            add((ins, cyc), _ratio(cards, ins, cyc), [
                f"{scope}instructions per cycle", f"{scope}IPC", f"{scope}ipc",
                f"{scope}instructions per clock cycle",
                f"{scope}instructions retired per cycle",
                f"how many instructions per cycle {scope}".strip(),
            ])
            add((cyc, ins), _ratio(cards, cyc, ins), [
                f"{scope}cycles per instruction", f"{scope}CPI", f"{scope}cpi",
                f"{scope}clock cycles per instruction",
            ])

    # Branch misprediction rate.
    if _present(cards, "cpu_branch_misses", "cpu_branch_instructions"):
        add(("cpu_branch_misses", "cpu_branch_instructions"),
            _ratio(cards, "cpu_branch_misses", "cpu_branch_instructions"), [
                "branch misprediction rate", "branch miss rate",
                "fraction of branches mispredicted", "branch miss ratio",
                "branch mispredictions per branch",
                "how often are branches mispredicted",
            ])

    # Cache hit / miss rate — denominator is hits + misses (new structure).
    if _present(cards, "cache_hits", "cache_misses"):
        add(("cache_hits", "cache_misses"),
            _share(cards, "cache_hits", "cache_hits", "cache_misses"), [
                "cache hit rate", "cache hit ratio",
                "fraction of cache accesses that hit",
                "what fraction of cache lookups hit",
                "cache hits as a share of total accesses",
            ])
        add(("cache_misses", "cache_hits"),
            _share(cards, "cache_misses", "cache_hits", "cache_misses"), [
                "cache miss rate", "cache miss ratio",
                "fraction of cache accesses that miss",
                "what fraction of cache lookups miss",
            ])

    # Average block-IO size: bytes per operation.
    if _present(cards, "blockio_bytes", "blockio_operations"):
        add(("blockio_bytes", "blockio_operations"),
            _ratio(cards, "blockio_bytes", "blockio_operations"), [
                "average io size", "average block io size",
                "bytes per io operation", "mean request size",
                "average size of block io requests", "bytes per block operation",
            ])

    # Average packet size: bytes per packet.
    if _present(cards, "network_bytes", "network_packets"):
        add(("network_bytes", "network_packets"),
            _ratio(cards, "network_bytes", "network_packets"), [
                "average packet size", "bytes per packet", "mean packet size",
                "network bytes per packet",
            ])

    # CPU frequency scaling: aperf / mperf.
    if _present(cards, "cpu_aperf", "cpu_mperf"):
        add(("cpu_aperf", "cpu_mperf"),
            _ratio(cards, "cpu_aperf", "cpu_mperf"), [
                "cpu frequency scaling factor", "aperf to mperf ratio",
                "frequency scaling factor", "effective frequency ratio",
                "how much are the cpus boosting", "turbo ratio",
            ])

    # TLB flushes per cycle.
    if _present(cards, "cpu_tlb_flush", "cpu_cycles"):
        add(("cpu_tlb_flush", "cpu_cycles"),
            _ratio(cards, "cpu_tlb_flush", "cpu_cycles"), [
                "tlb flushes per cycle", "tlb flush rate per cycle",
                "tlb flushes relative to cycles",
            ])

    # Instructions per syscall (work done per system call).
    if _present(cards, "cpu_instructions", "syscall"):
        add(("cpu_instructions", "syscall"),
            _ratio(cards, "cpu_instructions", "syscall"), [
                "instructions per syscall", "cpu instructions per syscall",
                "instructions retired per system call",
            ])

    # ── cost-per-unit-of-traffic/work ───────────────────────────────────────
    # "How much compute does each packet / byte / request cost?" — the core
    # efficiency question for cache/CDN/proxy workloads. Pair each COST metric
    # (a numerator: compute spent) with each WORK unit (a denominator: traffic
    # served). NL nouns are kept disjoint so one phrasing never maps to two
    # golds (bare "packet" -> network_packets; "tcp packet" -> tcp_packets).
    COSTS = [
        ("cpu_usage", "cpu usage", "cpu time"),
        ("cpu_instructions", "instructions", "cpu instructions"),
        ("cpu_cycles", "cycles", "cpu cycles"),
        ("syscall", "syscalls", "system calls"),
    ]
    UNITS = [
        ("network_packets", "packet", "network packet"),
        ("network_bytes", "network byte", "byte served"),
        ("tcp_packets", "tcp packet", "tcp packet"),
        ("blockio_operations", "io operation", "block io operation"),
        ("cache_hits", "cache hit", "cached request"),
    ]
    for cm, c1, c2 in COSTS:
        for um, u1, u2 in UNITS:
            if cm == um or not _present(cards, cm, um):
                continue
            add((cm, um), _ratio(cards, cm, um), [
                f"{c1} per {u1}", f"{c1} per {u1} processed",
                f"how much {c1} per {u1}", f"{c1} cost per {u1}",
                f"{c1} for each {u1}", f"{c2} per {u1}",
            ])

    # ── error / loss rates (a bad event relative to total work) ─────────────
    ERRORS = [
        ("tcp_retransmit", "tcp_packets",
         ["tcp retransmission rate", "retransmit rate", "retransmits per packet",
          "fraction of packets retransmitted", "how often packets are retransmitted"]),
        ("network_drop", "network_packets",
         ["packet drop rate", "dropped packets per packet", "network drop rate",
          "fraction of packets dropped"]),
        ("cpu_dtlb_miss", "cpu_instructions",
         ["dtlb miss rate", "data tlb misses per instruction",
          "dtlb misses per instruction"]),
        ("memory_numa_miss", "cpu_instructions",
         ["numa miss rate", "numa misses per instruction",
          "remote numa accesses per instruction"]),
    ]
    for bad, total, variants in ERRORS:
        if _present(cards, bad, total):
            add((bad, total), _ratio(cards, bad, total), variants)

    # ── utilization normalized by capacity ─────────────────────────────────
    # cpu_usage is nanoseconds of CPU time; dividing by cpu_cores and 1e9 yields
    # the fraction of total CPU capacity in use (the dashboards' "% busy" idiom).
    if _present(cards, "cpu_usage", "cpu_cores"):
        add(("cpu_usage", "cpu_cores"),
            f"sum({base_expr(cards['cpu_usage'])}) / sum(cpu_cores) / 1000000000", [
                "cpu utilization", "overall cpu utilization",
                "cpu utilization as a fraction of capacity", "how busy are the cpus",
                "system-wide cpu utilization", "what fraction of cpu capacity is in use",
            ])

    return out
