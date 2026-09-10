//! The rez-specific half of archive rewriting: which columns a projection must
//! keep, and upgrading a v1/v2 tar `.rez` into the v3 container.
//!
//! The container-level copy — catalog `UPDATE`s, verbatim BLOB copies,
//! arrow-level segment projection — is `dendro::rewrite`. What is here is the
//! part that knows what a `.rez` column is for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dendro::db::{Db as RezDb, SegmentMeta, SourceMeta, Tx as RezTx};
use dendro::rewrite::{ColumnFilter, CopySpec as DendroCopySpec};

use crate::rez::table_sampler;

/// A `.rez` column filter: keep the structural columns unconditionally, plus
/// the value columns (and their per-metric window sidecars) for the named
/// metrics.
pub struct RezColumns<'a>(pub &'a BTreeSet<String>);

impl ColumnFilter for RezColumns<'_> {
    fn keep(&self, field: &arrow::datatypes::Field) -> bool {
        keep_rez_column(field, self.0)
    }
    fn is_data(&self, field: &arrow::datatypes::Field) -> bool {
        is_value_column(field.name())
    }
}

/// What one copy pass carries across, in `.rez` terms.
///
/// The sampler/metric vocabulary is this layer's; [`CopySpec::to_dendro`]
/// lowers it to the container's predicates.
pub struct CopySpec<'a> {
    pub start: u64,
    pub end: u64,
    /// Keep only tables whose *sampler* — the part of a `<sampler>/<group>`
    /// key before the first `/` — is in this set. `None` keeps every table.
    ///
    /// Filtering by sampler rather than by table key is deliberate: a sampler
    /// is the unit an operator names, and under V3 one sampler owns several
    /// group tables. Dropping "the sampler" has to drop all of its groups.
    pub keep_samplers: Option<&'a BTreeSet<String>>,
    pub metadata_extra: Option<&'a BTreeMap<String, String>>,
    /// Project each copied segment down to these metrics' columns.
    pub keep_metrics: Option<&'a BTreeSet<String>>,
}

impl CopySpec<'_> {
    /// Every recording, every table, every row, metadata untouched.
    pub fn everything() -> Self {
        CopySpec {
            start: 0,
            end: u64::MAX,
            keep_samplers: None,
            metadata_extra: None,
            keep_metrics: None,
        }
    }
}

/// Copy every recording in `src` into the open destination transaction.
///
/// A thin lowering onto [`dendro::rewrite::copy_sources_into`]: the sampler
/// filter becomes a stream predicate and the metric filter a [`ColumnFilter`].
pub fn copy_sources_into(
    src: &RezDb,
    tx: &RezTx<'_>,
    spec: &CopySpec<'_>,
) -> Result<usize, String> {
    let keep_sampler = spec
        .keep_samplers
        .map(|keep| move |table: &str| keep.contains(table_sampler(table)));
    let keep_columns = spec.keep_metrics.map(RezColumns);
    let lowered = DendroCopySpec {
        start: crate::wal::dendro_ts_bound(spec.start),
        end: crate::wal::dendro_ts_bound(spec.end),
        keep_streams: keep_sampler.as_ref().map(|f| f as &dyn Fn(&str) -> bool),
        metadata_extra: spec.metadata_extra,
        keep_columns: keep_columns.as_ref().map(|c| c as &dyn ColumnFilter),
    };
    dendro::rewrite::copy_sources_into(src, tx, &lowered, &crate::wal::RezEncoder)
        .map_err(String::from)
}

