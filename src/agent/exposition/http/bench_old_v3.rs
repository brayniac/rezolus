//! BENCH BRANCH ONLY: the agent's `create_v3` and `SkeletonCache` as they
//! were before metriken's `GroupBuilder` replaced them (upstream/main
//! 7dd615c8), for `bench_old_vs_new`. Test-only; not for merge.
#![allow(dead_code)]
use super::*;
use crate::agent::timing::AcquisitionGuard;
use metriken_exposition::{GroupSchema, GroupSnapshot, SnapshotV3};
use std::collections::hash_map::Entry;
use tracing::debug;

/// Per-group schema cache for [`create_v3`], keyed by group name
/// (`"<sampler>/<name>"`).
///
/// A cache HIT (this tick's [`GroupSkeleton::identity`] equals last tick's)
/// skips [`GroupSchema`] assembly entirely: no `MetricDesc`, no `BTreeMap`,
/// no formatted member name is built for that group this tick — only its
/// values, which change every tick regardless and must always be read. The
/// cached `Arc<GroupSchema>` is cloned (a refcount bump) and the cached wire
/// `schema_hash` is reused verbatim. A MISS (identity differs, or there is
/// no cached entry yet) assembles the schema exactly as before and pays for
/// a fresh [`GroupSchema::hash`] — the full canonical msgpack encode + fold
/// that scales with schema size.
///
/// # Two hashes, distinct roles
///
/// - **Identity** ([`GroupSkeleton::identity`]) — internal, never
///   transmitted, cheap to fold: per group per tick, for each kind in fixed
///   order (counters, gauges, histograms), for each metric fold its id and
///   the version of its per-entry metadata (`metadata_version`, one atomic
///   load — see `fold_group_version`), then for each member in the same
///   order the schema lists them fold its index — raw integer bytes, no
///   `format!`, no allocation. The version stands in for the metadata
///   itself: metriken bumps it on every mutation, so the labels are never
///   read on a hit tick. They used to be byte-hashed for every member every
///   tick, which was 13–19% of the agent's sampling CPU. 128-bit FNV-1a,
///   same collision reasoning as the wire hash: a collision here would
///   serve a stale schema — the C1 failure class below.
/// - **Wire `schema_hash`** ([`GroupSkeleton::hash`]) — unchanged:
///   [`GroupSchema::hash`], computed only on a miss.
///
/// Deliberately narrower than a full `GroupSchema` fingerprint: identity
/// omits a group's own static metric-level metadata (the `"metric"`/
/// `"sampler"` pair `metric_metadata` derives), which is fixed at
/// registration and never mutates at runtime — `insert_metadata`/
/// `set_metadata` are only ever called on a `CounterGroup`/`GaugeGroup`'s
/// PER-INDEX metadata (a task's `comm`, a cgroup's `name`), never on a
/// `MetricEntry`'s own `metadata()`. That per-index metadata is what the
/// folded version tracks — every such write bumps it inside metriken's
/// store, whoever the writer is — so the C1 case below still forces a miss.
///
/// # Delivered: a hit allocates a small, member-count-independent constant
///
/// This closes the gap an earlier version of this cache (which compared
/// full `GroupSchema` equality — assembling it unconditionally, cache hit
/// or miss, and skipping only the hash) left open: that cache still
/// allocated MORE per tick than V2 on a hit, measured 1.2–3.4× V2's
/// allocations, with the emit-time `cached.schema.clone()` alone 54% of
/// allocations at 2k members. Two upstream changes made the fix possible —
/// both landed on the pinned metriken rev and both are in active use here:
/// the borrowing `with_metadata`/`for_each_metadata` accessors (no full-map
/// clone to decide "did this member's identity change"), and
/// `GroupSnapshot.schema: Option<Arc<GroupSchema>>` (a hit hands out a
/// refcount bump, not a deep clone). See
/// `v3_hit_tick_allocations_are_a_small_constant_not_o_n` for the measured
/// before/after allocation counts on a 512-member fixture group.
///
/// # Why full-schema equality was unsound with names only (still true here)
///
/// An earlier version of this cache (before the identity hash existed)
/// compared member NAME lists only. That's unsound: metriken metadata
/// mutates in place at a stable index (`insert_metadata`/`set_metadata`,
/// e.g. a task's `comm` or a cgroup's `name`), and the kernel recycles PIDs
/// and cgroup ids — so a slot's metadata can change while its
/// `"{metric_id}x{idx}"` name stays byte-for-byte identical. A names-only
/// cache would call that a hit, keep serving the OLD occupant's metadata
/// under an UNCHANGED `schema_hash`, and a receiver caching parsed schemas
/// by `(name, schema_hash)` would bind new values to dead labels
/// indefinitely. The identity fold covers names AND the per-index metadata
/// version for exactly this reason — see
/// `declared_group_schema_reflects_metadata_mutated_at_a_stable_index`, the
/// pinned regression test, which mutates metadata through metriken directly
/// rather than through `SlotIdentity`, and so also pins that the signal
/// lives in the store and not in the agent's own write path.
///
/// # No eviction
///
/// Entries are never removed. Acceptable because the key space is bounded
/// by the number of acquisition groups a build can ever produce (samplers'
/// declared groups plus one default group per sampler plus `external/main`)
/// — a small, essentially fixed set, not something that grows with runtime
/// cardinality (CPUs, tasks, cgroups... those are members WITHIN a group's
/// schema, not distinct group keys).
pub(super) struct SkeletonCache {
    entries: HashMap<String, GroupSkeleton>,
    rebuilds: u64,
}

struct GroupSkeleton {
    /// This tick's cheap membership fingerprint, folded by
    /// [`fold_group_identities`]. Compared against next tick's freshly
    /// folded identity to decide hit vs. miss BEFORE `create_v3`'s
    /// value-collecting walk begins — see the `SkeletonCache` doc comment.
    identity: (u64, u64),
    schema: Arc<GroupSchema>,
    hash: (u64, u64),
}

impl SkeletonCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            rebuilds: 0,
        }
    }

    /// Number of times a group's schema has been rebuilt (hash recomputed)
    /// since this cache was created.
    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> u64 {
        self.rebuilds
    }
}

/// FNV-1a-128 offset basis and prime — the standard constants, same
/// algorithm [`GroupSchema::hash`] uses but a completely separate hash
/// space: this one is an internal cache key that is never transmitted, so
/// nothing requires (or forbids) sharing constants with the wire hash.
const IDENTITY_FNV_OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const IDENTITY_FNV_PRIME: u128 = 0x0000000001000000000000000000013b;

#[inline]
fn identity_fold(mut acc: u128, bytes: &[u8]) -> u128 {
    for &b in bytes {
        acc ^= b as u128;
        acc = acc.wrapping_mul(IDENTITY_FNV_PRIME);
    }
    acc
}

/// Fold a group metric's identity prefix: its registry id and the version of
/// its per-entry metadata.
///
/// The version stands in for the labels themselves. The pre-pass used to
/// byte-hash every entry's label map every tick to notice a slot relabelled
/// at a stable index (a recycled pid, a recreated cgroup); metriken's store
/// now bumps a counter on every mutation, so one atomic load per metric says
/// the same thing, and this fold was 13–19% of the agent's sampling CPU.
///
/// **The version is folded BEFORE the members are read**, here and in the
/// walk that builds the schema. A mutation landing between this read and a
/// member's metadata read then shows as a changed version on the next tick
/// and costs one spurious rebuild. The other order could store a schema built
/// from old metadata under a NEW version, and the pre-pass would call it a
/// hit forever.
#[inline]
fn fold_group_version(acc: u128, metric_id: u64, version: u64) -> u128 {
    let h = identity_fold(acc, &metric_id.to_le_bytes());
    identity_fold(h, &version.to_le_bytes())
}

/// Per-group running identity, one accumulator per kind so the final
/// identity can fold them in the fixed (counters, gauges, histograms) order
/// `GroupSchema` lists members in, regardless of the interleaved order the
/// registry walk actually visits different kinds' registry entries in.
#[derive(Clone, Copy)]
struct GroupIdentityAccum {
    counters: u128,
    gauges: u128,
    histograms: u128,
}

impl Default for GroupIdentityAccum {
    fn default() -> Self {
        Self {
            counters: IDENTITY_FNV_OFFSET,
            gauges: IDENTITY_FNV_OFFSET,
            histograms: IDENTITY_FNV_OFFSET,
        }
    }
}

