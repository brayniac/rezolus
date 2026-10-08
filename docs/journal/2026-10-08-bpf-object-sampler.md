# A sampler from a BPF object named in config

- **Opened:** 2026-10-08
- **Status:** OPEN — proposal, pre-build. Nothing is implemented. Facts are
  stated at `main` (`4b6863a4`); `v5.25.1` (`2393c800`) is named only where
  it differs. Open questions 1, 2 and 4 need a decision before coding.
- **Driver:** [`cpu_perf` under virtualization](../backlog.md). In a KVM
  guest, `cpu_perf`'s `sched_switch` program costs 20.5 µs per run because
  every counter read traps to the hypervisor. A guest can avoid the trap
  only with a kernel interface that Rezolus cannot depend on.
- **Tracked in:** [`docs/backlog.md`](../backlog.md) → "Agent — samplers
  from BPF objects".

## Motivation

`docs/backlog.md` → "Agent — `cpu_perf` under virtualization" records the
cost:

- 8,235,630 runs of `cpu_perf`'s `sched_switch` program took 169 s of
  program time over a 247 s `perf bench sched pipe` in a KVM guest.
- That is 20.5 µs per run, against 114 ns and 665 ns for the other programs
  on the same tracepoint.
- The time is the hypervisor's trap-and-emulate of each
  `bpf_perf_event_read`.

### A trap-free read path

Reading hardware counters in a guest without a trap needs both sides of the
VM boundary.

**Host side.** A patched KVM (`kvm.pmc_page=1`, and on x86 also
`kvm_amd.rdpmc_passthrough=1`) keeps a page per vCPU, for a guest that has
no virtual PMU. The page names the hardware counter that holds
the vCPU's guest-only cycles and the one that holds its instructions.

**Guest side.** An out-of-tree guest module (`guest_pmc`) registers the pages
and exports kfuncs that read a counter by perf's userspace protocol:
`bpf_guest_pmc_read(event)` and `bpf_guest_pmc_index(event)`. A
`sched_switch` program calls them where `cpu_perf`'s program calls
`bpf_perf_event_read`.

**Measured on a Zen 1 host,** with `perf bench sched pipe`, the median of
three runs per mode. A `sched pipe` operation is a round trip of at least two
context switches. The first two rows are from one run of experiments and the
third from another, whose no-vPMU figures were 25.4 and 25.9 µs.

| mode | µs per operation | reading two counters at each switch adds, µs per operation |
|---|---|---|
| no vPMU, no program | 25.79 | baseline |
| no vPMU, reading through the page | 26.17 | 0.4 (0.3–0.6 across three runs) |
| emulated vPMU, reading through it | 222.6 | 89.9, over a vPMU baseline of 132.7 |

- **Accuracy.** Per-task counts through the page matched the host's
  guest-only counts within 1.3%, with no forced counter moves. With two
  forced moves they matched within 0.9% for cycles and 0.1% for
  instructions.
- **Raspberry Pi 4 (Cortex-A72).** Reading through the page changed the
  benchmark by −0.7 to +2.9 µs on about 45.8 µs, within run-to-run spread.
  `cpu_perf` over the vPMU added 5.1 µs on 60.5 µs.

### Why it cannot ship in Rezolus

That program cannot be part of Rezolus:

- The page, its MSR (`0x4b564df0`) and its arm64 hypercall exist only in
  out-of-tree KVM patches.
- The kfuncs exist only in an out-of-tree module.
- The page layout carries a sequence counter but no format version.
- No CI here can load the module, so a break would go unnoticed.
- On a stock kernel the code would never run.

`[external_metrics]`, the existing way to bring in outside data, does not fit
either. It takes values pushed by another process over a Unix socket, and
falls short in four ways:

- **Timing.**
  - Native counters are read when a snapshot is built
    (`SnapshotBuilder::build`, `src/agent/exposition/http/snapshot.rs`).
  - An external metric is the last value pushed, stamped with the
    `Instant` it arrived (`src/agent/external_metrics/store.rs`).
  - For a source pushing once a second and a 1 s snapshot window, a rate can
    cover anything from nearly zero to two seconds of counting.
