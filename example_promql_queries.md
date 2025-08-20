# PromQL Label Filtering Examples

The PromQL engine now supports label filtering for dashboard queries. Here are examples of the queries that can be used:

## Basic Queries

### Simple metric access:
```promql
cpu_cores
```

### Metric with label filtering:
```promql
cpu_cores{cpu="0"}
```

## Rate Functions

### Basic rate query:
```promql
irate(cpu_cycles[5m])
```

### Rate with label filtering (equivalent to dashboard `counter_rate_sum("network_bytes", [("direction", "transmit")])`):
```promql
irate(network_bytes{direction="transmit"}[5m])
```

### Sum of rate with filtering (equivalent to dashboard `counter_rate_sum("blockio_bytes", [("op", "read")])`):
```promql
sum(irate(blockio_bytes{op="read"}[1m]))
```

## Dashboard Migration Examples

### Network Bandwidth Transmit:
**Old Dashboard Code:**
```rust
data.counter_rate_sum("network_bytes", [("direction", "transmit")])
    .map(|v| v * 8.0)
```

**New PromQL Query:**
```promql
sum(irate(network_bytes{direction="transmit"}[5m])) * 8
```

### Block I/O Read Throughput:
**Old Dashboard Code:**
```rust
data.counter_rate_sum("blockio_bytes", [("op", "read")])
```

**New PromQL Query:**
```promql
sum(irate(blockio_bytes{op="read"}[5m]))
```

### CPU Migrations:
**Old Dashboard Code:**
```rust
data.counter_rate_sum("cpu_migrations", [("direction", "to")])
```

**New PromQL Query:**
```promql
sum(irate(cpu_migrations{direction="to"}[5m]))
```

## Label Syntax

The engine supports:
- Multiple labels: `metric{label1="value1",label2="value2"}`
- Single or double quotes: `metric{label='value'}` or `metric{label="value"}`
- Spaces in values: `metric{label="value with spaces"}`
- Spaces around operators: `metric{label = "value"}`

## API Endpoints

- **Instant Query:** `GET /api/v1/query?query=<promql>&time=<timestamp>`
- **Range Query:** `GET /api/v1/query_range?query=<promql>&start=<timestamp>&end=<timestamp>&step=<seconds>`

The PromQL engine provides a foundation for migrating dashboard code from direct Rust method calls to standardized PromQL queries, enabling better flexibility and compatibility with Prometheus tooling.