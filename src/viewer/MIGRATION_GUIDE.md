# Dashboard Migration Guide: Imperative to Declarative

## Overview

This guide demonstrates migrating from the imperative dashboard style to a declarative Builder pattern.

## Benefits of Declarative Pattern

1. **Separation of Concerns**: Data fetching logic is separated from dashboard structure
2. **Reusability**: Common patterns can be extracted into helper functions
3. **Testability**: Each component can be tested independently
4. **Composability**: Dashboards can be built from smaller, reusable pieces
5. **Type Safety**: Builder pattern ensures required fields are set at compile time

## Migration Example: CPU Dashboard

### Before (Imperative)

```rust
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    let mut view = View::new(data, sections);
    
    // Manually create group
    let mut utilization = Group::new("Utilization", "utilization");
    
    // Manually push plots with inline data fetching
    utilization.push(Plot::line(
        "Busy %",
        "busy-pct",
        Unit::Percentage,
        data.cpu_avg("cpu_usage", ()).map(|v| (v / 1000000000.0)),
    ));
    
    // Lots of conditional logic mixed with presentation
    if let (Some(cycles), Some(instructions)) = (
        data.counters("cpu_cycles", ()).map(|v| v.rate().sum()),
        data.counters("cpu_instructions", ()).map(|v| v.rate().sum()),
    ) {
        let ipc = instructions / cycles;
        performance.plot(
            PlotOpts::line("IPC", "ipc", Unit::Count),
            Some(ipc),
        );
    }
    
    view.group(utilization);
    view
}
```

### After (Declarative)

```rust
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(utilization_group())
        .group(performance_group())
        .build()
}

fn utilization_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Utilization", "utilization")
        .plot(
            PlotConfig::line("Busy %", "busy-pct", Unit::Percentage)
                .data(
                    DataSource::cpu_avg("cpu_usage")
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
}

fn performance_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Performance", "performance")
        .plot(ipc_plot())
}

fn ipc_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::line("IPC", "ipc", Unit::Count)
        .data(
            DataSource::computed(|data| {
                match (
                    data.counters("cpu_cycles", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_instructions", ()).map(|v| v.rate().sum()),
                ) {
                    (Some(cycles), Some(instructions)) => Some(instructions / cycles),
                    _ => None,
                }
            })
        )
        .build()
}
```

## Key Patterns

### 1. Data Sources

Data sources abstract the data fetching logic:

```rust
// Simple counter
DataSource::counter("metric_name")

// Counter with labels
DataSource::counter_with_labels("metric", vec![("key", "value")])

// With transformation
DataSource::cpu_avg("metric")
    .with_transform(|v| v / 1000000000.0)

// Computed from multiple sources
DataSource::computed(|data| {
    // Complex calculation
})
```

### 2. Conditional Plots

Add plots conditionally based on data availability:

```rust
PlotConfig::Conditional {
    condition: Box::new(|data| data.has_metric("cpu_cycles")),
    plot: Box::new(ipc_plot()),
}
```

### 3. Reusable Components

Extract common patterns:

```rust
fn cpu_metric_pair<'a>(
    title: &str,
    id: &str,
    metric: &'a str,
) -> Vec<PlotConfig<'a>> {
    vec![
        PlotConfig::line(title, id, Unit::Rate)
            .data(DataSource::counter(metric))
            .build(),
        PlotConfig::heatmap(title, format!("{}-heatmap", id), Unit::Rate)
            .data(HeatmapSource::cpu_heatmap(metric))
            .build(),
    ]
}
```

## Migration Steps

1. **Identify Groups**: Extract each group creation into a separate function
2. **Abstract Data Sources**: Replace inline data fetching with DataSource enums
3. **Extract Plot Creation**: Create helper functions for common plot patterns
4. **Use Builder Pattern**: Replace imperative group building with declarative GroupConfig
5. **Test Components**: Write tests for individual components

## Testing

The declarative pattern enables better testing:

```rust
#[test]
fn test_ipc_calculation() {
    let data = mock_tsdb();
    let plot = ipc_plot();
    // Test plot configuration and data source
}

#[test]
fn test_utilization_group() {
    let group = utilization_group();
    // Test group structure
}
```

## Next Steps

1. Migrate remaining dashboards (network, scheduler, etc.)
2. Create a library of common plot patterns
3. Add validation for plot configurations
4. Implement dashboard templates for common use cases