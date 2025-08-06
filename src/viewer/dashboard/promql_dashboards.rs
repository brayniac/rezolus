use super::common::PromQLDashboard;

// Re-export common types for backward compatibility
pub use super::common::{
    DashboardSection, PanelOptions, PanelType, PromQLGroup, PromQLPanel, PromQLQueryDef, Unit,
};

/// Get dashboard definition by name
pub fn get_dashboard(name: &str) -> Option<PromQLDashboard> {
    match name {
        "cpu" => Some(super::cpu::dashboard()),
        "network" => Some(super::network::dashboard()),
        "blockio" => Some(super::blockio::dashboard()),
        "scheduler" => Some(super::scheduler::dashboard()),
        "syscall" => Some(super::syscall::dashboard()),
        "softirq" => Some(super::softirq::dashboard()),
        "rezolus" => Some(super::rezolus::dashboard()),
        "cgroups" => Some(super::cgroups::dashboard()),
        "overview" => Some(super::overview::dashboard()),
        _ => None,
    }
}

/// Generate all dashboard definitions
pub fn generate_all_dashboards() -> Vec<PromQLDashboard> {
    vec![
        super::overview::dashboard(),
        super::cpu::dashboard(),
        super::network::dashboard(),
        super::blockio::dashboard(),
        super::scheduler::dashboard(),
        super::syscall::dashboard(),
        super::softirq::dashboard(),
        super::rezolus::dashboard(),
        super::cgroups::dashboard(),
    ]
}