- **Names.**
  - `upsert` refuses any name a registered metric already uses, so an
    external source cannot provide `cpu_cycles` in a guest where `cpu_perf`
    did not start.
  - External columns carry no unit or description.
  - `docs/external_metrics.md` says names get an `ext_` prefix; no code
    applies one.
- **Cgroup attribution.** Per-cgroup values join with Rezolus's own only if
  they use its key (the CPU controller's `css.id`) and its slot names. An
  external process would reproduce both and keep them in step.
- **Lifecycle.** The pushing process is a second service that Rezolus
  neither starts nor reports on.

Timing and lifecycle come from the code running outside Rezolus's sampling
loop; names and cgroup attribution come from the external metrics interface.
This proposal runs the code inside the loop: Rezolus loads a BPF object named
in config, attaches its programs, and reads its maps when it builds a
snapshot, as it does for a built-in sampler.

## What exists today

### Samplers

- **The trait.** A sampler implements `Sampler { fn name() -> &'static str;
  async fn refresh(); }` (`src/agent/samplers/mod.rs`).
- **Registration.** Each sampler registers a `SamplerEntry` in the `SAMPLERS`
  linkme slice, and `agent::run` calls each entry's `init`.
- **Refresh.** `SnapshotBuilder::build` refreshes every sampler concurrently,
  with `join_all`, when the cached snapshot is older than its TTL. Nothing
  runs in the background unless a stream subscriber asks for periodic
  sampling (`clock.rs`).
- **Unknown sampler names.** On `main`, an unknown `[samplers.X]` name is
  warned about and ignored (`unknown_names`, `src/agent/config/mod.rs`). In
  `v5.25.1` it was accepted silently.

### BPF

- **Skeletons only.** Every object is a libbpf-rs skeleton generated at
  build time from `SOURCES` in `build.rs`. Nothing loads a `.bpf.o` from a
  path.
- **The builder.** `Builder<T: SkelBuilder>` (`src/agent/bpf/builder.rs`)
  opens, loads and attaches each autoloaded program. It requires `SkelExt`,
  whose `map(name)` is a per-skeleton `match` ending in `unimplemented!()`
  (`src/agent/bpf/mod.rs`).
- **Ringbuf handlers** are plain `fn(&[u8]) -> i32` pointers. A sampler
  reaches its `SlotIdentity` through a `static`.
- **The readers** take a plain `&libbpf_rs::Map` and mmap it
  (`src/agent/bpf/counters.rs`): `Counters`, `CpuCounters`,
  `PackedCounters`, `SparsePackedCounters`, filesystem counters and
  histograms.
  - They write into `&'static` metriken counters and groups, and stamp a
    `&'static AcquisitionGroup`.
  - They `expect` or panic on a map whose size does not match.
- **The two counter layouts**, both on an mmapable `BPF_MAP_TYPE_ARRAY` of
  `u64`:
  - `MAX_CPUS` banks, each holding N counters padded to whole cachelines.
    `Counters` sums the banks; `CpuCounters` keeps one value per CPU.
  - Slots read as one flat array (`PackedCounters`), as the cgroup arrays
    are.
- **No kfuncs yet.** No program uses `__ksym` or kfuncs. libbpf is 1.7.0,
  through libbpf-rs 0.26.2.

### Metrics and groups

- **Static registration.** Native metrics are static `#[metric]`s, and the
  snapshot walks `metriken::metrics()`. Metrics built at runtime with
  `MetricBuilder` are included too, after the statics (in heap-address
  order), so static metric ids keep their numbers. metriken allows two
  metrics with one name.
- **The `sampler` label** comes from a metric's module path, through
  `attribute_sampler` (`src/agent/samplers/mod.rs`), called from four
  places: the router's group lookup (`router.rs`), `metric_metadata`, and
  the two group lookups in V2 `create` (`snapshot.rs`). `metric_metadata`
  also overwrites any `sampler` metadata key. A `MetricBuilder` metric's
  module is `""`, so it is `unattributed`, and the router then looks for its
  group under `unattributed`.
- **One group per reader.** Readers stamp their groups differently:
  `CpuCounters` bounds its group to present CPUs and stamps it itself;
  `PackedCounters` makes its group reader-stamped. Built-in samplers
  therefore use a group per reader (`cpu/linux/perf/stats.rs`:
  `CPU_PERF_ACQ` and `CGROUP_CYCLES_ACQ`).
- **Descriptions.** `/metrics/descriptions` keeps the first description seen
  for a name, and statics come first. The recorder copies it into
  recordings.
- **Acquisition groups.** The router builds its group table once, from the
  `ACQUISITION_GROUPS` linkme slice (`exposition/http/router.rs`). A metric
  naming an unregistered group goes to the default group.
- **External metrics** form an `ExtraGroup` of flat members, with column keys
  `external/<name>#<labels hash>`. `SlotIdentity` labels cannot reach them.
- **Slot identity.** Per-CPU and per-cgroup values are metriken
  `CounterGroup`s, whose slots carry `id` and, through
  `metriken::group::SlotIdentity` (`assign`/`release`), a `name` and a
  `__uid__`. Each `SlotIdentity` mints its own uids, so `cpu_usage` and
  `cpu_perf` already give the same cgroup different `__uid__`s.

### Cgroups

- **The key.** It is `task->sched_task_group->css.id`, capped at
  `MAX_CGROUPS` 4096. Ids at or above the cap are dropped
  (`src/agent/bpf/cgroup.h`; backlog "Agent — cgroup slots").
- **Per-sampler maps.** A sampler owns:
  - a `cgroup_info` ringbuf;
  - a `cgroup_serial_numbers` array;
  - its value arrays.
- **New cgroups.** When a slot's `css.serial_nr` changes, `cgroup.h` emits a
  `struct cgroup_info`, and the calling program zeroes that slot in its own
  value arrays (`cpu/linux/usage/mod.bpf.c`). The zeroing is caller code,
  not part of `cgroup.h`.
- **Includes.** `cgroup.h` includes the arch-specific `vmlinux.h` and
  `core_fixes.h`. `helpers.h` includes `histogram.h`.

### No PMU

`cpu_perf` probes for schedulable counters (`src/agent/pmu.rs`). With none,
`agent::run` records it as `PmuStarved` and does not start it, without error
(`src/agent/mod.rs`). With counters on some CPUs but not all, it runs as
`PmuLimited`.

### Failure

- **At init.** A failure records the sampler as `Failed` or `Unsupported`,
  and the agent continues. The states are `Active`, `Disabled`, `Failed`,
  `PmuLimited`, `PmuStarved` and `Unsupported`
  (`src/agent/sampler_status.rs`).
- **Where status goes.** `/samplers` serves the status, and the recorder
  writes it into each source's metadata as `sampler_status` when a recording
  starts (`src/recorder/mod.rs`).
- **After init.** If a BPF thread exits, `refresh` panics.

### Agent

The agent runs as root (`debian/rezolus.rezolus.service`). Nothing checks who
owns its config file.

## Design

### Config

```toml
[objects.guest_pmc]
path = "/usr/lib/rezolus/objects/guest_pmc.bpf.o"
manifest = "/usr/lib/rezolus/objects/guest_pmc.toml"   # default: path with .toml
enabled = true
```

- **A section of its own.** Objects get `[objects.<name>]`, outside
  `[samplers]`. An object's keys are parsed with `deny_unknown_fields`, so a
  misspelled key is an error. An agent older than this ignores `[objects]`
  silently, since `Config` accepts unknown keys; the guide says so.
- **Names.**
  - `<name>` must match `[a-z][a-z0-9_]*` and must not be a built-in
    sampler's name.
  - It is the object's one identity: its `Sampler::name`, its entry in
    `/samplers`, the `sampler` label on its metrics, and the prefix of its
    acquisition groups (`object_<name>_<map>`).
  - The name is leaked to `&'static str` once at init, as `Sampler::name`
    and the status table require.

### Manifest

The object ships with a TOML manifest saying what each map means:

```toml
contract = 1

[[map]]
name = "counters"
layout = "banked"            # banked | slots
counters = 2                 # counters per bank; banks are MAX_CPUS

[[map]]
name = "cgroup_cycles"
layout = "slots"
slots = "cgroup"             # slots are MAX_CGROUPS, keyed by the cgroup contract

[[metric]]
name = "cpu_cycles"
map = "counters"
index = 0
read = "per_cpu"             # per_cpu | sum   (banked maps)
kind = "counter"             # counter | gauge
unit = "cycles"
description = "Guest-only CPU cycles, read through the KVM pmc page at context switch"

[[metric]]
name = "cgroup_cpu_cycles"
map = "cgroup_cycles"
kind = "counter"
unit = "cycles"
description = "Guest-only CPU cycles per cgroup, read through the KVM pmc page"
```

- **Layouts.** Contract 1 has two, the two `counters.rs` already reads:
  - `banked`: `MAX_CPUS` banks of `counters` `u64`s, each bank padded to
    whole cachelines. A metric names its `index` in the bank. `read =
    "per_cpu"` reads it as a per-CPU group (`CpuCounters`); `read = "sum"`
    reads it as one total (`Counters`).
  - `slots = "cgroup"`: `MAX_CGROUPS` `u64` slots, one per cgroup, under the
    cgroup contract below (`PackedCounters`).
  - Histograms, hash maps and per-task slots are not in contract 1; see
    Phasing.
- **Validation.**
  - Before load, Rezolus reads each declared map's definition from the
    object and checks four things: `BPF_MAP_TYPE_ARRAY`, `BPF_F_MMAPABLE`,
    an 8-byte value, and `max_entries` equal to what the layout implies. For
    `banked` that is `MAX_CPUS × whole_cachelines::<u64>(counters) ×
    COUNTERS_PER_CACHELINE`, the bank width `CounterMap::with_banks`
    computes (`counters.rs`).
  - Every mismatch is a load failure naming the map and the field.
  - The readers need one change for this. `Counters` and `CpuCounters`
    take the bank width from the number of metrics passed to them and map
    the i-th counter to the i-th metric. A manifest with `counters = 9`
    that reads only index 0 would get a stride of 8 against a real bank of
    16 and read wrong values without an error. They take an explicit
    counter count and an index-to-metric map instead.
- **The contract number.** `contract` changes when a layout's meaning
  changes. Rezolus refuses a contract it does not know.

### Metrics

Object metrics are metriken metrics created at runtime with `MetricBuilder`,
not the external path. That way per-CPU and per-cgroup values are
`CounterGroup`s with slot identity, the same as built-in samplers' values,
and the existing readers fill them with the change above.

- **Groups.** Each map gets its own `AcquisitionGroup`, named
  `object_<name>_<map>`, created at init. A `slots` map's group is
  reader-stamped, as built-in cgroup groups are.
- **Column keys** are the snapshot's usual `<metric_id>x<idx>`, with no new
  form needed.
- **Descriptions and units** go into each metric's metadata and onto
  `/metrics/descriptions`.

The refactor this needs:

1. **Sampler attribution for runtime metrics.** A wrapper over
   `attribute_sampler` takes the `&MetricEntry`. It returns the object's
   name when the metric was built with
   `MetricBuilder::provide(ObjectSampler(name))` (read back with
   `metric.request_ref::<ObjectSampler>()`, both in metriken-core), and
   otherwise calls `attribute_sampler` with the module path, which stays
   for its tests. `ObjectSampler` holds the object's leaked `&'static str`
   name (Config, Names), as the router's group keys require. The callers
   move to the wrapper:
   - `RezolusRouter::sampler`, with the object check made before its
     per-module cache, since every object metric's module is `""`;
   - `metric_metadata` in `snapshot.rs`;
   - the two group lookups in V2 `create` (`snapshot.rs`), which otherwise
     miss every object metric's group: the first reaches a `debug_assert!`,
     the second falls back to the stored window without one.

   Without this, every object metric is `unattributed` and lands in the
   default group.
2. **Groups at runtime.** `group_registry()`, a `OnceLock` over the
   `ACQUISITION_GROUPS` slice today, takes groups registered after startup.
   The router, which also serves the stream path, and V2 `create` both read
   it.
3. **Ringbuf handlers as closures.** `Builder::ringbuf_handler` takes a
   boxed closure, so an object's `cgroup_info` handler can hold its own
   `SlotIdentity`. Built-in samplers can keep passing their `fn`s.
4. **Readers with an explicit layout.** `Counters` and `CpuCounters` take
   the counter count and an index-to-metric map (see Validation).
5. **A builder over an opened object.** `Builder` is generic over "an opened
   object whose maps can be found by name". A skeleton is one implementation
   and a runtime `Object` is another, with lookup returning `Option`.
6. **Leaked statics.** Metrics, groups and identities are created once per
   object and leaked to `'static`, as the readers require. Objects load once
   at startup (open question 4), so the leak is bounded.

### Cgroup contract

A `slots = "cgroup"` map uses Rezolus's cgroup key and slot names.

- **Slot labels.** Its slots carry the same `name` label as
  `cgroup_cpu_usage`, and a `__uid__` of their own, as each built-in
  sampler's do.
- **What the object must declare:**
  - a `cgroup_info` ringbuf of `struct cgroup_info`;
  - a `cgroup_serial_numbers` `u32 → u64` array of `MAX_CGROUPS` entries;
  - its slot arrays.
- **What the object must do:** call `cgroup.h`'s helpers, and zero its own
  slots for an id when the helper reports a new cgroup there. The contract
  states the zeroing, which `cgroup.h` does not do.
- **What Rezolus installs,** under `/usr/include/rezolus/bpf/`, so an object
  builds against the same definitions:
  - `cgroup.h`, `core_fixes.h`, `helpers.h` and `histogram.h`;
  - a new header for the bank layout. Today each program defines `MAX_CPUS`
    itself (`cpu/linux/usage/mod.bpf.c`) and pads its banks to match
    `CounterMap::with_banks` by hand;
  - each architecture's `vmlinux.h`.
- **What becomes public API:** `MAX_CGROUPS`, the key and
  `struct cgroup_info`. A change to any of them is a contract change.

Two objects, or an object and a built-in sampler, each own their
`cgroup_info`, serial numbers and identity. Each one notices that a cgroup id
was reused on its own next event for that cgroup. Until then they can
disagree about which cgroup holds the id, as built-in samplers can today.

### Substitution

An object's metric may use a built-in metric's name, under three conditions:

- the manifest says so with `substitutes = true` on that metric;
- no running built-in sampler fills that metric;
- the check is made after built-in init, per metric.

Nothing records today which metrics a running sampler fills; whether
`cpu_perf` fills `cgroup_cpu_cycles` is known only inside its `init`. So
each `SamplerEntry` gains a function that, given the config, lists the
metrics the sampler fills. Substitution checks a metric against the lists of
the samplers that started.

For example, `cpu_perf` starved of counters is not running, so an object may
provide both `cpu_cycles` and `cgroup_cpu_cycles`. `cpu_perf` running with
`cgroup_attribution = false` lists `cpu_cycles` but not `cgroup_cpu_cycles`,
so an object may provide only the second.

- **Two metrics, one name.** The static metric still exists when its sampler
  did not start, and metriken allows a second metric with the same name.
  Snapshot members are positional, so there is no collision: the unfilled
  static emits nothing (its group is bounded to no members, or never
  allocated), and parquet gets one populated column.
- **Labels.** A substituted metric carries `sampler=<object name>` (refactor
  item 1), which names its source, since object names cannot be built-in
  sampler names. No separate label is needed.
- **`PmuLimited`.** A `cpu_perf` running on some CPUs still lists
  `cpu_cycles`, so an object is refused for every CPU, including those
  `cpu_perf` does not cover. Per-CPU substitution is not in contract 1.
- **Descriptions.** `/metrics/descriptions` keeps the first description it
  sees, and statics come first, so a substituted metric would be recorded
  with the built-in description. The handler prefers the substituting
  metric's description for a name.
- **When refused.** A metric a running sampler fills is not created. The
  object's health message says which and why.
- **External metrics.** `reserved_names` for the external store is built
  after objects load, so a pushed metric cannot reuse an object's name.

Substitution changes what the name measures, which the manifest's
description and the `sampler` label have to say. For the first user:

- **Guest-only counts.** `cpu_cycles` counts guest execution only.
- **When the value moves.** A `sched_switch` program credits counts at
  context switches, so its per-CPU totals move only when a CPU switches. On a
  CPU running one task without switching, the series is flat and then steps.
  The built-in `cpu_cycles` is read when the snapshot is built.

### Loading

- **Open from a checked file.** Walk the path from `/` with `openat` and
  `O_NOFOLLOW` at each component, checking each directory and the file on
  the open descriptor (owned by root, not group- or world-writable). Load
  the object from the bytes read through that descriptor with
  `ObjectBuilder::open_memory`. The manifest is opened the same way, since
  it decides metric names and substitution.
- **What this check is for.** The agent already runs as root and reads its
  config without checking the owner. This makes an object at least as hard
  to replace as the agent's binary.
- **Program types.** Only these section kinds are accepted: `tp_btf`,
  `raw_tp`, `tp`, `kprobe` and `kretprobe`, `fentry` and `fexit`. The kind
  is decided from the section name, since `fentry`, `fmod_ret` and `iter`
  share a program type. `perf_event` is left out: libbpf cannot attach it
  without an event and period, which the manifest does not carry. This
  limits where an object attaches. It does not make an object read-only: a
  `kprobe` can still call `bpf_override_return` or
  `bpf_probe_write_user`, and refusing those helpers would need the
  verifier's help. The loaded object is trusted code, like the agent.
- **No pinned maps.** Maps with a pinning attribute are refused, so an
  object cannot reach maps outside itself.
- **How the pre-load checks are made.** libbpf-rs 0.26 exposes a map's type
  and `max_entries` before load, but not its value size, flags or pin path.
  Those are read through libbpf-sys on the open map (`bpf_map__value_size`,
  `bpf_map__map_flags`, `bpf_map__pin_path`), with the pointer from
  `AsRawLibbpf`. The pin path is set at open for `LIBBPF_PIN_BY_NAME`, so
  the pinning check works before load. A program's section name comes from
  `OpenProgram::section()`.
- **Kfuncs.** libbpf resolves `extern ... __ksym` kfuncs against vmlinux BTF
  and then the loaded modules' BTF. A missing kfunc fails the load with
  `-ESRCH` ("not found in kernel or module BTFs"). A `__weak` one resolves to
  0, and the program is expected to test `bpf_ksym_exists()` itself.
  - The load error carries only the errno; the symbol's name is in libbpf's
    log line. The opened object cannot be asked either: at open, libbpf
    rewrites each `.ksyms` function's BTF linkage to global, and whether an
    extern is `__weak` is in the ELF symbol table, not in BTF.
  - So before open, Rezolus parses the bytes it already holds: `.BTF` with
    `btf__new` on that section's data, and `.symtab` for symbol binding (this
    adds `object` as a direct Linux dependency; it is in `Cargo.lock` at
    0.37.3 already, through `backtrace`). It lists FUNC types with extern
    linkage whose symbols are not `STB_WEAK`, checks each against vmlinux and
    module BTF, and records a missing one as `Unsupported` with its name.
    Today any load error is `Failed`. A kfunc that exists with an incompatible
    prototype fails the load with `-EINVAL` and stays `Failed`.
  - `btf_custom_path` is used only for CO-RE relocation, not for kfuncs.