impl GroupIdentityAccum {
    fn finish(&self) -> (u64, u64) {
        let mut h = IDENTITY_FNV_OFFSET;
        h = identity_fold(h, &self.counters.to_le_bytes());
        h = identity_fold(h, &self.gauges.to_le_bytes());
        h = identity_fold(h, &self.histograms.to_le_bytes());
        ((h >> 64) as u64, h as u64)
    }
}

/// One group's cache decision for this tick, resolved by
/// [`fold_group_identities`] before `create_v3`'s real walk begins.
struct GroupDecision {
    /// `true`: this tick's identity differs from `cache`'s (or there is no
    /// cached entry yet) — `create_v3` must assemble a fresh `GroupSchema`
    /// for this group. `false`: membership (names + per-member metadata) is
    /// byte-identical to last tick's — `create_v3` clones the cached
    /// `Arc<GroupSchema>` instead of touching a single `MetricDesc`.
    ///
    /// The identity value itself isn't carried past this decision: on a
    /// miss, `create_v3` derives the identity it STORES from what its own
    /// walk actually collects (`GroupBuilder::walk_identity`), not from
    /// this pre-pass's read — see the "miss-tick cache poisoning" note on
    /// `fold_group_identities` for why that distinction matters.
    needs_schema: bool,
}

/// First pass over the metriken registry: fold each group's member
/// identity — NOT values, NOT windows, see the `SkeletonCache` doc comment
/// for what "identity" covers and omits — into a cheap running hash without
/// building a single `MetricDesc`, `BTreeMap`, or formatted name. Compares
/// each group's freshly folded identity against `cache`'s last-tick
/// identity to decide, before `create_v3`'s value-collecting walk begins,
/// which groups can skip schema assembly entirely this tick.
///
/// This is a full second walk of `metriken::metrics()` — but the registry
/// itself is small (bounded by declared metrics, not by live cardinality:
/// CPUs/tasks/cgroups are members WITHIN one registry entry, walked here
/// too, but cheaply, with no allocation). What this pass avoids is walking
/// a STABLE group's members while allocating for each one, which is what
/// made the previous full-schema-equality cache cost more per tick than V2
/// even on a hit.
///
/// External metrics are not folded here — see `create_v3`'s external-
/// metrics block, which always assembles a fresh schema for `external/main`
/// (this pass simply never produces a decision for that key, so
/// `create_v3` defaults it to `needs_schema: true`). External metrics are
/// push-ingested and comparatively few, not cardinality-scaled by BPF/task/
/// cgroup population, so they are out of scope for the allocation target
/// this cache exists to hit; always rebuilding keeps their exact prior
/// behavior (and test coverage) untouched.
///
/// # A wider (still narrow, still accepted) torn-read window for default
/// groups
///
/// A DEFAULT (non-declared) group's membership is value-derived (the
/// transitional V2-style sentinel skip — see `create_v3`'s doc comment):
/// whether index `idx` counts as a member depends on reading its CURRENT
/// value, here AND again in `create_v3`'s own walk. Those are two
/// SEPARATE, non-atomic reads of the same counter/gauge, same class as the
/// value/metadata torn-read gap `create_v3`'s CounterGroup arm already
/// documents and accepts (2-3% torn under a deliberate concurrent-recycle
/// hammer, effectively zero in production because sampler writes complete
/// synchronously inside `refresh()` — fully quiesced before `create_v3`
/// ever runs — rather than racing the snapshot builder from another task).
/// This pass widens that window from "within one member's two reads" to
/// "across this whole pre-pass versus the main walk", so a value that
/// crosses the zero/`None` membership boundary in that window can make a
/// default group's identity (this pass) and its actual collected
/// membership (the main walk) disagree for that one tick. The failure mode
/// is bounded and contained at the agent: on a miss the stored identity is
/// folded from what the walk itself collected (so cache entries are always
/// self-consistent), and on a hit the emit site compares per-kind value
/// lengths against the cached schema and, on a mismatch, evicts the entry
/// and skips emitting that group — nothing invalid reaches the wire, and
/// the next tick rebuilds. One residual: the length check is a proxy for
/// identity, so an *equal-arity* membership swap inside the window (one
/// member drops below the sentinel while another crosses above it) would
/// pass it and bind this tick's values to the previous schema's labels.
/// Closing that means re-folding identity on the hit path, which costs the
/// per-member metadata read the hit path exists to avoid; recorded as an
/// accepted trade rather than taken. Not eliminated outright because doing so
/// would mean re-merging the two passes and losing the hit-path allocation
/// win this cache exists for; named here so it's a known, accepted
/// trade-off rather than a surprise a reviewer has to rediscover. Declared
/// groups have no such window — their membership is registration-derived,
/// not value-derived, so nothing about this pass or the main walk needs to
/// agree on a value to agree on membership.
/// The member indices of a declared group-typed metric.
///
/// Two shapes, because two things can be known. A `member_bound` says "the
/// first N indices", which is what a per-CPU sweep over `0..possible_cpus()`
/// has. An explicit set says exactly which indices are populated, which is what
/// a sampler allowed only part of the machine has — and that set is rarely a
/// prefix.
///
/// The distinction is not cosmetic, though what it costs depends on the
/// group's backing. metriken 0.11 gave an OWNED `CounterGroup` a `u64::MAX`
/// unwritten sentinel, so an over-declared prefix there publishes `None` —
/// honest, if noisy: columns that are always null. An EXTERNALLY backed group
/// (a BPF mmap) cannot carry a sentinel, because the kernel zero-fills that
/// memory, so an over-declared prefix still publishes `0` for indices nothing
/// measured — a wrong value where the honest answer is no value at all. Those
/// are exactly the per-CPU and per-cgroup groups a partial reservation
/// under-populates, so declaring the real membership still matters most
/// precisely where it always did.
enum MemberIter<'a> {
    Prefix(std::ops::Range<usize>),
    Set(std::slice::Iter<'a, usize>),
}

impl Iterator for MemberIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        match self {
            MemberIter::Prefix(range) => range.next(),
            MemberIter::Set(iter) => iter.next().copied(),
        }
    }
}

/// Resolve a declared group's members: the explicit set when one was declared,
/// otherwise the dense prefix, clamped to what the backing array actually has.
fn members<'a>(set: Option<&'a [usize]>, bound: Option<usize>, entries: usize) -> MemberIter<'a> {
    match set {
        // An explicit set wins: a caller that knows the exact indices knows
        // strictly more than a bound. Still clamped — a stale set must not walk
        // past the backing array.
        Some(set) => {
            let end = set.partition_point(|idx| *idx < entries);
            MemberIter::Set(set[..end].iter())
        }
        None => MemberIter::Prefix(0..bound.map_or(entries, |b| b.min(entries))),
    }
}

