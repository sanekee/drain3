use serde::{Deserialize, Serialize};

use drain3::config::TemplateMinerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoConfig {
    #[serde(default = "default_miner_config")]
    pub miner_config: TemplateMinerConfig,
    #[serde(default = "default_enable_profiler")]
    pub enable_profiler: bool,
    #[serde(default = "default_save_state")]
    pub save_state: bool,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

fn default_miner_config() -> TemplateMinerConfig {
    TemplateMinerConfig::default()
}

fn default_enable_profiler() -> bool {
    false
}

fn default_save_state() -> bool {
    false
}

fn default_max_lines() -> usize {
    0
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            miner_config: default_miner_config(),
            enable_profiler: default_enable_profiler(),
            save_state: default_save_state(),
            max_lines: default_max_lines(),
        }
    }
}

impl DemoConfig {
    pub fn load(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }
}