- **Attach.** Each autoloaded program is attached as `Builder` does now,
  with the same per-program unsupported and degraded accounting.
- **Refresh.** Objects implement `Sampler`, so they are refreshed in the
  same `join_all` as the built-in samplers.
- **Program statistics.** Objects report `rezolus_bpf_run_count` and
  `rezolus_bpf_run_time` like built-ins, through runtime metrics in place of
  `BpfProgStats`'s statics.

### Failure and health

- **Never fatal.** A failure to open, validate, load or attach an object is
  that object's `Failed` or `Unsupported` status, and never stops the agent.
- **Recorded at start.** Recordings already carry `/samplers`, as
  `sampler_status` in each source's metadata, so a recording shows an
  object's state when it started. A health gauge would add changes during
  a recording. Those matter once a load can be retried (open question 4).
- **File descriptors.** Every mmap reader wraps libbpf's map fd in a `File`
  and drops it, closing the fd libbpf still holds (`counters.rs`,
  `histogram.rs`, `builder.rs`). Skeletons are never closed, so this has not
  mattered. An object that is closed after a failed attach would close those
  numbers a second time. The readers must duplicate the fd before wrapping
  it.

### Overhead

- **The loader** adds nothing per event. An object pays only for its own
  programs.
- **The first user adds one program to `sched_switch`, in place of
  `cpu_perf`'s,** which does not start in a guest with no vPMU.