/// Project one segment's parquet down to the columns for `keep_metrics` plus
/// the structural sidecars, re-encoding it with the archive's own writer
/// properties so the result is indistinguishable from a natively-sealed
/// segment (LZ4_RAW, no dictionary, default row groups — NOT report-save's
/// ZSTD).
///
/// A column projection changes neither the row count nor the timestamps nor
/// the windows, so the caller reuses the segment's existing `SegmentMeta`
/// unchanged. Returns `None` when no value column survives — the table holds
/// none of the kept metrics and should be dropped rather than reduced to bare
/// structural columns.
///
/// This is the one place the rewrite tools decode a segment: `combine`,
/// `filter --samplers` and `annotate` all move BLOBs verbatim, but a
/// per-column trim cannot. What `.rez` supplies is [`RezColumns`] — which
/// columns are structural is this layer's knowledge, not the container's.
pub fn project_segment_columns(
    bytes: &[u8],
    keep_metrics: &BTreeSet<String>,
) -> Result<Option<Vec<u8>>, String> {
    dendro::rewrite::project_segment_columns(bytes, &RezColumns(keep_metrics)).map_err(String::from)
}

fn is_structural_column(name: &str) -> bool {
    name == "timestamp"
        || name == crate::rez::WALL_OFFSET_COLUMN
        || name == crate::rez::WINDOW_BEGIN_COLUMN
        || name == crate::rez::WINDOW_WIDTH_COLUMN
}

/// The metric a per-metric window sidecar (`<m>:window_begin` /
/// `<m>:window_width`, a V2-derived table) belongs to. `None` for the bare
/// table-level pair (empty prefix) and for non-window columns.
fn per_metric_window_owner(name: &str) -> Option<&str> {
    name.strip_suffix(":window_begin")
        .or_else(|| name.strip_suffix(":window_width"))
        .filter(|base| !base.is_empty())
}

/// A column carrying actual metric values — not timestamp, offset, or any
/// window sidecar. The presence of at least one decides whether a table
/// survives a metric projection at all.
fn is_value_column(name: &str) -> bool {
    !is_structural_column(name) && per_metric_window_owner(name).is_none()
}

/// Whether a column survives a projection down to `keep_metrics`. Structural
/// columns always do; a per-metric window rides its metric; a value column is
/// matched by exact name, by the base before `:` (`foo` for `foo:buckets`), or
/// by the `metric` metadata fallback (Prometheus numeric-id columns).
fn keep_rez_column(f: &arrow::datatypes::Field, keep_metrics: &BTreeSet<String>) -> bool {
    let name = f.name();
    if is_structural_column(name) {
        return true;
    }
    if let Some(metric) = per_metric_window_owner(name) {
        return keep_metrics.contains(metric);
    }
    keep_metrics.contains(name)
        || name
            .split_once(':')
            .is_some_and(|(base, _)| keep_metrics.contains(base))
        || f.metadata()
            .get("metric")
            .is_some_and(|m| keep_metrics.contains(m))
}

fn segment_catalog_facts(bytes: &[u8]) -> Result<Option<SegmentMeta>, String> {
    use arrow::array::{Array, UInt64Array};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use parquet::arrow::ProjectionMask;

    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::copy_from_slice(bytes))
        .map_err(|e| format!("failed to open a segment: {e}"))?;
    let ts_idx = builder
        .parquet_schema()
        .columns()
        .iter()
        .position(|c| c.name() == "timestamp")
        .ok_or_else(|| "segment has no `timestamp` column".to_string())?;
    let mask = ProjectionMask::leaves(builder.parquet_schema(), [ts_idx]);
    let reader = builder
        .with_projection(mask)
        .build()
        .map_err(|e| format!("failed to read a segment's timestamps: {e}"))?;

    let mut rows = 0u64;
    let mut first: Option<u64> = None;
    let mut last: Option<u64> = None;
    for batch in reader {
        let batch = batch.map_err(|e| format!("failed to read a segment's timestamps: {e}"))?;
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| "segment `timestamp` column is not UInt64".to_string())?;
        for i in 0..col.len() {
            if col.is_null(i) {
                continue;
            }
            let ts = col.value(i);
            first.get_or_insert(ts);
            last = Some(ts);
            rows += 1;
        }
    }
    match (first, last) {
        // An empty segment carries no time span, so it has nothing to catalog
        // and is dropped rather than inserted with a fabricated one.
        (Some(first_ts), Some(last_ts)) => Ok(Some(SegmentMeta {
            rows,
            first_ts: crate::wal::dendro_ts(first_ts)?,
            last_ts: crate::wal::dendro_ts(last_ts)?,
        })),
        _ => Ok(None),
    }
}