fn fold_group_identities<'a>(
    cache: &SkeletonCache,
    group_registry: &HashMap<(&'static str, &'static str), &'static AcquisitionGroup>,
    sampler_mods: &'a [(&'a str, &'a str)],
) -> HashMap<(&'a str, &'a str), GroupDecision> {
    let mut accums: HashMap<(&'a str, &'a str), GroupIdentityAccum> = HashMap::new();
    let mut idx_scratch: Vec<usize> = Vec::new();

    for (metric_id, metric) in metriken::metrics().iter().enumerate() {
        let Some(value) = metric.value() else {
            continue;
        };

        let name = metric.name();
        if name.starts_with("log_") {
            continue;
        }

        // Routing mirrors create_v3's (KEEP IN SYNC), and mirrors `create`
        // (V2)'s `group_windows` for the SAME reason: `attribute_sampler`
        // returns a borrowed `&str` (tied to `sampler_mods`, which outlives
        // this whole function), and `metric.metadata().get("acq_group")`
        // is a transient borrowed lookup key, used only against
        // `group_registry` below — never stored. Once matched, the group's
        // OWN `&'static str` fields (`ag.sampler`/`ag.name`) become the
        // key that's actually stored/returned, so this pass never
        // `format!`s a `String` at all: not for the metric-level metadata
        // `BTreeMap` (still skipped entirely, as before), and now not for
        // routing either — the ~334-registry-entry `format!("{sampler}/
        // {acq_group}")` this used to pay for every tick is gone.
        let sampler = crate::agent::samplers::attribute_sampler(metric.module(), sampler_mods);
        let mut declared = false;
        let mut member_bound: Option<usize> = None;
        let mut member_set: Option<&[usize]> = None;
        let mut reader_stamped = false;
        let group_key: (&str, &str) = match metric.metadata().get("acq_group") {
            Some(acq_group) => {
                if let Some(ag) = group_registry.get(&(sampler, acq_group)) {
                    declared = true;
                    member_bound = ag.member_bound();
                    member_set = ag.member_set();
                    reader_stamped = ag.is_reader_stamped();
                    (ag.sampler, ag.name)
                } else {
                    (sampler, "main")
                }
            }
            None => (sampler, "main"),
        };

        let accum = accums.entry(group_key).or_default();
        let metric_id = metric_id as u64;

        match value {
            Value::Counter(_) => {
                accum.counters = identity_fold(accum.counters, &metric_id.to_le_bytes());
            }
            Value::Gauge(_) => {
                accum.gauges = identity_fold(accum.gauges, &metric_id.to_le_bytes());
            }
            Value::CounterGroup(g) => {
                // The version FIRST, then membership — see `fold_group_version`.
                accum.counters =
                    fold_group_version(accum.counters, metric_id, g.metadata_version());
                if reader_stamped {
                    idx_scratch.clear();
                    g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                    idx_scratch.sort_unstable();
                    for &idx in idx_scratch.iter() {
                        accum.counters = identity_fold(accum.counters, &(idx as u64).to_le_bytes());
                    }
                } else {
                    for idx in members(member_set, member_bound, g.entries()) {
                        if !declared {
                            let Some(v) = g.counter_value(idx) else {
                                continue;
                            };
                            if v == 0 {
                                continue;
                            }
                        }
                        accum.counters = identity_fold(accum.counters, &(idx as u64).to_le_bytes());
                    }
                }
            }
            Value::GaugeGroup(g) => {
                accum.gauges = fold_group_version(accum.gauges, metric_id, g.metadata_version());
                if reader_stamped {
                    idx_scratch.clear();
                    g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                    idx_scratch.sort_unstable();
                    for &idx in idx_scratch.iter() {
                        accum.gauges = identity_fold(accum.gauges, &(idx as u64).to_le_bytes());
                    }
                } else {
                    for idx in members(member_set, member_bound, g.entries()) {
                        if !declared && g.gauge_value(idx).is_none() {
                            continue;
                        }
                        accum.gauges = identity_fold(accum.gauges, &(idx as u64).to_le_bytes());
                    }
                }
            }
            Value::Histogram(h) if declared || h.load().is_some() => {
                accum.histograms = identity_fold(accum.histograms, &metric_id.to_le_bytes());
            }
            _ => {}
        }
    }

    accums
        .into_iter()
        .map(|(group_key, accum)| {
            let identity = accum.finish();
            // The cache itself is still keyed by the wire-format
            // "{sampler}/{name}" `String` (it persists ACROSS ticks, so it
            // must own its keys regardless) — this `format!` runs once per
            // DISTINCT GROUP here (bounded by declared-group count, ~tens),
            // not once per registry entry, so it's not the cost this pass
            // exists to avoid.
            let cache_key = format!("{}/{}", group_key.0, group_key.1);
            let needs_schema = match cache.entries.get(&cache_key) {
                Some(cached) => cached.identity != identity,
                None => true,
            };
            (group_key, GroupDecision { needs_schema })
        })
        .collect()
}

/// Per-group accumulation while walking the metriken registry: the group's
/// acquisition window (read once, at first touch — see `create_v3`) plus,
/// per kind, this tick's values (always collected) and descriptors (only
/// collected when `needs_schema` — see `GroupDecision`).
///
/// `reader_guard` is only ever `Some` for a
/// [reader-stamped](AcquisitionGroup::is_reader_stamped) group (a
/// `PackedCounters` mmap-direct group): its acquisition IS this walk's read
/// of the group's members, so first touch (the `Entry::Vacant` arm below)
/// acquires the bracket directly instead of reading a window a sampler
/// already stamped, and the group-emit loop `finish()`es it once every
/// member's value has been read — see the doc comment there.
///
/// `walk_identity` is folded ALONGSIDE `counter_descs`/`gauge_descs`/
/// `histogram_descs` on the MISS path only (see `create_v3`'s `if
/// group.needs_schema` arm) — the same `fold_group_version`/`identity_fold`
/// calls `fold_group_identities` makes, applied to what THIS walk actually
/// pushes into the schema rather than to the pre-pass's own read. Finalize
/// stores `walk_identity.finish()`, not the pre-pass's identity, as the
/// cached identity for a rebuilt group — see the "miss-tick cache
/// poisoning" note on `fold_group_identities` for why the two can
/// legitimately differ and why storing the pre-pass's would be wrong. Left
/// at its `Default` (unfolded) on a hit — nothing needs it there, since a
/// hit reuses the cached identity verbatim.
///
/// `Default::default()` sets `needs_schema: true` (a safe "always rebuild"
/// default): the external-metrics block relies on it via
/// `HashMap::or_default`, and every other call site sets `needs_schema`
/// explicitly from `fold_group_identities`'s decision at first touch.
struct GroupBuilder {
    window: Option<Window>,
    reader_guard: Option<AcquisitionGuard<'static>>,
    needs_schema: bool,
    walk_identity: GroupIdentityAccum,
    counter_descs: Vec<MetricDesc>,
    counter_values: Vec<Option<u64>>,
    gauge_descs: Vec<MetricDesc>,
    gauge_values: Vec<Option<i64>>,
    histogram_descs: Vec<MetricDesc>,
    histogram_values: Vec<Option<histogram::Histogram>>,
}

impl Default for GroupBuilder {
    fn default() -> Self {
        Self {
            window: None,
            reader_guard: None,
            needs_schema: true,
            walk_identity: GroupIdentityAccum::default(),
            counter_descs: Vec::new(),
            counter_values: Vec::new(),
            gauge_descs: Vec::new(),
            gauge_values: Vec::new(),
            histogram_descs: Vec::new(),
            histogram_values: Vec::new(),
        }
    }
}

/// Reconcile a declared group's window across one `create_v3` walk.
///
/// `first` is read at the group's first touch, before any of its members'
/// values are read (see the `Entry::Vacant` arm in `create_v3`). For a
/// group with few members or a fast walk that's also the window this
/// function emits with. But the walk over the FULL registry — every
/// group, every member — has measured as long as several milliseconds
/// (mean 1.85ms, max 5.7ms observed span), which is long enough for an
/// async sampler write to complete a whole new `acquire()`/`finish()`
/// cycle in the middle of it. `latest` is a second read of the same
/// group's window, taken at emit time after all of that group's values
/// have been read. If `first` and `latest` differ, some of the values
/// just read may actually be newer than `first` claims — the window can
/// only ever LAG the true acquisition time (`AcquisitionGuard` stamps
/// last), never lead it, so `first` alone would UNDER-claim what this
/// walk covers. The honest fix is the union: `first.begin_ns` is still
/// correct (nothing read during the walk is older than that), so keep it,
/// and extend the end to `latest.end_ns` to honestly bracket every value
/// this walk actually read, rather than silently narrowing the claimed
/// window to only the pre-mid-walk-stamp subset.
///
/// `first: None, latest: Some(_)` means the group was stamped for the
/// first time during this very walk (unstamped at first touch, stamped by
/// the time of emit) — there's nothing to union with, so `latest` alone is
/// the walk's window. `first: Some(_), latest: None` is not expected in
/// practice (a group's seqlock only reads `None` before its first-ever
/// stamp, and stamps never revert to unstamped outside the seqlock's
/// documented u64-wraps-to-exactly-0 edge case) — `first` is kept rather
/// than discarding a real reading for a transient artifact.
fn resolve_walk_window(first: Option<Window>, latest: Option<Window>) -> Option<Window> {
    match (first, latest) {
        (Some(f), Some(l)) if f == l => Some(f),
        (Some(f), Some(l)) => Some(Window::new(f.begin_ns, l.end_ns)),
        (None, Some(l)) => Some(l),
        (Some(f), None) => Some(f),
        (None, None) => None,
    }
}

