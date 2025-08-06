# Chart Definition Architecture Comparison

## Current Approach (Imperative)

```rust
// In cpu.rs
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    let mut view = View::new(data, sections);
    let mut utilization = Group::new("Utilization", "utilization");
    
    // Manually create each chart
    utilization.push(Plot::line(
        "Busy %",  // No context about what this means
        "busy-pct",
        Unit::Percentage,
        data.cpu_avg("cpu_usage", ()).map(|v| (v / 1000000000.0)),
    ));
    
    // Lots of imperative code...
    view.group(utilization);
}
```

**Problems:**
- Charts scattered across multiple files
- No metadata or descriptions
- Hard to discover all available charts
- Titles lack context ("Busy %" vs "CPU Busy %")
- No searchability or keywords

## Proposed Approach 1: Derive Macros + Distributed Slices

```rust
#[chart(
    short_title = "CPU Busy",
    long_title = "CPU Busy Percentage", 
    description = "Percentage of CPU time spent executing processes",
    keywords = ["cpu", "usage", "utilization", "busy"],
    use_cases = "Monitor for performance bottlenecks",
    section = Cpu,
    group = "utilization",
)]
pub struct CpuBusyChart;

impl ChartDefinition for CpuBusyChart {
    fn generate(&self, data: &Tsdb) -> Option<Plot> {
        Plot::line(
            self.metadata().long_title,
            "cpu-busy",
            Unit::Percentage,
            data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
        )
    }
}
```

**Benefits:**
- Declarative with rich metadata
- Automatically collected via distributed_slice
- Type-safe and compile-time checked
- Self-documenting

## Proposed Approach 2: Builder Pattern (Works Today)

```rust
registry.register(
    ChartBuilder::new("CPU Busy")
        .long_title("CPU Busy Percentage")
        .description("Percentage of CPU time spent executing processes")
        .keywords(vec!["cpu", "usage", "utilization"])
        .section("cpu")
        .group("utilization")
        .generator(|data| {
            Plot::line(
                "CPU Busy Percentage",
                "cpu-busy",
                Unit::Percentage,
                data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
            )
        })
        .build()
);
```

**Benefits:**
- Can implement today without proc macros
- Still declarative and metadata-rich
- Central registry for all charts
- Easy to export metadata

## Frontend Benefits

With enriched metadata, the frontend can:

```javascript
// Available chart metadata
{
  "short_title": "CPU Busy",
  "long_title": "CPU Busy Percentage",
  "description": "Percentage of CPU time spent executing processes",
  "keywords": ["cpu", "usage", "utilization", "busy"],
  "section": "cpu",
  "group": "utilization",
  "unit": "percentage",
  "related_charts": ["CPU Idle", "CPU User", "CPU System"]
}

// LLM sees clear, contextual information
"CPU Busy Percentage: Percentage of CPU time spent executing processes"
// Instead of just "Busy %"
```

## Implementation Path

1. **Phase 1: Add Metadata to Current Code** (Quick win)
   - Add description field to PlotOpts
   - Use full titles ("CPU Busy %" instead of "Busy %")
   
2. **Phase 2: Builder Pattern Registry** (Medium effort)
   - Implement ChartBuilder and ChartRegistry
   - Migrate charts incrementally
   - Export metadata for frontend

3. **Phase 3: Derive Macros** (Longer term)
   - Create proc macro crate
   - Use distributed_slice for auto-collection
   - Full declarative approach

## Example Migration

```rust
// Before
utilization.push(Plot::line(
    "Busy %",
    "busy-pct",
    Unit::Percentage,
    data.cpu_avg("cpu_usage", ()).map(|v| (v / 1000000000.0)),
));

// After (Phase 1 - minimal change)
utilization.push(Plot::line_with_meta(
    "CPU Busy %",
    "busy-pct",
    Unit::Percentage,
    data.cpu_avg("cpu_usage", ()).map(|v| (v / 1000000000.0)),
    PlotMetadata {
        description: "Percentage of CPU time spent executing processes",
        keywords: vec!["cpu", "usage", "utilization"],
    }
));

// After (Phase 2 - with registry)
CHARTS.register_cpu_busy();  // Defined once, reused everywhere
```

## Benefits Summary

1. **Better LLM Understanding**: Full context in titles and descriptions
2. **Improved Search**: Keywords and semantic descriptions
3. **Maintainability**: Central definition of all charts
4. **Documentation**: Self-documenting code
5. **Type Safety**: Compile-time validation
6. **Discoverability**: Easy to list all available charts
7. **Consistency**: Enforced metadata structure