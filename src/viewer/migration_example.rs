// Example of how to migrate existing dashboard code to use the registry
// This shows both the old way and the new way side by side

use crate::registry::*;
use crate::*;

// === OLD WAY (current implementation) ===

pub fn generate_cpu_old(data: &Tsdb, sections: Vec<Section>) -> View {
    let mut view = View::new(data, sections);
    
    // Utilization group
    let mut utilization = Group::new("Utilization", "utilization");
    
    // Manually create each chart with minimal context
    utilization.push(Plot::line(
        "Busy %",  // No context - what's busy?
        "busy-pct",
        Unit::Percentage,
        data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
    ));
    
    utilization.push(Plot::heatmap(
        "Busy %",  // Duplicate title, no differentiation
        "busy-pct-heatmap",
        Unit::Percentage,
        data.cpu_heatmap("cpu_usage", ()).map(|v| v / 1000000000.0),
    ));
    
    // More imperative code...
    for state in &["User", "System"] {
        utilization.push(Plot::line(
            format!("{state} %"),
            format!("{}-pct", state.to_lowercase()),
            Unit::Percentage,
            data.cpu_avg("cpu_usage", [("state", state.to_lowercase())])
                .map(|v| v / 1000000000.0),
        ));
    }
    
    view.group(utilization);
    
    // Performance group
    let mut performance = Group::new("Performance", "performance");
    
    if let (Some(cycles), Some(instructions)) = (
        data.counters("cpu_cycles", ()).map(|v| v.rate().sum()),
        data.counters("cpu_instructions", ()).map(|v| v.rate().sum()),
    ) {
        let ipc = instructions / cycles;
        performance.plot(
            PlotOpts::line("Instructions per Cycle (IPC)", "ipc", Unit::Count),
            Some(ipc),
        );
    }
    
    view.group(performance);
    view
}

// === NEW WAY (using registry) ===

