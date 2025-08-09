use super::common::*;

/// AI-powered dashboard that's dynamically generated
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "AI Assistant".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "AI Generated Dashboard".to_string(),
                id: "ai-dashboard".to_string(),
                panels: vec![
                    // Panels will be dynamically generated based on user input
                    // This is just a placeholder that will be replaced by the frontend
                    PromQLPanel {
                        title: "Ask AI for help...".to_string(),
                        id: "ai-placeholder".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![],
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
        ],
    }
}