/// Build a `SnapshotV3` (acquisition-group snapshot) from the current
/// metriken registry, mirroring `create`'s walk/naming/metadata rules with
/// one addition: routing each entry into an acquisition group.
///
/// # Routing
///
/// A metric whose own metadata carries `acq_group = "<name>"` routes to the
/// declared group `"<sampler>/<name>"`, provided `(sampler, name)` is
/// actually registered on [`crate::agent::samplers::ACQUISITION_GROUPS`].
/// Everything else — the overwhelming majority of metrics today, since no
/// sampler has migrated yet — falls into that sampler's default group,
/// `"<sampler>/main"`.
///
/// A metric that names an `acq_group` with no matching registry entry is a
/// migration bug (a typo, or a group that was renamed on one side and not
/// the other): it is routed to the default group so the tick still produces
/// a valid snapshot, but `debug_assert!` catches it in tests/debug builds
/// rather than letting it pass silently in release. Note what that means
/// operationally: on a debug build this panics the scrape task for that one
/// tick (`/metrics/binary`'s handler task, not the sampler tasks) — the
/// `tokio::sync::Mutex` guarding `SnapshotBuilder` does not poison on a
/// panicked holder, so the next scrape simply retries and calls `refresh()`
/// again rather than the agent wedging.
///
/// The mirror case — an `AcquisitionGroup` IS registered but no metric ever
/// names it via `acq_group` — is not an error at all (e.g. a group declared
/// ahead of the sampler code that will use it). `create_v3` only creates a
/// `GroupBuilder` when some metric actually routes to a group, so a
/// registered-but-unused group is silently absent from the emitted
/// snapshot's `groups` list entirely, rather than appearing as an empty
/// `GroupSnapshot`. Pinned by
/// `registered_group_with_no_routed_metrics_is_absent_from_the_snapshot`.
///
/// # Default vs. declared group semantics
///
/// Default groups keep V2's transitional membership semantics: a
/// `CounterGroup` entry reading exactly `0`, or a `GaugeGroup` entry reading
/// exactly `i64::MIN`, is treated as "not really there" and skipped, exactly
/// as `create` does today. This is a known compromise carried over
/// unchanged from V2 — real zero/never-set values are indistinguishable —
/// and it is deliberately kept ONLY here, in the pre-migration default
/// groups, so flipping the wire format to V3 does not by itself explode
/// cardinality with a flood of phantom dense slots (every possible CPU,
/// device, or task index some sampler could ever report on, most of them
/// unpopulated). Default groups also carry no acquisition window
/// (`window: None`): V2 attached a window to every individual metric, but a
/// default group has no registered acquisition boundary to report — that
/// returns metric-by-metric once each sampler declares real groups.
///
/// Declared groups use registration membership instead: every entry in
/// `0..group.entries()` is a member, full stop — UNLESS the group has a
/// member-population bound set ([`AcquisitionGroup::set_member_bound`]), in
/// which case membership is `0..bound.min(group.entries())`. Registration
/// membership for a per-CPU group like a `CpuCounters`-backed one IS its
/// possible-CPU population; the backing array's `entries()` is an
/// implementation ceiling sized for the worst case (`MAX_CPUS`; see
/// docs/principles.md principle 6 and
/// docs/superpowers/plans/2026-08-18-stage3c-wave1-sampler-migration.md),
/// not a claim that every one of those slots is a real member on this host.
/// Counter/gauge group entries within the bound send `Some(value)`
/// including an honest zero — no sentinel skip — and a group whose backing
/// store was never written at all reports `None` for every member
/// ("registered but no reading yet"), never fabricating a value or
/// silently dropping the member. A scalar (non-group) declared metric
/// behaves differently: a `LazyCounter`/`LazyGauge` reports no value at all
/// (`metric.value()` is `None`) until its first `set()`/`increment()`, so
/// it is simply ABSENT from the group's schema — not present with a `None`
/// value — until then; that first appearance is one schema rebuild
/// (`SkeletonCache` absorbs it like any other schema change) rather than an
/// ongoing cost. The group's window comes from the
/// registered [`AcquisitionGroup`]'s own window slot, not from any
/// per-metric window.
///
/// External metrics land in a single windowless `"external/main"` group;
/// their own per-metric windows are intentionally dropped here (a
/// `GroupSnapshot` carries one window for the whole group) and will return
/// once external sources get real declared groups of their own.
///
/// # A third membership mode: reader-stamped (metadata-presence)
///
/// [`AcquisitionGroup::is_reader_stamped`] groups (mmap-direct
/// `PackedCounters` — cgroup and task counters) use neither the unbounded
/// `0..entries()` walk nor `member_bound`'s dense prefix. Membership is
/// `load_metadata(idx).is_some()`: an index the sampler's ringbuf handler
/// has registered metadata for (a live cgroup, a live task) is a member,
/// walked via `metadata_snapshot()` rather than a `0..N` loop — see the
/// CounterGroup/GaugeGroup match arms below.
///
/// **Walk-cost grounding** (docs/superpowers/plans/2026-08-19-stage3c-wave2.md
/// Part A asks this explicitly): does today's declared-group walk already
/// scan `0..entries()` for these groups, and is there a metriken iterator
/// over populated entries rather than backing-array capacity? Checked
/// against the pinned metriken rev
/// (`f601f48cffcfe27d2acc835bf05c90d0e481d1f7`, `metriken/src/group/{counter,metadata}.rs`):
/// yes to both. Before this mode existed, a packed/sparse `CounterGroup`
/// declared metric with no `member_bound` fell through to the unbounded
/// branch above — `0..g.entries()`, i.e. all 4,194,304 `MAX_PID` slots for
/// `task_cpu_usage`, every tick, in the V3 walk (V2's `create()` still does
/// this — see its own doc comment). `metadata_snapshot()`
/// (`CounterGroupMetric`/`GaugeGroupMetric` trait method, implemented by
/// `GroupMetadata::snapshot()`) is backed by a
/// `parking_lot::RwLock<HashMap<usize, HashMap<String, String>>>` holding
/// only populated indices — its cost is O(live population), the same
/// asymptotic class as `member_bound`'s dense-prefix walk, not a regression
/// against it. It IS a regression against the near-zero cost of an empty
/// group (a fresh `RwLock<HashMap>` clone-and-collect even at zero
/// population isn't free), but that trades a bounded, population-scaled
/// cost for what was previously an unconditional 4.2M-iteration sweep — a
/// net win, not a wash.
pub(super) fn create_v3(
    timestamp: SystemTime,
    duration: Duration,
    external_metrics: Vec<ExternalMetric>,
    cache: &mut SkeletonCache,
    stamp: (i64, i64),
) -> Snapshot {
    // See BUILDER_TEST_LOCK: serialise builder calls under `cargo test` so the
    // shared reader-stamped slots keep their single-writer invariant.
    #[cfg(test)]
    let _serialize = BUILDER_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let sampler_mods = crate::agent::samplers::sampler_modules();
    let group_registry = group_registry();

    // Pre-pass: decide, per group, hit or miss — see `fold_group_identities`
    // and the `SkeletonCache` doc comment. Everything below this point
    // either assembles a schema (miss) or skips straight to values (hit)
    // based on this map; it never re-derives the decision itself.
    let group_decisions = fold_group_identities(cache, group_registry, &sampler_mods);

    let mut groups: HashMap<(&str, &str), GroupBuilder> = HashMap::new();
    // Reused across every reader-stamped group's sparse membership walk
    // below (both the schema-building and values-only arms) — cleared
    // per group, never reallocated once it reaches its high-water mark, so
    // this one scratch buffer is the only heap cost the sparse-membership
    // walk pays across the whole tick, not one allocation per group.
    let mut idx_scratch: Vec<usize> = Vec::new();

    for (metric_id, metric) in metriken::metrics().iter().enumerate() {
        let Some(value) = metric.value() else {
            continue;
        };

        let name = metric.name();

        if name.starts_with("log_") {
            continue;
        }

        // Route: a declared `acq_group` wins only if it actually resolves
        // against the registry; otherwise fall back to the sampler's
        // default group (and flag the mismatch in debug builds — see the
        // function-level doc comment). Mirrors `fold_group_identities`'
        // routing (KEEP IN SYNC) — deliberately NOT calling `metric_metadata`
        // here: that builds a `BTreeMap` this walk may not need at all (a
        // cache hit needs no metadata whatsoever), so it's deferred below,
        // behind the `needs_schema` check. Also mirrors `create` (V2)'s
        // `group_windows` lookup: `(&str, &str)` tuple keys throughout, not
        // a `format!`ed `String` per registry entry — see
        // `fold_group_identities`'s matching comment for the full
        // rationale.
        let sampler = crate::agent::samplers::attribute_sampler(metric.module(), &sampler_mods);
        let mut declared = false;
        // The member-population bound for a declared, group-typed metric
        // (`CounterGroup`/`GaugeGroup`): `Some(n)` walks `0..n` instead of
        // the full backing-array `entries()`. Resolved alongside routing,
        // before `group_key` is moved into `groups.entry` below.
        let mut member_bound: Option<usize> = None;
        // Members that are not a dense prefix; see `members`.
        let mut member_set: Option<&[usize]> = None;
        // Reader-stamped (`PackedCounters` mmap-direct) groups use a THIRD
        // membership mode instead of `member_bound`'s dense prefix: every
        // index with metadata registered is a member — see the
        // CounterGroup/GaugeGroup arms below and the doc comment on
        // `create_v3` for the walk-cost grounding.
        let mut reader_stamped = false;
        let group_key: (&str, &str) = match metric.metadata().get("acq_group") {
            Some(acq_group) => {
                if let Some(ag) = group_registry.get(&(sampler, acq_group)) {
                    declared = true;
                    member_bound = ag.member_bound();
                    member_set = ag.member_set();
                    reader_stamped = ag.is_reader_stamped();
                    (ag.sampler, ag.name)
                } else {
                    debug_assert!(
                        false,
                        "metric `{name}` declares acq_group=\"{acq_group}\" for sampler \
                         `{sampler}`, but no AcquisitionGroup (\"{sampler}\", \
                         \"{acq_group}\") is registered on ACQUISITION_GROUPS; routing to \
                         the default group instead",
                    );
                    (sampler, "main")
                }
            }
            None => (sampler, "main"),
        };

        let group = match groups.entry(group_key) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                // First touch for this group THIS TICK.
                //
                // Reader-stamped groups (`PackedCounters` mmap-direct — see
                // `AcquisitionGroup::is_reader_stamped`): this walk's read of
                // the group's members IS the acquisition, so acquire the
                // bracket right here, at first touch, instead of reading a
                // window some sampler already stamped. There is nothing to
                // read yet — `window` stays `None` until the group-emit loop
                // below calls `guard.finish()` to PUBLISH. The published
                // WIDTH, though, is decided earlier: the CounterGroup/
                // GaugeGroup arms call `guard.mark_end()` immediately after
                // each touching metric's member-value loop, so the window's
                // end reflects when this group's own values were actually
                // read (µs-scale), not when `finish()` happens to run after
                // the rest of the tick's walk — see
                // `AcquisitionGuard::mark_end`. Unlike the sampler-stamped
                // case, there is no racing writer to guard against: the
                // ONLY writer for a reader-stamped group's slot is this very
                // walk (see `AcquisitionGroup::set_reader_stamped`'s doc
                // comment on the single-writer contract), so this
                // acquire()/mark_end()/finish() sequence is honest, not an
                // approximation.
                //
                // Sampler-stamped groups keep the original discipline: read
                // the window now, before any of this group's members'
                // values accumulate below — not once per member (redundant
                // seqlock loads, and walk-order-dependent) and not deferred
                // solely to emit time (see `resolve_walk_window`) (unsafe:
                // the walk over the FULL registry, across every group, can
                // take long enough that a concurrent sampler tick completes
                // a whole new acquire()/finish() cycle in the meantime,
                // which would pair window(N+1) with the values(N) already
                // read here — a confident, wrong claim that this data is
                // newer than it actually is). Read before values, mirroring
                // timing.rs's stamp-last rule from the read side: a stale
                // window paired with fresh-enough values only under-claims
                // freshness, which is the safe direction — same "can only
                // lag, never lead" guarantee `AcquisitionGuard` gives
                // writers, applied here to the reader.
                let ag = group_registry.get(e.key());
                let (window, reader_guard) = match ag {
                    Some(ag) if ag.is_reader_stamped() => (None, Some(ag.acquire())),
                    Some(ag) => (ag.window(), None),
                    None => (None, None),
                };

                let needs_schema = group_decisions
                    .get(e.key())
                    .map(|d| d.needs_schema)
                    .unwrap_or(true);

                let mut builder = GroupBuilder {
                    window,
                    reader_guard,
                    needs_schema,
                    ..Default::default()
                };

                // Cache hit: pre-size this tick's value vectors from the
                // cached schema's member counts, so pushing values below
                // never reallocates. The only heap allocations a hit group
                // pays for are these — at most three `Vec::with_capacity`
                // calls, one per kind, not one per member. `cache.entries`
                // is String-keyed (it persists across ticks, so it must own
                // its keys) — this `format!` runs once per DISTINCT GROUP
                // at first touch, not once per registry entry, so it isn't
                // the cost `fold_group_identities`'s routing avoids.
                if !needs_schema {
                    let (sampler, name) = *e.key();
                    if let Some(cached) = cache.entries.get(&format!("{sampler}/{name}")) {
                        builder.counter_values = Vec::with_capacity(cached.schema.counters.len());
                        builder.gauge_values = Vec::with_capacity(cached.schema.gauges.len());
                        builder.histogram_values =
                            Vec::with_capacity(cached.schema.histograms.len());
                    }
                }

                e.insert(builder)
            }
        };

        if group.needs_schema {
            // MISS path: assemble the schema exactly as before — full
            // metadata, formatted member names, everything a receiver needs
            // to parse this group's values.
            //
            // KEEP IN SYNC with create AND fold_group_identities — see the
            // doc comment on `metric_metadata` (the shared prefix) and
            // below (the per-kind walk/naming/membership rules, which are
            // NOT shared and are duplicated independently in each of these
            // three places).
            let (mut metadata, _) = metric_metadata(metric, &sampler_mods);

            // Strip unconditionally, not just when `declared`. On the
            // declared path it is redundant with the group's own name
            // (`GroupSnapshot.name`, `"{sampler}/{acq_group}"`) — left in
            // place it would be one more copy of the same key/value pair
            // repeated in every member's `MetricDesc.metadata`, thousands of
            // identical wasted copies for a large declared group (tasks/
            // cgroups/CPUs). On the unmatched-registry fallback path (the
            // `debug_assert!` arm above — a typo'd or renamed group), the
            // metric still carries its stale `acq_group` value even though
            // it just got routed to the DEFAULT group instead; leaving it in
            // would leak that value as a phantom label on release builds,
            // where the `debug_assert!` compiles away and this fallback runs
            // silently instead of panicking. A metric with no `acq_group`
            // tag at all has nothing to remove, so this is a no-op there.
            metadata.remove("acq_group");

            let entry_name = format!("{metric_id}");
            // Reused below wherever a member's identity is folded into
            // `group.walk_identity` — see fix (a) on the `SkeletonCache`
            // doc comment / `fold_group_identities`'s "miss-tick cache
            // poisoning" note for why this walk folds its OWN identity
            // instead of trusting the pre-pass's.
            let metric_id_u64 = metric_id as u64;

            match value {
                Value::Counter(v) => {
                    group.walk_identity.counters =
                        identity_fold(group.walk_identity.counters, &metric_id_u64.to_le_bytes());
                    group.counter_descs.push(MetricDesc {
                        name: entry_name,
                        metadata,
                    });
                    group.counter_values.push(Some(v));
                }
                Value::Gauge(v) => {
                    group.walk_identity.gauges =
                        identity_fold(group.walk_identity.gauges, &metric_id_u64.to_le_bytes());
                    group.gauge_descs.push(MetricDesc {
                        name: entry_name,
                        metadata,
                    });
                    group.gauge_values.push(Some(v));
                }
                Value::CounterGroup(g) => {
                    // The version first, then the members — see `fold_group_version`.
                    group.walk_identity.counters = fold_group_version(
                        group.walk_identity.counters,
                        metric_id_u64,
                        g.metadata_version(),
                    );
                    if reader_stamped {
                        // Reader-stamped (mmap-direct `PackedCounters`)
                        // group: membership is metadata-presence, not
                        // `0..bound` — a registered index (a live cgroup/
                        // task the sampler's ringbuf handler attached
                        // metadata to) is a member, full stop, regardless of
                        // the backing array's capacity (`MAX_CGROUPS`/
                        // `MAX_PID`, sized for the worst case — see
                        // docs/principles.md principle 6). Walking
                        // `0..entries()` here would mean sweeping all 4.2M
                        // `MAX_PID` slots every tick for `task_cpu_usage`
                        // regardless of how many tasks are actually live;
                        // see the walk-cost grounding on `create_v3`'s doc
                        // comment. `for_each_metadata`'s cost is
                        // O(populated), not O(entries()) — it walks
                        // metriken's own sparse `HashMap<usize, _>` metadata
                        // store, not the dense value array, and — unlike
                        // `metadata_snapshot()` — borrows each entry instead
                        // of cloning it.
                        //
                        // Iteration order is NOT stable tick-to-tick on its
                        // own (hashbrown gives no ordering guarantee) even
                        // when the populated set is unchanged — collect
                        // indices only (no metadata read yet) and sort so a
                        // stable member set produces a byte-stable schema
                        // order (and therefore an identity match) across
                        // ticks, the same determinism concern the external-
                        // metrics sort below addresses for a different
                        // source.
                        idx_scratch.clear();
                        g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                        idx_scratch.sort_unstable();
                        for &idx in idx_scratch.iter() {
                            // Same torn-recycle caveat as the non-reader-
                            // stamped arm below — see its comment: the value
                            // read and the metadata read are not atomic.
                            let v = g.counter_value(idx);
                            let idx64 = idx as u64;
                            // Fold this member's identity from what THIS
                            // walk observed — the same fold, in the same
                            // order, as fold_group_identities' matching arm,
                            // so a later tick's pre-pass can reproduce this
                            // exact value on a genuine hit. Membership only:
                            // the labels are covered by the version folded
                            // at the top of this arm.
                            group.walk_identity.counters =
                                identity_fold(group.walk_identity.counters, &idx64.to_le_bytes());
                            g.with_metadata(idx, &mut |m| {
                                let mut entry_metadata = metadata.clone();
                                entry_metadata.insert("id".to_string(), idx.to_string());
                                if let Some(m) = m {
                                    for (k, v) in m {
                                        entry_metadata.insert(k.clone(), v.clone());
                                    }
                                }
                                group.counter_descs.push(MetricDesc {
                                    name: format!("{metric_id}x{idx}"),
                                    metadata: entry_metadata,
                                });
                            });
                            group.counter_values.push(v);
                        }
                        // Mark the end HERE — right after this metric's
                        // member values were actually read — not at emit
                        // time, when `finish()` runs below after the rest of
                        // the walk (every other group's schema assembly,
                        // hashing, etc.) has also happened. See
                        // `AcquisitionGuard::mark_end`. A group with several
                        // like-entity members (e.g. `cgroup_syscall`'s 16
                        // op-class maps) touches this arm once per member;
                        // each call moves the mark forward, so the LAST
                        // touch — this group's true last member read — is
                        // what ends up published, exactly like `finish()`'s
                        // original stamp-last derivation, just decoupled
                        // from publish timing.
                        if let Some(guard) = group.reader_guard.as_mut() {
                            guard.mark_end();
                        }
                    } else {
                        // Registration membership for a per-CPU (or similar)
                        // group IS the group's real member population —
                        // `possible_cpus()` for a `CpuCounters`-backed group
                        // — not the backing array's `entries()` capacity,
                        // which is a fixed implementation ceiling
                        // (`MAX_CPUS`; see docs/principles.md principle 6,
                        // "over-allocates on small machines") sized for the
                        // worst case, not this host. Walking the full
                        // capacity on every declared group would put an
                        // ~18-CPU host's tick at ~19× the entries it
                        // actually populated; walk the bound instead when
                        // one is set (clamped to `entries()` in case a
                        // stale/misconfigured bound somehow exceeds the
                        // backing array).
                        for idx in members(member_set, member_bound, g.entries()) {
                            let v = g.counter_value(idx);

                            // Transitional V2-style sentinel skip — default
                            // groups only. See doc comment.
                            if !declared {
                                let Some(v) = v else { continue };
                                if v == 0 {
                                    continue;
                                }
                            }

                            // `counter_value(idx)` above and the metadata
                            // read below are two SEPARATE reads, not one
                            // atomic pair — unlike `AcquisitionGroup`'s
                            // window (a seqlock), a group entry's value and
                            // its metadata have no shared lock. A slot
                            // recycled by a concurrent writer between these
                            // two reads (e.g. a pid/cgroup id reused
                            // mid-tick) can pair the NEW occupant's value
                            // with the OLD occupant's labels, or vice versa,
                            // for that one tick. This matches V2's
                            // `create()`, which has the identical two-step
                            // read here — not a regression introduced by
                            // V3. Measured under a deliberate
                            // concurrent-recycle hammer: ~2-3% of ticks
                            // torn; in production today it's effectively
                            // zero, because sampler writes complete
                            // synchronously inside `refresh()` rather than
                            // racing the snapshot builder from another task.
                            // Migration note: a sampler that calls
                            // `insert_metadata` more than once per slot per
                            // refresh (cpu usage does 4) should move to a
                            // single atomic metadata update (`set_metadata`,
                            // one call) when it migrates to a declared
                            // group, to close this window rather than just
                            // narrow it.
                            let mut entry_metadata = metadata.clone();
                            entry_metadata.insert("id".to_string(), idx.to_string());
                            let idx64 = idx as u64;
                            // See the reader-stamped arm above.
                            group.walk_identity.counters =
                                identity_fold(group.walk_identity.counters, &idx64.to_le_bytes());
                            g.with_metadata(idx, &mut |m| {
                                if let Some(m) = m {
                                    for (k, v) in m {
                                        entry_metadata.insert(k.clone(), v.clone());
                                    }
                                }
                            });

                            group.counter_descs.push(MetricDesc {
                                name: format!("{metric_id}x{idx}"),
                                metadata: entry_metadata,
                            });
                            group.counter_values.push(v);
                        }
                    }
                }
                Value::GaugeGroup(g) => {
                    // The version first, then the members — see `fold_group_version`.
                    group.walk_identity.gauges = fold_group_version(
                        group.walk_identity.gauges,
                        metric_id_u64,
                        g.metadata_version(),
                    );
                    if reader_stamped {
                        // See the identical branch on the CounterGroup arm
                        // above for the full rationale (walk-cost grounding,
                        // and why the sort is required for schema
                        // stability). No `PackedCounters`-style gauge group
                        // exists in the codebase yet, but this keeps the
                        // declared-group membership rule symmetric across
                        // both group kinds rather than leaving a silent gap
                        // for the first one that does.
                        idx_scratch.clear();
                        g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                        idx_scratch.sort_unstable();
                        for &idx in idx_scratch.iter() {
                            let v = g.gauge_value(idx);
                            let idx64 = idx as u64;
                            group.walk_identity.gauges =
                                identity_fold(group.walk_identity.gauges, &idx64.to_le_bytes());
                            g.with_metadata(idx, &mut |m| {
                                let mut entry_metadata = metadata.clone();
                                entry_metadata.insert("id".to_string(), idx.to_string());
                                if let Some(m) = m {
                                    for (k, v) in m {
                                        entry_metadata.insert(k.clone(), v.clone());
                                    }
                                }
                                group.gauge_descs.push(MetricDesc {
                                    name: format!("{metric_id}x{idx}"),
                                    metadata: entry_metadata,
                                });
                            });
                            group.gauge_values.push(v);
                        }
                        // See the CounterGroup arm above: mark the end right
                        // after THIS metric's member values were read, not
                        // at emit time.
                        if let Some(guard) = group.reader_guard.as_mut() {
                            guard.mark_end();
                        }
                    } else {
                        // Same member-population bound as the `CounterGroup`
                        // arm above — see its comment.
                        for idx in members(member_set, member_bound, g.entries()) {
                            let v = g.gauge_value(idx);

                            // Transitional V2-style sentinel skip — default
                            // groups only. See doc comment. Unlike
                            // CounterGroup's `== 0` (still live above: 0 is
                            // a legitimate initialized-but-untouched counter
                            // value, indistinguishable from an explicit 0),
                            // there is no `== i64::MIN` check here:
                            // `GaugeGroup::gauge_value` already maps its
                            // internal never-set sentinel to `None` before
                            // this ever sees it (metriken owns that
                            // mapping), so `Some(i64::MIN)` cannot occur —
                            // an explicit re-check here would be dead code.
                            if !declared && v.is_none() {
                                continue;
                            }

                            // Separate value/metadata reads, same
                            // torn-recycle caveat as the CounterGroup arm
                            // above.
                            let mut entry_metadata = metadata.clone();
                            entry_metadata.insert("id".to_string(), idx.to_string());
                            let idx64 = idx as u64;
                            group.walk_identity.gauges =
                                identity_fold(group.walk_identity.gauges, &idx64.to_le_bytes());
                            g.with_metadata(idx, &mut |m| {
                                if let Some(m) = m {
                                    for (k, v) in m {
                                        entry_metadata.insert(k.clone(), v.clone());
                                    }
                                }
                            });

                            group.gauge_descs.push(MetricDesc {
                                name: format!("{metric_id}x{idx}"),
                                metadata: entry_metadata,
                            });
                            group.gauge_values.push(v);
                        }
                    }
                }
                Value::Histogram(h) => {
                    // `config()` doesn't require a loaded value, so this is
                    // always available regardless of what `load()` returns
                    // below.
                    let mut entry_metadata = metadata;
                    entry_metadata.insert(
                        "grouping_power".to_string(),
                        h.config().grouping_power().to_string(),
                    );
                    entry_metadata.insert(
                        "max_value_power".to_string(),
                        h.config().max_value_power().to_string(),
                    );

                    let hv = h.load();
                    let desc = MetricDesc {
                        name: entry_name,
                        metadata: entry_metadata,
                    };
                    // Same membership test as fold_group_identities'
                    // matching arm (declared || h.load().is_some()) — fold
                    // only when this member is actually about to be pushed
                    // below.
                    if declared || hv.is_some() {
                        group.walk_identity.histograms = identity_fold(
                            group.walk_identity.histograms,
                            &metric_id_u64.to_le_bytes(),
                        );
                    }
                    if declared {
                        // Registration membership: this metric IS the
                        // member, full stop — `None` means "registered but
                        // no reading yet" (e.g. before its BPF map
                        // attaches), not "not a member". Omitting it here
                        // would make membership value-derived on the
                        // declared path, churning the schema hash on
                        // exactly the transient event (a histogram that
                        // hasn't loaded yet) the design commits to NOT
                        // treating as a membership change.
                        group.histogram_descs.push(desc);
                        group.histogram_values.push(hv);
                    } else if let Some(hv) = hv {
                        // Default path: unchanged V2-style membership-by-
                        // presence — an unloaded histogram isn't a member at
                        // all.
                        group.histogram_descs.push(desc);
                        group.histogram_values.push(Some(hv));
                    }
                }
                _ => {}
            }
        } else {
            // HIT path: identity unchanged since last tick (see
            // `fold_group_identities`) — read this tick's values, in the
            // SAME order and under the SAME membership rules the schema
            // arms above use, but build no `MetricDesc`, no `BTreeMap`, no
            // formatted member name. The cached `Arc<GroupSchema>` is
            // reused verbatim at emit time below.
            match value {
                Value::Counter(v) => group.counter_values.push(Some(v)),
                Value::Gauge(v) => group.gauge_values.push(Some(v)),
                Value::CounterGroup(g) => {
                    if reader_stamped {
                        idx_scratch.clear();
                        g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                        idx_scratch.sort_unstable();
                        for &idx in idx_scratch.iter() {
                            group.counter_values.push(g.counter_value(idx));
                        }
                        if let Some(guard) = group.reader_guard.as_mut() {
                            guard.mark_end();
                        }
                    } else {
                        for idx in members(member_set, member_bound, g.entries()) {
                            let v = g.counter_value(idx);
                            if !declared {
                                let Some(v) = v else { continue };
                                if v == 0 {
                                    continue;
                                }
                            }
                            group.counter_values.push(v);
                        }
                    }
                }
                Value::GaugeGroup(g) => {
                    if reader_stamped {
                        idx_scratch.clear();
                        g.for_each_metadata(&mut |idx, _| idx_scratch.push(idx));
                        idx_scratch.sort_unstable();
                        for &idx in idx_scratch.iter() {
                            group.gauge_values.push(g.gauge_value(idx));
                        }
                        if let Some(guard) = group.reader_guard.as_mut() {
                            guard.mark_end();
                        }
                    } else {
                        for idx in members(member_set, member_bound, g.entries()) {
                            let v = g.gauge_value(idx);
                            if !declared && v.is_none() {
                                continue;
                            }
                            group.gauge_values.push(v);
                        }
                    }
                }
                Value::Histogram(h) => {
                    let hv = h.load();
                    if declared || hv.is_some() {
                        group.histogram_values.push(hv);
                    }
                }
                _ => {}
            }
        }
    }

    // External metrics: one windowless group, own naming scheme (they are
    // not metriken registry entries, so there is no metric_id to key on).
    // Always schema-built — see `fold_group_identities`'s doc comment for
    // why the identity fold skips this group (and so `GroupBuilder::default`
    // always has `needs_schema: true`, matched here unconditionally).
    if !external_metrics.is_empty() {
        let group = groups.entry(("external", "main")).or_default();
        debug_assert!(
            group.needs_schema,
            "\"external/main\" must always be a fresh GroupBuilder::default() (needs_schema: \
             true) — fold_group_identities never produces a decision for it, so if this ever \
             fires, something inserted an entry ahead of this block and left needs_schema \
             false, which would wrongly skip building its schema below",
        );

        // `get_active()`'s Vec order follows the store's `HashMap<MetricKey,
        // _>` iteration order, which is not a stable contract tick-to-tick
        // (hashbrown makes no ordering guarantee, independent of any TTL
        // eviction/insertion churn). Sort by the identity-derived name
        // assigned below so the group's member order — and therefore the
        // skeleton cache's schema comparison — is deterministic
        // regardless of store iteration order; otherwise the cache would
        // spuriously "miss" (and rebuild/rehash) on every tick whenever the
        // store happened to reorder with no real membership change.
        let mut entries: Vec<(String, ExternalMetric)> = external_metrics
            .into_iter()
            .map(|metric| {
                // A positional name (e.g. `external{i}`) is not a stable
                // identity: both membership and Vec order can churn
                // tick-to-tick, so the same metric would silently reattach
                // under a different name — a name-keyed consumer would read
                // that as a valid continuous series when it isn't one. Name
                // alone isn't sufficient either, since two external metrics
                // may share a name with different labels. Derive the entry
                // name from (name, labels) via the same hash the store's own
                // `MetricKey` uses for identity (`hash_labels`: sorted-key
                // `DefaultHasher`, not process-randomized — verified: three
                // separate process runs over the same label set produced the
                // identical hash), so a metric's values reattach under the
                // same name every tick regardless of where it lands in the
                // store.
                let labels_hash = MetricKey::new(&metric.name, &metric.labels).labels_hash;
                let entry_name = format!("external/{}#{labels_hash:016x}", metric.name);
                (entry_name, metric)
            })
            .collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (entry_name, metric) in entries {
            let mut metadata: BTreeMap<String, String> = [
                ("metric".to_string(), metric.name.clone()),
                ("source".to_string(), "external".to_string()),
            ]
            .into();

            for (k, v) in metric.labels {
                metadata.insert(k, v);
            }

            match metric.value {
                ExternalMetricValue::Counter(v) => {
                    group.counter_descs.push(MetricDesc {
                        name: entry_name,
                        metadata,
                    });
                    group.counter_values.push(Some(v));
                }
                ExternalMetricValue::Gauge(v) => {
                    group.gauge_descs.push(MetricDesc {
                        name: entry_name,
                        metadata,
                    });
                    group.gauge_values.push(Some(v));
                }
                ExternalMetricValue::Histogram {
                    grouping_power,
                    max_value_power,
                    buckets,
                } => {
                    if let Ok(hv) =
                        histogram::Histogram::from_buckets(grouping_power, max_value_power, buckets)
                    {
                        metadata.insert("grouping_power".to_string(), grouping_power.to_string());
                        metadata.insert("max_value_power".to_string(), max_value_power.to_string());
                        group.histogram_descs.push(MetricDesc {
                            name: entry_name,
                            metadata,
                        });
                        group.histogram_values.push(Some(hv));
                    }
                }
            }
        }
    }

    let mut group_snapshots: Vec<GroupSnapshot> = Vec::with_capacity(groups.len());

    for (group_key, group) in groups {
        // A metric routes to (and so creates) a `GroupBuilder` before its
        // `Value` is matched above, so a metric whose value kind isn't one
        // `create_v3` knows how to expose (falls into the `_ => {}` arm —
        // e.g. a `HistogramGroup`-typed metric, a gap V2's `create` shares)
        // can leave a group with nothing ever pushed into it. An
        // empty-schema `GroupSnapshot` carries no information a receiver
        // can use and would otherwise be hashed and transmitted every
        // tick for nothing — and it contradicts this function's own doc
        // comment, which says a group nothing routes to is absent. Skip it
        // entirely rather than emit a zero-member group. Checked against
        // the VALUE vectors, not the desc vectors — a hit group's desc
        // vectors are always empty by design (see the HIT arm above), so
        // checking those here would wrongly drop every hit group.
        //
        // For a reader-stamped group this `continue` also drops
        // `group.reader_guard` WITHOUT calling `finish()` — an explicit
        // guard-discard, not an oversight: the same "no `finish()` on a
        // read that produced nothing" discipline `AcquisitionGuard`
        // documents for an ordinary failed read (see its doc comment).
        // The group's window slot keeps whatever it held before; nothing
        // was actually read this tick, so there is nothing honest to
        // publish. A real reader-stamped group reaches this with no
        // populated slots: `cpu_usage`'s task group when
        // `task_attribution` is off (its metrics are registered, nothing
        // backs them), and any slot-keyed group whose slots are all empty.
        if group.counter_values.is_empty()
            && group.gauge_values.is_empty()
            && group.histogram_values.is_empty()
        {
            continue;
        }

        // Wire-format name, built once per DISTINCT GROUP here (bounded by
        // declared-group count, ~tens) — not once per registry entry (the
        // `format!` this loop used to pay for at ROUTING time, before the
        // `(&str, &str)` tuple-keyed `groups`/`group_registry` change).
        let group_name = format!("{}/{}", group_key.0, group_key.1);

        // Reader-stamped groups (`PackedCounters` mmap-direct): `finish()`
        // PUBLISHES the bracket acquired at first touch — stamp-last, same
        // rule `AcquisitionGuard` enforces for sampler-stamped groups (see
        // its doc comment), applied here to the reader instead of a
        // sampler. The published WIDTH was already decided earlier, by the
        // last `mark_end()` call the CounterGroup/GaugeGroup arms made for
        // this group (immediately after each touching metric's member-
        // value loop, above) — `finish()` here only decides WHEN the slot
        // becomes visible, not what it contains. No `resolve_walk_window`
        // reconciliation is needed: the ONLY writer for a reader-stamped
        // group's slot is this walk itself (see
        // `AcquisitionGroup::set_reader_stamped`'s single-writer note), so
        // this acquire()/mark_end()/finish() sequence is the complete, sole
        // write for the tick — there is no concurrent background sampler
        // that could have re-stamped it mid-walk, unlike the sampler-
        // stamped case `resolve_walk_window` guards against.
        let window = if let Some(guard) = group.reader_guard {
            guard.finish();
            group_registry.get(&group_key).and_then(|ag| ag.window())
        } else {
            // Re-read the window here (after this group's values, above)
            // and reconcile with the first-touch read via
            // `resolve_walk_window` — see its doc comment for why a second
            // read is necessary.
            let latest_window = group_registry.get(&group_key).and_then(|ag| ag.window());
            resolve_walk_window(group.window, latest_window)
        };

        let (schema, hash) = if group.needs_schema {
            let schema = GroupSchema {
                counters: group.counter_descs,
                gauges: group.gauge_descs,
                histograms: group.histogram_descs,
            };
            let hash = schema.hash();
            let schema = Arc::new(schema);
            // Fix for miss-tick cache poisoning: the identity STORED here
            // is folded from what THIS WALK actually collected
            // (`group.walk_identity`, folded alongside every desc pushed
            // above), NOT from `fold_group_identities`' pre-pass read. For
            // a default group, those two can legitimately disagree (its
            // membership is value-derived — see the "wider torn-read
            // window" note on `fold_group_identities`): the pre-pass might
            // trigger this rebuild based on a read that doesn't match what
            // the walk below actually saw. Storing the pre-pass's identity
            // anyway would bind THIS schema to an identity the walk didn't
            // produce — if membership later drifts back to what the
            // pre-pass saw, a future tick would wrongly HIT and ship this
            // schema against a values vector collected under a DIFFERENT
            // membership, forever (not self-correcting). Storing the
            // walk's own identity makes the invariant "stored identity
            // always describes the stored schema" hold unconditionally, so
            // only the bounded, self-correcting hit-tick race (handled
            // below) remains.
            let identity = group.walk_identity.finish();
            cache.entries.insert(
                group_name.clone(),
                GroupSkeleton {
                    identity,
                    schema: schema.clone(),
                    hash,
                },
            );
            cache.rebuilds += 1;
            (schema, hash)
        } else {
            // Hit path. `fold_group_identities`' pre-pass read a default
            // group's value-derived membership separately from this walk
            // (see its doc comment) — rarely, a value can cross the
            // membership boundary in between, so the pre-pass's "hit"
            // call and what THIS walk actually collected can disagree.
            // Verify arity against the cached schema before trusting it:
            // a mismatch means the cached schema does not actually
            // describe this tick's collected values. Evict the entry and
            // skip emitting this group for this tick rather than shipping
            // a payload every conformant `GroupSnapshot::validate()` call
            // would reject anyway — this is the self-correcting half of
            // the same race; the miss path above closes the
            // NON-self-correcting (permanent poisoning) half.
            match cache.entries.get(&group_name) {
                Some(cached)
                    if cached.schema.counters.len() == group.counter_values.len()
                        && cached.schema.gauges.len() == group.gauge_values.len()
                        && cached.schema.histograms.len() == group.histogram_values.len() =>
                {
                    (cached.schema.clone(), cached.hash)
                }
                Some(cached) => {
                    debug!(
                        "SkeletonCache arity mismatch on a hit tick for group `{group_name}` \
                         (cached schema counters={}/gauges={}/histograms={}, this tick's \
                         collected values counters={}/gauges={}/histograms={}) — evicting the \
                         stale entry and skipping this group for this tick",
                        cached.schema.counters.len(),
                        cached.schema.gauges.len(),
                        cached.schema.histograms.len(),
                        group.counter_values.len(),
                        group.gauge_values.len(),
                        group.histogram_values.len(),
                    );
                    // Note: for a reader-stamped group this `continue`
                    // runs after `guard.finish()` above, so the window is
                    // published for a tick whose group we then drop —
                    // unlike the empty-group skip, which deliberately
                    // discards its guard. Unreachable today: reader-stamped
                    // implies declared, whose membership is
                    // registration-derived and therefore identical in both
                    // passes, so this arm cannot be reached with a reader
                    // guard in hand. If a future membership rule breaks
                    // that implication, move the arity check above the
                    // window resolution.
                    cache.entries.remove(&group_name);
                    continue;
                }
                None => {
                    // Shouldn't happen (fold_group_identities only says
                    // needs_schema=false when a cache entry exists for
                    // this exact group), but defensive rather than a
                    // panic: skip this group for this tick.
                    debug!(
                        "SkeletonCache: group `{group_name}` marked needs_schema=false but has \
                         no cache entry — fold_group_identities and this loop disagree on \
                         cache state; skipping this group for this tick"
                    );
                    continue;
                }
            }
        };

        group_snapshots.push(GroupSnapshot {
            name: group_name,
            schema_hash: hash,
            schema: Some(schema),
            window,
            counters: group.counter_values,
            gauges: group.gauge_values,
            histograms: group.histogram_values,
        });
    }

    group_snapshots.sort_by(|a, b| a.name.cmp(&b.name));

    // Close the pass: whatever the walk observed becomes one replayable step
    // from the state this pass started at. A pass that moved nothing records
    // nothing — see `IndexHistory::record`.

    Snapshot::V3(SnapshotV3 {
        systemtime: timestamp,
        duration,
        metadata: [
            ("source".to_string(), env!("CARGO_BIN_NAME").to_string()),
            ("version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
            // Rides on every snapshot, not just on `/status`, because it is
            // what lets a consumer notice the agent restarted BETWEEN two
            // scrapes: at that moment every counter in the payload restarted
            // from zero together, and the values alone cannot say so.
            (
                "producer_epoch".to_string(),
                crate::agent::epoch::producer_epoch().to_string(),
            ),
            // The timeline `systemtime` sits on. A consumer that has this can
            // place a reading without trusting its own clock to agree with
            // this host's, and can tell a wall-clock step from elapsed time:
            // `systemtime` moves with a step, the anchor does not.
            //
            // Carried here as well as on `/status` because a snapshot is the
            // whole of what some consumers ever read, and an anchor fetched
            // separately could belong to a different run of the agent.
            (
                "clock_anchor_wall_ns".to_string(),
                crate::agent::epoch::clock_anchor_wall_ns().to_string(),
            ),
            // The pass's own stamp on that timeline, and the wall clock's
            // disagreement with it at the read. `ts + wall_offset ==
            // systemtime`, so a consumer that keeps these keeps the moment the
            // agent READ the values — which is not the moment it answered, and
            // not the moment the answer arrived.
            ("ts".to_string(), stamp.0.to_string()),
            ("wall_offset".to_string(), stamp.1.to_string()),
        ]
        .into(),
        groups: group_snapshots,
    })
}