/// One table's segments, each paired with the catalog facts read from it.
type CatalogedTable<'a> = (&'a str, Vec<(SegmentMeta, &'a Vec<u8>)>);

/// Convert a v1/v2 (tar) `.rez` archive into a v3 (SQLite) one.
///
/// Segment parquet BLOBs are carried across byte-identical — the container
/// changes, the data does not. v1 and v2 come through the same path because
/// they differ only in how the manifest names a table's segments (`file` vs
/// `files`), which the reader already normalizes.
///
/// The whole archive is held in memory during the conversion, because the v2
/// reader materializes it that way. That is the same footprint `parquet
/// combine` has always had on a v2 input, but it does bound the size of
/// archive this can upgrade in one pass.
pub fn upgrade_tar_to_v3(src: &Path, dest: &Path) -> Result<usize, String> {
    use crate::rez;

    let (manifest, recordings) = rez::read_archive_bytes(src)
        .map_err(|e| format!("failed to read {}: {e}", src.display()))?;

    let mut db = RezDb::create(dest)?;
    let mut complete_ids: Vec<i64> = Vec::new();
    let count = db.transaction(|tx| {
        let mut n = 0usize;
        for (entry, rb) in manifest.recordings.iter().zip(recordings.iter()) {
            // Catalog every segment first: a v1 manifest has no clock anchor,
            // and the earliest row is the only truthful stand-in for one.
            let mut cataloged: Vec<CatalogedTable<'_>> = Vec::new();
            let mut earliest: Option<i64> = None;
            for (sampler, segments) in &rb.tables {
                let mut kept = Vec::new();
                for bytes in segments {
                    if let Some(meta) = segment_catalog_facts(bytes)? {
                        earliest = Some(earliest.map_or(meta.first_ts, |e| e.min(meta.first_ts)));
                        kept.push((meta, bytes));
                    }
                }
                cataloged.push((sampler.as_str(), kept));
            }

            let id = tx.insert_source(&SourceMeta {
                labels: rb.labels.clone(),
                metadata: rb.metadata.clone(),
                clock_anchor_wall_ns: entry
                    .clock_anchor_wall_ns
                    .map(crate::wal::dendro_ts)
                    .transpose()?
                    .or(earliest)
                    .unwrap_or_default(),
            })?;
            n += 1;
            if rb.complete {
                complete_ids.push(id);
            }

            for (sampler, segments) in cataloged {
                for (seq, (meta, bytes)) in segments.into_iter().enumerate() {
                    tx.insert_segment(id, sampler, seq as u64, &meta, bytes)?;
                }
            }
        }
        Ok(n)
    })?;

    // Faithfully, not unconditionally: a v2 archive recovered from a
    // checkpoint rather than cleanly finalized presents as incomplete, and an
    // upgrade must not launder that into a clean one.
    for id in complete_ids {
        db.mark_complete(id)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use crate::rez_sqlite::RezDb;
    use std::collections::BTreeSet;
    /// A tar archive upgrades to v3 with its data and its identity intact:
    /// segment BLOBs byte-for-byte, labels, metadata, and — the one most
    /// easily lost — the `complete` flag, so a recording recovered from a
    /// checkpoint still reads as recovered afterwards.
    #[test]
    fn upgrading_a_tar_archive_carries_data_labels_and_completeness() {
        use crate::rez;
        use crate::rez_stream::write_segmented_rez;

        let d = tempfile::tempdir().unwrap();
        // One cleanly finalized, one recovered from a checkpoint.
        let clean = write_segmented_rez(
            &d.path().join("clean.rez"),
            "rezolus",
            [("arm".to_string(), "baseline".to_string())]
                .into_iter()
                .collect(),
            &["cpu_usage", "scheduler"],
            6,
            2,
            true,
        );
        let dirty = write_segmented_rez(
            &d.path().join("dirty.rez"),
            "rezolus",
            [("arm".to_string(), "experiment".to_string())]
                .into_iter()
                .collect(),
            &["cpu_usage"],
            6,
            2,
            false,
        );

        for (src, arm, expect_complete) in
            [(&clean, "baseline", true), (&dirty, "experiment", false)]
        {
            let before = rez::read_archive_bytes(src).unwrap().1.remove(0).tables;
            let out = d.path().join(format!("{arm}-v3.rez"));
            let n = super::upgrade_tar_to_v3(src, &out).unwrap();
            assert_eq!(n, 1);

            assert_eq!(
                rez::detect_rez_format(&out).unwrap(),
                rez::RezFormat::V3Sqlite
            );
            let db = RezDb::open(&out).unwrap();
            let recordings = db.read_sources().unwrap();
            assert_eq!(recordings.len(), 1);
            assert_eq!(
                recordings[0].meta.labels.get("arm").map(String::as_str),
                Some(arm),
                "labels come across"
            );
            assert_eq!(
                recordings[0].complete, expect_complete,
                "`complete` is a property of the data and must survive the upgrade — \
                 laundering a recovered recording into a clean one would hide the loss"
            );

            // Segment BLOBs verbatim, in order.
            for (stream, segments) in &before {
                let got = db.read_segments(recordings[0].id, stream).unwrap();
                assert_eq!(
                    got.iter().map(|s| s.bytes.clone()).collect::<Vec<_>>(),
                    *segments,
                    "{stream} segments must be carried byte-for-byte"
                );
                assert!(
                    got.iter().all(|s| s.meta.first_ts <= s.meta.last_ts),
                    "{stream} segment spans must be cataloged from the parquet itself"
                );
            }
        }
    }

    // ── column projection ──

    /// Build a segment parquet with two value columns, the table-level window
    /// pair, timestamp and wall-offset — a minimal V3 group table shape — and
    /// return its bytes plus the row count.
    fn two_metric_segment() -> (Vec<u8>, usize) {
        use arrow::array::{Int64Array, UInt64Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use std::sync::Arc;

        let rows = 4usize;
        let ts: Vec<u64> = (0..rows as u64).map(|i| 1_000 + i).collect();
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::UInt64, false),
            Field::new(crate::rez::WALL_OFFSET_COLUMN, DataType::Int64, true),
            Field::new(crate::rez::WINDOW_BEGIN_COLUMN, DataType::Int64, true),
            Field::new(crate::rez::WINDOW_WIDTH_COLUMN, DataType::UInt64, true),
            Field::new("cpu_usage_busy", DataType::UInt64, true),
            Field::new("cpu_usage_ops", DataType::UInt64, true),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(UInt64Array::from(ts.clone())),
                Arc::new(Int64Array::from(vec![0i64; rows])),
                Arc::new(Int64Array::from(
                    ts.iter().map(|&t| t as i64).collect::<Vec<_>>(),
                )),
                Arc::new(UInt64Array::from(vec![50u64; rows])),
                Arc::new(UInt64Array::from(vec![7u64; rows])),
                Arc::new(UInt64Array::from(vec![9u64; rows])),
            ],
        )
        .unwrap();

        let mut buf = Vec::new();
        {
            let mut w =
                ArrowWriter::try_new(&mut buf, schema, Some(crate::rez::segment_writer_props()))
                    .unwrap();
            w.write(&batch).unwrap();
            w.close().unwrap();
        }
        (buf, rows)
    }

    fn segment_columns(bytes: &[u8]) -> Vec<String> {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        let b =
            ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::copy_from_slice(bytes)).unwrap();
        b.schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    /// A projection keeps the requested metric's value column plus every
    /// structural sidecar (timestamp, wall-offset, the table-level window
    /// pair), drops the unrequested metric, and preserves the row count.
    #[test]
    fn projecting_keeps_requested_metric_and_all_structural_columns() {
        let (bytes, rows) = two_metric_segment();
        let keep: BTreeSet<String> = ["cpu_usage_ops".to_string()].into_iter().collect();

        let projected = super::project_segment_columns(&bytes, &keep)
            .unwrap()
            .expect("a kept metric survives, so the table is not dropped");
        let cols = segment_columns(&projected);

        assert!(
            cols.contains(&"cpu_usage_ops".to_string()),
            "kept: {cols:?}"
        );
        assert!(
            !cols.contains(&"cpu_usage_busy".to_string()),
            "the unrequested metric is dropped: {cols:?}"
        );
        for structural in [
            "timestamp",
            crate::rez::WALL_OFFSET_COLUMN,
            crate::rez::WINDOW_BEGIN_COLUMN,
            crate::rez::WINDOW_WIDTH_COLUMN,
        ] {
            assert!(
                cols.contains(&structural.to_string()),
                "structural column {structural} must survive: {cols:?}"
            );
        }

        // Row count is unchanged by a column projection.
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        let reader = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(projected))
            .unwrap()
            .build()
            .unwrap();
        let got: usize = reader.map(|b| b.unwrap().num_rows()).sum();
        assert_eq!(got, rows, "projection drops columns, never rows");
    }

    /// A table holding none of the kept metrics projects to no value column and
    /// is signalled for dropping (`None`) rather than reduced to bare sidecars.
    #[test]
    fn projecting_a_table_with_no_kept_metric_returns_none() {
        let (bytes, _) = two_metric_segment();
        let keep: BTreeSet<String> = ["something_else".to_string()].into_iter().collect();
        assert!(
            super::project_segment_columns(&bytes, &keep)
                .unwrap()
                .is_none(),
            "no value column survives, so the table is dropped"
        );
    }

    /// A per-metric window sidecar (`<m>:window_begin`) rides its metric: kept
    /// when the metric is kept, dropped when it is not.
    #[test]
    fn projecting_keeps_per_metric_window_only_for_kept_metrics() {
        use arrow::array::{Int64Array, UInt64Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use std::sync::Arc;

        let rows = 3usize;
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::UInt64, false),
            Field::new("a", DataType::UInt64, true),
            Field::new("a:window_begin", DataType::Int64, true),
            Field::new("a:window_width", DataType::UInt64, true),
            Field::new("b", DataType::UInt64, true),
            Field::new("b:window_begin", DataType::Int64, true),
            Field::new("b:window_width", DataType::UInt64, true),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(UInt64Array::from(vec![1u64, 2, 3])),
                Arc::new(UInt64Array::from(vec![10u64; rows])),
                Arc::new(Int64Array::from(vec![0i64; rows])),
                Arc::new(UInt64Array::from(vec![5u64; rows])),
                Arc::new(UInt64Array::from(vec![20u64; rows])),
                Arc::new(Int64Array::from(vec![0i64; rows])),
                Arc::new(UInt64Array::from(vec![5u64; rows])),
            ],
        )
        .unwrap();
        let mut buf = Vec::new();
        {
            let mut w =
                ArrowWriter::try_new(&mut buf, schema, Some(crate::rez::segment_writer_props()))
                    .unwrap();
            w.write(&batch).unwrap();
            w.close().unwrap();
        }

        let keep: BTreeSet<String> = ["a".to_string()].into_iter().collect();
        let projected = super::project_segment_columns(&buf, &keep)
            .unwrap()
            .unwrap();
        let cols = segment_columns(&projected);
        assert!(cols.contains(&"a".to_string()));
        assert!(
            cols.contains(&"a:window_begin".to_string()),
            "kept metric's window rides it: {cols:?}"
        );
        assert!(cols.contains(&"a:window_width".to_string()));
        assert!(
            !cols.contains(&"b".to_string()),
            "dropped metric gone: {cols:?}"
        );
        assert!(
            !cols.contains(&"b:window_begin".to_string()),
            "a dropped metric's window is dropped too: {cols:?}"
        );
    }
}