- **Where an object does add a program to a hook Rezolus already uses,**
  [one program per hook](2026-10-03-one-program-per-hook.md) measured the
  dispatch on bare metal:
  - at most 29 ns per switch on `sched_switch`;
  - 38 ns per syscall on `sys_enter`;
  - in a guest, 50–64 ns per syscall on `sys_enter`; `sched_switch` did not
    separate.
- **Counting programs.** An object cannot be merged into a built-in sampler's
  program, so `/samplers` lists each object's attach points.

## Change list

- `src/agent/config/`: `[objects]` parsing, with `deny_unknown_fields` and
  the name rule.
- `src/agent/objects/` (new): manifest parsing, map validation, the checked
  open, the extern kfunc check from the ELF bytes, the program-type and
  pinning checks, runtime metrics and groups, substitution.
- `src/agent/bpf/builder.rs`: generic over an opened object; boxed ringbuf
  handlers; `-ESRCH` → `Unsupported`, as a fallback for what the pre-open
  kfunc check misses.
- `Cargo.toml`: `object` under
  `[target.'cfg(target_os = "linux")'.dependencies]`, without default
  features (`read_core`, `elf`).
- `src/agent/bpf/counters.rs`, `histogram.rs`: duplicate the map fd before
  wrapping it; explicit counter count and index-to-metric map.