// Step 1: Define charts once with rich metadata
fn register_cpu_charts() -> ChartSection {
    ChartSection::new("CPU", "/cpu")
        .description("CPU utilization, performance, and scheduling metrics")
        .add_group(
            ChartGroup::new("Utilization", "utilization")
                // CPU Busy - Line Chart
                .add_chart(
                    ChartBuilder::new("Busy %")
                        .long_title("CPU Busy Percentage")
                        .description(
                            "Percentage of CPU time spent executing processes. \
                            High sustained values (>80%) indicate CPU saturation. \
                            This includes both user and system time."
                        )
                        .keywords(["cpu", "usage", "utilization", "busy", "processor", "load"])
                        .use_cases("Monitor for performance bottlenecks, capacity planning, detecting CPU-bound workloads")
                        .related(["CPU Idle %", "CPU User %", "CPU System %", "CPU Busy Heatmap"])
                        .thresholds(Some(80.0), Some(95.0))
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::line(
                                "CPU Busy %",
                                "busy-pct",
                                Unit::Percentage,
                                data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                )
                // CPU Busy - Heatmap
                .add_chart(
                    ChartBuilder::new("Busy Heatmap")
                        .long_title("CPU Busy Percentage Heatmap")
                        .description(
                            "Per-CPU core utilization heatmap showing distribution of load across cores. \
                            Helps identify core imbalances and scheduling issues."
                        )
                        .keywords(["cpu", "heatmap", "cores", "distribution", "balance"])
                        .use_cases("Identify hot cores, detect scheduling problems, analyze parallelization")
                        .related(["CPU Busy %"])
                        .chart_type(ChartType::Heatmap)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::heatmap(
                                "CPU Busy Heatmap",
                                "busy-pct-heatmap",
                                Unit::Percentage,
                                data.cpu_heatmap("cpu_usage", ()).map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                )
                // CPU User %
                .add_chart(
                    ChartBuilder::new("User %")
                        .long_title("CPU User Mode Percentage")
                        .description(
                            "CPU time spent executing user space processes. \
                            High values indicate application CPU usage."
                        )
                        .keywords(["cpu", "user", "application", "userspace"])
                        .use_cases("Distinguish application load from system overhead")
                        .related(["CPU System %", "CPU Busy %"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::line(
                                "CPU User %",
                                "user-pct",
                                Unit::Percentage,
                                data.cpu_avg("cpu_usage", [("state", "user")])
                                    .map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                )
                // CPU System %
                .add_chart(
                    ChartBuilder::new("System %")
                        .long_title("CPU System Mode Percentage")
                        .description(
                            "CPU time spent in kernel mode handling system calls and hardware interrupts. \
                            High values may indicate excessive syscalls or driver issues."
                        )
                        .keywords(["cpu", "system", "kernel", "syscall"])
                        .use_cases("Identify kernel overhead, syscall bottlenecks, driver problems")
                        .related(["CPU User %", "CPU Busy %", "System Calls/s"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::line(
                                "CPU System %",
                                "system-pct",
                                Unit::Percentage,
                                data.cpu_avg("cpu_usage", [("state", "system")])
                                    .map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                )
        )
        .add_group(
            ChartGroup::new("Performance", "performance")
                .add_chart(
                    ChartBuilder::new("IPC")
                        .long_title("Instructions per Cycle")
                        .description(
                            "Average number of instructions executed per CPU cycle. \
                            Higher values (>1.0) indicate better CPU efficiency and parallelism. \
                            Low values may indicate memory stalls or branch mispredictions."
                        )
                        .keywords(["cpu", "performance", "ipc", "instructions", "efficiency", "pipeline"])
                        .use_cases("Analyze CPU efficiency, identify cache misses, optimize code performance")
                        .related(["IPNS", "CPU Cycles", "Cache Misses"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Count)
                        .generator(|data| {
                            let cycles = data.counters("cpu_cycles", ()).map(|v| v.rate().sum())?;
                            let instructions = data.counters("cpu_instructions", ())
                                .map(|v| v.rate().sum())?;
                            let ipc = instructions / cycles;
                            Plot::line("Instructions per Cycle", "ipc", Unit::Count, Some(ipc))
                        })
                        .build(),
                )
        )
}

// Step 2: Use the registry to generate dashboards
pub fn generate_cpu_new(data: &Tsdb, sections: Vec<Section>) -> View {
    // Simply use the pre-registered section
    let cpu_section = register_cpu_charts();
    cpu_section.generate(data, sections)
}

// === Benefits Demonstrated ===

// 1. Export for LLM - the LLM can now understand what each chart means
pub fn export_for_llm_example() {
    let registry = ChartRegistry::new()
        .add_section(register_cpu_charts());
    
    println!("=== LLM-Friendly Export ===\n");
    println!("{}", registry.export_for_llm());
    
    /* Output would be:
    ## CPU Section
    CPU utilization, performance, and scheduling metrics
    
    ### Utilization (utilization)
    - **CPU Busy Percentage** (Busy %)
      Description: Percentage of CPU time spent executing processes...
      Keywords: cpu, usage, utilization, busy, processor, load
      Use cases: Monitor for performance bottlenecks...
      Warning threshold: 80
      Critical threshold: 95
      Related: CPU Idle %, CPU User %, CPU System %
    */
}

// 2. Export as JSON for frontend
pub fn export_json_example() {
    let registry = ChartRegistry::new()
        .add_section(register_cpu_charts());
    
    let json = registry.export_metadata_json();
    // This JSON can be loaded by the frontend to understand available charts
}

// 3. Search for charts
pub fn search_example() {
    let registry = ChartRegistry::new()
        .add_section(register_cpu_charts());
    
    // Get all charts with their full context
    let all_charts = registry.all_charts_with_context();
    
    for (context, metadata) in all_charts {
        if metadata.keywords.contains(&"performance".to_string()) {
            println!("Found performance chart: {}", context);
            // Would print: "CPU / Performance / Instructions per Cycle"
        }
    }
}