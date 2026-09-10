# Extract the `.rez` container into `dendro`

- **Opened:** 2026-09-10
- **Status:** DONE — `iopsystems/dendro` exists and is public; `crates/rez`
  builds on it; the whole workspace is green (159 rez + 690 binary + the
  integration suites) and both wasm checks pass. One follow-up is open: the
  dendro CI workflow is written but unpushed, pending a `workflow` scope on the
  pushing account's token.
- **Arc:** consumes the container work
  ([per-sampler archive](2026-07-13-per-sampler-rez-archive.md),
  [streaming writer](2026-08-11-rez-streaming-writer.md),
  [SQLite container](2026-08-12-rez-sqlite-container.md)) and the crate split
  that preceded it (#1121). Does not change the on-disk `.rez` format for any
  archive rezolus writes today.
- **Owner:** Brian Martin.
- **Repos:** rezolus (`crates/rez/`, `src/`, `Cargo.toml`, `CLAUDE.md`) and the
  new `iopsystems/dendro`.

## Why

The `.rez` container had picked up users outside rezolus. The question was
whether it belonged in `metriken` — and the answer was that it belongs in
neither: what those users want is the *substrate*, not the metrics format.

Parquet is a batch format. A file is readable only once its footer is written,
so a process appending rows continuously has nothing to show for the batch it is
filling and nothing at all if it dies mid-batch. That problem, and the
WAL-plus-segments answer to it, has nothing to do with telemetry. It just
happens that telemetry is where we hit it first.

## Where the cut fell

Narrower than expected, and the code had already found it. Everywhere the
container touched the payload it was the same call — `materialize_wal_tail(key,
rows) -> Option<MaterializedTail>` — from exactly four sites: `seal_batch`,
`reader.rs` twice, and `rez_v3_rewrite`. And `MaterializedTail { rows, first_ts,
bytes }` was already precisely "a sealed segment".

So the boundary is one trait:

```rust
pub trait SegmentEncoder {
    fn encode(&self, stream: &str, rows: &[WalRow]) -> Result<Option<Segment>, String>;
}
```

dendro owns arrow `RecordBatch` ↔ parquet segment ↔ BLOB; the caller owns *how
my rows become a RecordBatch*. Counter/Gauge/Histogram — the one genuinely
metrics-shaped thing in the substrate — stays above the line, in
`rez::wal::RezEncoder`.

| moved to dendro | stayed in `crates/rez` |
|---|---|
| `rez_sqlite.rs` → `db` (container, catalog, retention, `vacuum_into`) | `RezManifest` and the recording/label model |
| `rez_v3_writer.rs:140–945` → `writer` (`Archive`, `RecordingWriter`, checkpoint thread, `seal_batch`) | `StreamRecorderV3` (snapshot ingest, schema cache) |
| `seal_policy.rs` → `seal` | `WalCell`/`WalValue`/`WalGroupRow` + `RezEncoder` |
| `rez_v3_rewrite.rs`'s generic copy → `rewrite` | `table_sampler`, `<sampler>/<group>` keying, the tar v1/v2 writer |
| `segment_writer_props` + the segment codec → `segment` | `RezReader: MetricsSource`, `schema.rs`, `window.rs` |

`metriken` and `metriken-query` stop at `crates/rez`. Neither reaches dendro,
which is what makes it a container rather than a metrics format with a general
name.

## Two decisions worth recording

**The rename reaches the schema.** dendro's stream column is `stream`, not
`sampler`, so the container is schema v4. v3 archives — every `.rez` and every
hindsight buffer written before today — open read-only through per-connection
`CREATE TEMP VIEW`s that shadow the main tables, so every statement in `db.rs`
names `stream` unconditionally and nothing on disk is modified by an open. That
last part matters: the file is often a buffer another process is still
appending to. Writes are refused by an explicit guard rather than by SQLite's
`cannot modify segments because it is a view`, which reads like a bug in the
library.

**Test-only accessors are `test-support`, not `#[cfg(test)]`.** The same lesson
`crates/rez` learned about the binary, one layer down: `finalize_single`,
`SegmentAccount::targets` and the retention internals are all reached by
rezolus's tests, and `#[cfg(test)]` on them is invisible across a crate
boundary.

## What the split is worth, as evidence

Two things went right in a way that says the boundary is real rather than
relocated:

- dendro's `tests/roundtrip.rs` drives the whole write→seal→read→rewrite path on
  a row shape that is an integer and a string. It passed on the first run. A
  container still secretly shaped around counters and histograms would not have.
- After the extraction, rez's suite came back at 159 tests and dendro's lib at
  31 — exactly the 190 that `crates/rez` had before, with none dropped and none
  double-counted, plus dendro's 11 new ones.

And one thing went wrong in a way worth writing down. The blanket
`sampler` → `stream` rename rewrote the compatibility view itself to `sampler AS
stream` → `stream AS stream`, erasing the single place the old name had to
survive. `tests/legacy_v3.rs` — which builds a v3 fixture from raw DDL rather
than checking in a binary — caught it immediately. A fixture written as SQL is
also documentation of the schema being promised.

## Vocabulary

dendro's terms are pinned in a table in both `lib.rs` and its README. Four
things nest — **archive → stream → segment → row** — plus *source*, *WAL*,
*seal*, *tail*, *catalog*, *encoder*.

The first cut had **recording** in that chain, and review caught it: the word
reads file-sized (you record a thing, you get a file), so an archive holding
several recordings, each holding streams, put two boxes where a reader expects
one — and once it looks like a box, the question "then what is a stream?" has no
good answer, because a box inside a box starts to sound like a segment.

It is a **namespace**, not a container. It makes a stream name unambiguous
(`cpu` on web-01 vs `cpu` on web-02) and gives its rows a shared wall-clock
anchor — timestamps are `anchor + monotonic elapsed`, so one of them is one
clock. That is why it could not simply be dissolved into per-stream labels, the
other option considered: 26 streams off one host would each carry their own
anchor with nothing keeping them in agreement. But nothing is stored *in* one
that is not in one of its streams, so it has no business on the ladder.

It is called **source** because that is the question it answers, and because it
is already the label a producer populates.

Naming it exposed a slip on the rezolus side that predates this work. The
mapping is *not* "a `.rez` recording is a dendro source" — a `.rez` **recording
is an archive**, a source is a source, and a sampler is one stream or several
(one per acquisition group under V3). `RezRecording` is misnamed: its fields are
`labels`, `metadata`, `complete`, `clock_anchor_wall_ns`, `clock_offsets`, which
is a source field for field, and `record --endpoint a --endpoint b` produces one
recording holding two *sources*.

rezolus's own CLI already uses the word both ways, which is the tell:

    rezolus recording metadata -i multi.rez --recording source=redis
             ^^^^^^^^^ the archive          ^^^^^^^^^^^ a source

Left alone here deliberately — `--recording` is a shipped flag, so renaming it
to `--source` wants a deprecated alias and its own change. Recorded in CLAUDE.md
so the next person does not re-derive it. Every doc
comment was swept for `sampler`, `rezolus`, `hindsight`, `fleet`, `scrape`,
`metric`, `agent`, `.rez` and the v1/v2/v3-tar migration history. The measured
reasoning survived the move — the 16-of-26 loss at `kill -9`, the 123-append
copy gap, `auto_vacuum` at 8.230 vs 8.807 ms per cycle — relocated into
`DESIGN.md` and attributed to where it was measured. A rezolus word turning up
in dendro is now a bug.

## Diagrams

Three generated charts live in the new repo: the model, the write path, the
read path (`dendro/docs/`, regenerated by `docs/regen.sh`, CI fails on a diff).
Derived rather than drawn — the catalog comes from `SCHEMA_SQL`, the writer's
message kinds from its `Msg` enum, the watermark from `LIVE_WAL_PREDICATE`.

They are the second application of this repo's
[diagram encoding convention](2026-09-02-diagram-encoding-conventions.md), whose
reopen condition was exactly that. Both of its open questions came back yes, and
the finding is recorded there: a style channel has to carry exactly one
category **across the whole set**, not one per chart. The first draft spent
dashes on three meanings and they stopped meaning anything.

## Open

- **dendro CI is unpushed.** `.github/workflows/ci.yml` (fmt, clippy, test,
  `--no-default-features` native, `--no-default-features` wasm32) exists in the
  working tree; pushing it needs `gh auth refresh -h github.com -s workflow`.
- **Pinned by git rev, not published.** The workspace `Cargo.toml` pins a rev
  and carries a commented `[patch]` block for side-by-side development. Publish
  to crates.io once dendro's other consumers have exercised the encoder
  boundary — that is the point at which the trait's shape stops being cheap to
  change.
- **Further untangling, deliberately deferred.** The recording/label model and
  `RezReader` still sit in `crates/rez`. Neither is wrong there, and moving the
  reader would mean moving `metriken-query` into dendro or splitting a third
  crate — a decision better made once there is a second reader to generalize
  against rather than ahead of one.
