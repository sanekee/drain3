use serde::{Deserialize, Serialize};

use crate::masking::MaskingInstructionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMinerConfig {
    #[serde(default = "default_engine")]
    pub engine: String,
    #[serde(default = "default_drain_depth")]
    pub drain_depth: usize,
    #[serde(default = "default_drain_sim_th")]
    pub drain_sim_th: f64,
    #[serde(default = "default_drain_max_children")]
    pub drain_max_children: usize,
    pub drain_max_clusters: Option<usize>,
    #[serde(default)]
    pub drain_extra_delimiters: Vec<String>,
    #[serde(default = "default_mask_prefix")]
    pub mask_prefix: String,
    #[serde(default = "default_mask_suffix")]
    pub mask_suffix: String,
    #[serde(default = "default_parametrize_numeric_tokens")]
    pub parametrize_numeric_tokens: bool,
    #[serde(default = "default_parameter_extraction_cache_capacity")]
    pub parameter_extraction_cache_capacity: usize,
    #[serde(default)]
    pub masking_instructions: Vec<MaskingInstructionConfig>,
    #[serde(default = "default_snapshot_interval_minutes")]
    pub snapshot_interval_minutes: u64,
}

fn default_engine() -> String {
    "Drain".to_string()
}
fn default_drain_depth() -> usize {
    4
}
fn default_drain_sim_th() -> f64 {
    0.4
}
fn default_drain_max_children() -> usize {
    100
}
fn default_mask_prefix() -> String {
    "<".to_string()
}
fn default_mask_suffix() -> String {
    ">".to_string()
}
fn default_parametrize_numeric_tokens() -> bool {
    true
}
fn default_parameter_extraction_cache_capacity() -> usize {
    3000
}
fn default_snapshot_interval_minutes() -> u64 {
    1
}

impl Default for TemplateMinerConfig {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            drain_depth: default_drain_depth(),
            drain_sim_th: default_drain_sim_th(),
            drain_max_children: default_drain_max_children(),
            drain_max_clusters: None,
            drain_extra_delimiters: vec![],
            mask_prefix: default_mask_prefix(),
            mask_suffix: default_mask_suffix(),
            parametrize_numeric_tokens: default_parametrize_numeric_tokens(),
            parameter_extraction_cache_capacity: default_parameter_extraction_cache_capacity(),
            masking_instructions: vec![],
            snapshot_interval_minutes: default_snapshot_interval_minutes(),
        }
    }
}

impl TemplateMinerConfig {
    pub fn load(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }
}
