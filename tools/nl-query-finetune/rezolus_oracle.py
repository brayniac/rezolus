"""rezolus_oracle.py — thin wrapper around the `rezolus` binary.

Two jobs:
  1. dump_schema():   build structured "metric cards" (name, type, unit, labels +
                      observed values, description) from one or more parquet files.
  2. query()/valid(): execute a PromQL query against a parquet and parse the result,
                      so the data generator can VALIDATE every gold query and the eval
                      harness can compare gold vs predicted by behaviour.

The binary is the single source of truth for both the schema and PromQL semantics.

NOTE: `rezolus mcp query` exits 0 even on failure — failure is signalled by a
`Query failed:` line on stdout. We parse stdout, never the exit code.

Stdlib only (no third-party deps) so Phases 0–1 and eval run anywhere.
"""

from __future__ import annotations

import atexit
import json
import os
import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# Metadata keys on a parquet column that are NOT labels.
_NON_LABEL_KEYS = {"metric", "metric_type", "unit"}

# Provenance / recording / histogram-config "labels" that are not semantic
# dimensions a user would query on. Excluded from metric cards so the data
# generator never builds "by endpoint" / "{source=...}" nonsense. `node` is
# excluded too — the viewer injects node scoping separately, so the model
# should never emit it.
EXCLUDE_LABELS = {
    "grouping_power", "max_value_power",   # H2 histogram config
    "instance", "endpoint", "source", "node",  # recording / topology provenance
}


def _default_binary() -> str:
    """Resolve the rezolus binary: $REZOLUS_BIN, then repo-local debug/release, then PATH."""
    env = os.environ.get("REZOLUS_BIN")
    if env:
        return env
    here = Path(__file__).resolve()
    # tools/nl-query-finetune/rezolus_oracle.py -> repo root is three parents up.
    repo = here.parents[2]
    for cand in (repo / "target/release/rezolus", repo / "target/debug/rezolus"):
        if cand.exists():
            return str(cand)
    return "rezolus"


@dataclass
class MetricCard:
    name: str
    type: str                       # counter | gauge | histogram
    unit: str = ""
    description: str = ""
    labels: dict = field(default_factory=dict)   # label_key -> sorted list of observed values
    parquets: set = field(default_factory=set)   # files this metric was seen in

    def label_keys(self) -> list:
        return sorted(self.labels.keys())

    def to_json(self) -> dict:
        return {
            "name": self.name,
            "type": self.type,
            "unit": self.unit,
            "description": self.description,
            "labels": {k: sorted(v) for k, v in self.labels.items()},
            "parquets": sorted(self.parquets),
        }


@dataclass
class Series:
    labels: dict
    points: int
    vmin: Optional[float]
    vmax: Optional[float]
    mean: Optional[float]


@dataclass
class QueryResult:
    ok: bool
    error: str = ""
    series: list = field(default_factory=list)   # list[Series]

    @property
    def empty(self) -> bool:
        return self.ok and (not self.series or all(s.points == 0 for s in self.series))