- `src/agent/exposition/http/router.rs` and the stream path: acquisition
  groups registered at runtime; sampler attribution through the metric
  entry, checked before the per-module cache.
- `src/agent/exposition/http/snapshot.rs`: sampler attribution through the
  metric entry in `metric_metadata` and the two V2 `create` group lookups;
  `group_registry()` takes runtime groups.
- `src/agent/exposition/http/mod.rs`: `/metrics/descriptions` prefers a
  substituting metric's description.
- `src/agent/samplers/mod.rs`: `SamplerEntry` lists the metrics a sampler
  fills under a config; a wrapper over `attribute_sampler` takes a
  `&MetricEntry` and checks for `ObjectSampler`; `attribute_sampler` stays
  for its tests.
- `src/agent/mod.rs`: load objects after built-in init; build
  `reserved_names` after that.
- `src/agent/bpf/`: move the bank layout into a header; install headers in
  the package.
- `debian/`, `rpm/`: ship `/usr/include/rezolus/bpf/`.
- `docs/`: an objects guide; correct `docs/external_metrics.md`, which
  promises an `ext_` prefix no code applies.

## Testing

- **A test object in the tree.** `build.rs` builds it, but it is loaded by
  path, never as a skeleton. It has a `banked` map read both ways, a cgroup
  map, and a program on a tracepoint a test can trigger. Test cases:
  - the values it exposes;
  - a map of the wrong size, refused with the map and field named;
  - a group-writable object, refused;
  - an unknown contract, refused;
  - an `lsm` program, refused;
  - a pinned map, refused;
  - a `__ksym` that no kernel provides, `Unsupported` with its name;
  - a `__weak` `__ksym` that no kernel provides, which still loads.
