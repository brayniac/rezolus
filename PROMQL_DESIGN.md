# PromQL Integration Design for Rezolus Viewer

## Overview
Restructure the Rezolus viewer to use PromQL queries instead of direct Rust method calls, providing a standard query interface that's compatible with Prometheus tooling.

## Architecture Components

### 1. PromQL Query Engine (`src/viewer/promql/`)
```rust
// src/viewer/promql/mod.rs
pub struct QueryEngine {
    tsdb: Arc<Tsdb>,
}

impl QueryEngine {
    /// Execute instant query at a specific time
    pub async fn query(&self, query: &str, time: Option<i64>) -> Result<QueryResult, Error> {
        let expr = promql_parser::parser::parse(query)?;
        self.execute_expr(expr, time)
    }
    
    /// Execute range query over a time range
    pub async fn query_range(
        &self, 
        query: &str, 
        start: i64, 
        end: i64, 
        step: Duration
    ) -> Result<RangeQueryResult, Error> {
        // Implementation
    }
}
```

### 2. Expression Evaluator (`src/viewer/promql/eval.rs`)
Maps PromQL AST nodes to TSDB operations:

```rust
impl QueryEngine {
    fn execute_expr(&self, expr: Expr, time: Option<i64>) -> Result<QueryResult, Error> {
        match expr {
            Expr::VectorSelector(vs) => self.eval_vector_selector(vs, time),
            Expr::AggregateExpr(agg) => self.eval_aggregate(agg, time),
            Expr::BinaryExpr(bin) => self.eval_binary(bin, time),
            Expr::Call(call) => self.eval_function_call(call, time),
            // ... other expression types
        }
    }
    
    fn eval_vector_selector(&self, vs: VectorSelector, time: Option<i64>) -> Result<Vector, Error> {
        // Map metric name to TSDB collections
        let name = vs.name.as_ref().unwrap();
        
        // Convert label matchers to Labels filter
        let labels = self.matchers_to_labels(&vs.matchers);
        
        // Get data from TSDB
        if let Some(counters) = self.tsdb.counters(name, labels) {
            // Convert to Prometheus Vector format
            Ok(self.to_vector(counters, time))
        } else if let Some(gauges) = self.tsdb.gauges(name, labels) {
            Ok(self.to_vector(gauges, time))
        } else {
            Ok(Vector::empty())
        }
    }
}
```

### 3. HTTP API Endpoints (`src/viewer/api/`)
Prometheus-compatible endpoints:

```rust
// src/viewer/api/mod.rs
pub fn routes(engine: Arc<QueryEngine>) -> Router {
    Router::new()
        .route("/api/v1/query", get(instant_query))
        .route("/api/v1/query_range", get(range_query))
        .route("/api/v1/label/:name/values", get(label_values))
        .route("/api/v1/metadata", get(metadata))
        .with_state(engine)
}

async fn instant_query(
    Query(params): Query<InstantQueryParams>,
    State(engine): State<Arc<QueryEngine>>
) -> Result<Json<QueryResponse>, Error> {
    let result = engine.query(&params.query, params.time).await?;
    Ok(Json(QueryResponse {
        status: "success",
        data: result,
    }))
}
```

### 4. Dashboard Refactoring
Replace direct TSDB calls with PromQL queries:

**Before:**
```rust
data.counter_rate_sum("network_bytes", [("direction", "transmit")])
    .map(|v| v * 8.0)
```

**After:**
```javascript
// In JavaScript dashboard code
const query = 'irate(network_bytes{direction="transmit"}[1m]) * 8';
const response = await fetch(`/api/v1/query?query=${encodeURIComponent(query)}`);
```

## Implementation Phases

### Phase 1: Core Query Engine
1. Add `promql-parser` dependency
2. Implement basic expression evaluator
3. Support vector selectors and simple aggregations
4. Map TSDB methods to PromQL operations

### Phase 2: PromQL Functions
Implement common PromQL functions:
- `irate()` - Calculate instantaneous rate of counter increase
- `sum()`, `avg()`, `min()`, `max()` - Aggregations
- `histogram_quantile()` - For percentile calculations
- `increase()`, `delta()` - Value changes
- `by()`, `without()` - Label grouping

### Phase 3: HTTP API
1. Add Prometheus-compatible endpoints
2. JSON serialization for query results
3. Error handling and validation

### Phase 4: Dashboard Migration
1. Update dashboard to use fetch API for queries
2. Convert existing Rust logic to PromQL queries
3. Update chart rendering to handle Prometheus response format

## Example Query Mappings

| Current Rust Code | PromQL Equivalent |
|------------------|-------------------|
| `data.counter_rate_sum("cpu_cycles", ())` | `sum(irate(cpu_cycles[1m]))` |
| `data.counters("network_bytes", [("direction", "transmit")])` | `network_bytes{direction="transmit"}` |
| `data.percentiles("tcp_packet_latency", (), [0.5, 0.99])` | `histogram_quantile(0.5, tcp_packet_latency)` |
| `v.rate().by_name()` | `sum by (name) (irate(metric[1m]))` |

## Benefits

1. **Standard Query Language**: Use industry-standard PromQL
2. **Tool Compatibility**: Works with Grafana, Prometheus tools
3. **Flexibility**: Users can write custom queries
4. **Caching**: Can cache query results
5. **Federation**: Could expose data to external Prometheus servers

## Performance Considerations

1. **Query Compilation**: Parse and compile queries once, cache execution plan
2. **Time Series Index**: Build inverted index for label matching
3. **Batch Operations**: Execute sub-expressions in parallel
4. **Memory Management**: Stream results for large queries

## Migration Strategy

1. Keep existing TSDB methods initially
2. Build PromQL engine alongside current system
3. Gradually migrate dashboards to use PromQL
4. Eventually deprecate direct TSDB access

## Example Implementation Start

```rust
// Cargo.toml
[dependencies]
promql-parser = "0.4"

// src/viewer/promql/mod.rs
use promql_parser::parser::{self, Expr};
use crate::viewer::tsdb::Tsdb;

pub struct QueryEngine {
    tsdb: Arc<Tsdb>,
}

impl QueryEngine {
    pub fn new(tsdb: Arc<Tsdb>) -> Self {
        Self { tsdb }
    }
    
    pub async fn query(&self, query_str: &str) -> Result<QueryResult, Error> {
        let expr = parser::parse(query_str)
            .map_err(|e| Error::ParseError(e.to_string()))?;
        
        match expr {
            Expr::VectorSelector(vs) => {
                // Handle basic metric selection
                let metric_name = vs.name.as_ref()
                    .ok_or_else(|| Error::InvalidQuery("No metric name"))?;
                
                // Convert matchers to our Labels type
                let mut labels = Labels::default();
                for matcher in &vs.matchers {
                    if matcher.op == MatchOp::Equal {
                        labels.inner.insert(
                            matcher.name.clone(),
                            matcher.value.clone()
                        );
                    }
                }
                
                // Query TSDB
                if let Some(collection) = self.tsdb.counters(metric_name, labels) {
                    Ok(QueryResult::Vector(self.collection_to_vector(collection)))
                } else {
                    Ok(QueryResult::Empty)
                }
            }
            _ => Err(Error::NotImplemented("Complex expressions not yet supported"))
        }
    }
}
```

This design provides a clean path to modernize the viewer while maintaining backward compatibility during the transition.