class _McpSession:
    """Persistent `rezolus mcp` stdio server (newline-delimited JSON-RPC).

    The server caches each parquet's TSDB + QueryEngine, so the first query for a
    file loads it and every later query is warm — vs. the one-shot `mcp query`
    CLI, which reloads the whole parquet per call. One session per Oracle/process.
    Any failure flips `dead` and the Oracle falls back to the subprocess path.
    """

    def __init__(self, binary: str):
        self.binary = binary
        self.proc: Optional[subprocess.Popen] = None
        self.dead = False
        self._id = 0

    def _ensure(self) -> bool:
        if self.proc is not None:
            return True
        if self.dead:
            return False
        try:
            self.proc = subprocess.Popen(
                [self.binary, "mcp"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL, text=True, bufsize=1,
            )
            atexit.register(self.close)
            self._rpc("initialize", {})  # handshake; raises on failure
            return True
        except Exception:
            self.dead = True
            self.close()
            return False

    def _rpc(self, method: str, params: dict) -> dict:
        self._id += 1
        self.proc.stdin.write(json.dumps(
            {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params}) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("mcp server closed the connection")
        return json.loads(line)

    def query(self, parquet: str, promql: str) -> "QueryResult":
        """Run a query via the server. Returns None if the session is unusable."""
        if not self._ensure():
            return None
        try:
            resp = self._rpc("tools/call", {
                "name": "query",
                "arguments": {"parquet_file": str(parquet), "query": promql},
            })
        except Exception:
            self.dead = True
            self.close()
            return None
        if "error" in resp:
            msg = resp["error"].get("message", "")
            # Align with the subprocess path: strip the "Query error: " wrapper so
            # parse errors still read "Parse error: ..." for parses().
            msg = re.sub(r"^Query error:\s*", "", msg).strip()
            return QueryResult(ok=False, error=msg)
        try:
            text = resp["result"]["content"][0]["text"]
            return _parse_query_json(json.loads(text))
        except Exception:
            self.dead = True
            self.close()
            return None

    def close(self):
        if self.proc is not None:
            try:
                self.proc.stdin.close()
            except Exception:
                pass
            try:
                self.proc.terminate()
            except Exception:
                pass
            self.proc = None


def _parse_query_json(obj: dict) -> "QueryResult":
    """Parse a serialized QueryResult (matrix/vector/scalar) into Series.

    Stats mirror src/mcp/mod.rs::format_query_result exactly: points = len(values),
    min/max via fold, mean = sum/len — so server and subprocess paths agree.
    """
    rtype = obj.get("resultType")
    series = []
    if rtype == "matrix":
        for s in obj.get("result", []):
            vals = [v for _, v in s.get("values", [])]
            series.append(_series_from(s.get("metric", {}), vals))
    elif rtype == "vector":
        for s in obj.get("result", []):
            v = s.get("value")
            series.append(_series_from(s.get("metric", {}), [v[1]] if v else []))
    elif rtype == "scalar":
        r = obj.get("result")
        series.append(_series_from({}, [r[1]] if r else []))
    # histogram heatmap / unknown → no comparable series (treated as empty)
    return QueryResult(ok=True, series=series)


def _series_from(metric: dict, vals: list) -> "Series":
    labels = {str(k): str(v) for k, v in metric.items()}
    if not vals:
        return Series(labels=labels, points=0, vmin=None, vmax=None, mean=None)
    fvals = [float(v) for v in vals]
    return Series(labels=labels, points=len(fvals), vmin=min(fvals),
                  vmax=max(fvals), mean=sum(fvals) / len(fvals))


class Oracle:
    def __init__(self, binary: Optional[str] = None):
        self.binary = binary or _default_binary()
        # Persistent MCP server keeps parquets warm; disabled with
        # REZOLUS_ORACLE_SERVER=0 (then every query reloads via the CLI).
        self._session = (_McpSession(self.binary)
                         if os.environ.get("REZOLUS_ORACLE_SERVER", "1") != "0" else None)

    # ── schema ────────────────────────────────────────────────────────────
    def dump_schema(self, parquets) -> dict:
        """Aggregate metric cards across the given parquet files."""
        cards: dict = {}
        for pq in parquets:
            pq = str(pq)
            meta = self._parquet_metadata_json(pq)
            for col in meta.get("schema", []):
                md = col.get("metadata", {})
                name = md.get("metric")
                mtype = md.get("metric_type")
                if not name or mtype in (None, "timestamp", "duration"):
                    continue
                card = cards.get(name)
                if card is None:
                    card = cards[name] = MetricCard(name=name, type=mtype, unit=md.get("unit", ""))
                card.parquets.add(Path(pq).name)
                for k, v in md.items():
                    if k in _NON_LABEL_KEYS or k in EXCLUDE_LABELS:
                        continue
                    card.labels.setdefault(k, set()).add(str(v))
            # Descriptions only live in the human `describe-metrics` text.
            for name, desc in self._describe_descriptions(pq).items():
                if name in cards and not cards[name].description:
                    cards[name].description = desc
        return cards

    def _parquet_metadata_json(self, parquet: str) -> dict:
        out = subprocess.run(
            [self.binary, "parquet", "metadata", "-i", parquet, "--json"],
            capture_output=True, text=True,
        )
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise RuntimeError(f"parquet metadata --json failed for {parquet}:\n{out.stdout[:500]}\n{out.stderr[:500]}")

    def _describe_descriptions(self, parquet: str) -> dict:
        """Parse `mcp describe-metrics` text for {metric_name: description}."""
        out = subprocess.run(
            [self.binary, "mcp", "describe-metrics", parquet],
            capture_output=True, text=True,
        ).stdout
        descs: dict = {}
        cur = None
        for line in out.splitlines():
            m = re.match(r"^\s*[•*]\s+([A-Za-z0-9_]+)\s*$", line)
            if m:
                cur = m.group(1)
                continue
            d = re.match(r"^\s*Description:\s*(.+?)\s*$", line)
            if d and cur:
                descs[cur] = d.group(1)
                cur = None
        return descs

    # ── query / validation ────────────────────────────────────────────────
    def query(self, parquet: str, promql: str) -> QueryResult:
        # Fast path: persistent MCP server (parquet stays loaded across queries).
        if self._session is not None:
            r = self._session.query(parquet, promql)
            if r is not None:
                return r
            self._session = None  # session died → fall back for the rest of the run
        return self._query_subprocess(parquet, promql)

    def _query_subprocess(self, parquet: str, promql: str) -> QueryResult:
        out = subprocess.run(
            [self.binary, "mcp", "query", str(parquet), promql],
            capture_output=True, text=True,
        ).stdout
        fail = re.search(r"Query failed:\s*(.*)", out)
        if fail:
            return QueryResult(ok=False, error=fail.group(1).strip())
        return QueryResult(ok=True, series=self._parse_series(out))

    def valid(self, parquet: str, promql: str) -> bool:
        """A gold query is valid iff it executes AND returns at least one non-empty series."""
        r = self.query(parquet, promql)
        return r.ok and not r.empty

    @staticmethod
    def _parse_series(out: str) -> list:
        """Parse the `mcp query` range-vector text block into Series objects."""
        series = []
        cur_labels = None
        info = {}

        def flush():
            nonlocal cur_labels, info
            if cur_labels is not None:
                series.append(Series(
                    labels=cur_labels,
                    points=int(info.get("points", 0)),
                    vmin=info.get("min"), vmax=info.get("max"), mean=info.get("mean"),
                ))
            cur_labels, info = None, {}

        for line in out.splitlines():
            head = re.match(r"^\s*(\{.*\}|\{\}):\s*$", line)
            if head:
                flush()
                cur_labels = _parse_labels(head.group(1))
                continue
            pm = re.search(r"Time series with (\d+) points", line)
            if pm:
                info["points"] = pm.group(1)
                continue
            for key in ("Min", "Max", "Mean"):
                m = re.match(rf"^\s*{key}:\s*([-\d.eE+]+)\s*$", line)
                if m:
                    info[key.lower()] = float(m.group(1))
        flush()
        return series


def _parse_labels(blob: str) -> dict:
    """'{state="user",id="3"}' -> {'state': 'user', 'id': '3'};  '{}' -> {}."""
    inner = blob.strip()[1:-1]
    out = {}
    for m in re.finditer(r'([A-Za-z0-9_]+)="([^"]*)"', inner):
        out[m.group(1)] = m.group(2)
    return out