- **Substitution.** With `cpu_perf` disabled, the object's `cpu_cycles` is
  exposed with `sampler=test`. Refusal is tested against a metric of a
  sampler that runs on any CI machine, since runners may expose no PMU and
  `cpu_perf` would then be starved.
- **Where the tests run.** Loading needs root, so these run in the Linux
  leg of the CI smoketest, which already starts the agent with `sudo`, not
  in `cargo test`. The checkout is owned by the runner's user, so the test
  installs the object and manifest with `sudo install -o root -m 0644`
  into a root-owned directory before loading.
- **The guest reader stays outside this repo.** Its end-to-end check is a
  guest with no vPMU on a host with the page, comparing `cpu_cycles` with the
  host's guest-only count for the same window.

## Phasing

1. **Loader.** Config, manifest, `banked` maps, validation, the checked
   open, program-type and pinning checks, runtime metrics and groups, and
   status. This is refactor items 1, 2, 4, 5 and 6 above, and the fd
   change.
2. **Cgroups.** The cgroup contract, closure ringbuf handlers (refactor
   item 3) and the installed headers.
3. **Substitution.**
4. **Later, as an object needs them:** histograms (the `histogram` reader's
   layout), and per-task slots under the `task_attribution` convention.

### The first user

The guest reader is not written yet. The existing program in the
out-of-tree repository is a test: it keys totals by tgid in a hash map and
keeps per-CPU state in a `PERCPU_ARRAY`. Neither is a contract-1 layout.

