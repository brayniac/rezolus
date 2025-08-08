pub mod common;
pub mod overview;
pub mod cpu;
pub mod network;
pub mod blockio;
pub mod scheduler;
pub mod syscall;
pub mod softirq;
pub mod rezolus;
pub mod cgroups;

use self::common::PromQLDashboard;

// Re-export common types for backward compatibility
pub use self::common::{
    DashboardSection, PanelOptions, PanelType, PromQLGroup, PromQLPanel, PromQLQueryDef, Unit,
};

/// Get dashboard definition by name
pub fn get_dashboard(name: &str) -> Option<PromQLDashboard> {
    match name {
        "cpu" => Some(cpu::dashboard()),
        "network" => Some(network::dashboard()),
        "blockio" => Some(blockio::dashboard()),
        "scheduler" => Some(scheduler::dashboard()),
        "syscall" => Some(syscall::dashboard()),
        "softirq" => Some(softirq::dashboard()),
        "rezolus" => Some(rezolus::dashboard()),
        "cgroups" => Some(cgroups::dashboard()),
        "overview" => Some(overview::dashboard()),
        _ => None,
    }
}

/// Generate all dashboard definitions
pub fn generate_all_dashboards() -> Vec<PromQLDashboard> {
    vec![
        overview::dashboard(),
        cpu::dashboard(),
        network::dashboard(),
        blockio::dashboard(),
        scheduler::dashboard(),
        syscall::dashboard(),
        softirq::dashboard(),
        rezolus::dashboard(),
        cgroups::dashboard(),
    ]
}