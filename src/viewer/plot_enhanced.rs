// Enhanced PlotOpts with metadata support
// This can be integrated into the existing plot.rs or used alongside it

use serde::{Deserialize, Serialize};

/// Enhanced plot options with metadata
#[derive(Serialize, Clone)]
pub struct PlotOptsEnhanced {
    // Original fields
    pub title: String,
    pub id: String,
    pub style: String,
    pub format: Option<FormatConfig>,
    
    // New metadata fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_title: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ChartMetadata>,
}

impl PlotOptsEnhanced {
    /// Create line chart options with metadata
    pub fn line_with_meta(
        title: impl Into<String>,
        id: impl Into<String>,
        unit: Unit,
        description: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            style: "line".to_string(),
            format: Some(FormatConfig::from_unit(unit)),
            long_title: None,
            description: Some(description.into()),
            keywords: None,
            metadata: None,
        }
    }
    
    /// Add full metadata to existing plot options
    pub fn with_metadata(mut self, metadata: ChartMetadata) -> Self {
        self.long_title = Some(metadata.long_title);
        self.description = Some(metadata.description);
        self.keywords = Some(metadata.keywords);
        self.metadata = Some(metadata);
        self
    }
}

// Backward compatibility: extend existing PlotOpts
impl PlotOpts {
    /// Add description to existing plot
    pub fn with_description(mut self, desc: impl Into<String>) -> PlotOptsEnhanced {
        PlotOptsEnhanced {
            title: self.title,
            id: self.id,
            style: self.style,
            format: self.format,
            long_title: None,
            description: Some(desc.into()),
            keywords: None,
            metadata: None,
        }
    }
    
    /// Convert to enhanced version with metadata
    pub fn enhance(self) -> PlotOptsEnhanced {
        PlotOptsEnhanced {
            title: self.title,
            id: self.id,
            style: self.style,
            format: self.format,
            long_title: None,
            description: None,
            keywords: None,
            metadata: None,
        }
    }
}

// Extension trait for Plot to support metadata
pub trait PlotMetadataExt {
    fn with_metadata(self, metadata: &ChartMetadata) -> Self;
    fn with_description(self, description: impl Into<String>) -> Self;
}

impl PlotMetadataExt for Option<Plot> {
    fn with_metadata(self, metadata: &ChartMetadata) -> Self {
        self.map(|mut plot| {
            // If we had access to modify Plot's opts field, we'd do:
            // plot.opts = plot.opts.enhance().with_metadata(metadata.clone());
            plot
        })
    }
    
    fn with_description(self, description: impl Into<String>) -> Self {
        self.map(|mut plot| {
            // If we had access to modify Plot's opts field, we'd do:
            // plot.opts = plot.opts.with_description(description);
            plot
        })
    }
}