Written against contract 1, it keeps its previous counts in a `banked` map
and credits totals into one. It can use phase 1 for per-CPU totals, and
needs phases 2 and 3 for per-cgroup counts under the built-in names.

## Open questions

1. **Sidecar or embedded manifest.**
   - A sidecar TOML is simple, and the object and its manifest ship
     together in one package.
   - Embedding it in the object (an ELF section, or BTF-typed `.rodata`
     read at open) makes the object self-describing and removes a way for
     the two to disagree.
   - Proposed: sidecar for contract 1.
2. **Making the cgroup convention public.**
   - It freezes `MAX_CGROUPS`, the `css.id` key and `struct cgroup_info`.
     Backlog "Agent — cgroup slots" has an open item on ids above the cap.
   - Contract 1 could ship without `slots = "cgroup"` until that is settled.
3. **Unloading a module in use.** A loaded program that calls a module's
   kfunc probably holds a reference on the module (the verifier's kfunc
   BTF table), so the module cannot be unloaded while Rezolus runs. I have
   not checked this in kernel source.
4. **Load order.**
   - A module's kfunc resolves only if the module is loaded before the
     object.
   - Loading once at startup fails if the module loads later in boot.
   - Either leave ordering to the service manager, or add
     `requires_module = "<name>"` and retry the load when the module
     appears. Retrying is when a health gauge and closing objects (the fd
     change) start to matter.
5. **Substituted names and `sampler` consumers.** With refactor item 1, a
   substituted `cpu_cycles` carries `sampler=<object name>`, not
   `cpu_perf`. These read the `sampler` label and would place it under the
   object: `subsystem_of` (`src/analysis/extract/mod.rs`),
   `ground_truth.rs`, `hindsight/buffer.rs`, and the per-sampler tables in
   `.rez` files that `parquet filter --samplers` selects. Whether a
   substituted metric should also appear under the built-in sampler's
   subsystem is undecided. I have not checked the viewer's dashboards.
