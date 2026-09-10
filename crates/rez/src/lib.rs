//! The `.rez` archive format: container, writers, and reader.
//!
//! Extracted from the `rezolus` binary so the WASM viewer can read the same
//! archives the binary writes. `rezolus` is a binary-only crate with no `lib`
//! target, so nothing could depend on it; a `.rez` reader living there was
//! reachable by the server viewer and by nothing else, which is why the
//! static-site viewer has never opened a `.rez` at all.
//!
//! Nothing here knows about samplers, endpoints, or the CLI — it speaks in
//! recordings, tables, segments, and bytes.

/// Reshape a plain parquet into `.rez` recordings (metriken-free). Not behind
/// `write` — the browser assembles `.rez` reports from uploaded parquet bytes.
pub mod parquet_ingest;
pub mod reader;
/// The container, now `dendro`. Re-exported under the path call sites already
/// use, and under the names they already say.
pub mod rez_sqlite {
    pub use dendro::db::{
        Db as RezDb, Evicted, SegmentMeta, SegmentRow, SourceMeta, SourceRow, Span, Tx as RezTx,
        WalRow, PAGE_SIZE,
    };
}
/// Segment sealing policy, now `dendro`.
pub mod seal_policy {
    pub use dendro::seal::*;
}
pub mod rez;
/// The tar (v1/v2) `.rez` writer, kept only so tests can build v1/v2 fixtures.
/// Nothing ships that writes this container any more.
#[cfg(any(test, feature = "test-support"))]
pub mod rez_stream;
/// v3 rewrite tooling (`combine`/`filter`/`upgrade`) and report assembly.
///
/// Not behind `write`: everything here is metriken-free (catalog `UPDATE`s,
/// verbatim BLOB copies, arrow-level segment projection, WAL-tail
/// materialization), so the browser viewer can assemble/trim a `.rez` report
/// in the reader build. The metriken-dependent ingest (agent snapshot → rows)
/// lives in `rez_v3_writer`, which stays `write`-gated.
pub mod rez_v3_rewrite;
/// The v3 streaming writer.
#[cfg(feature = "write")]
pub mod rez_v3_writer;
pub mod schema;
pub mod wal;
pub mod window;
