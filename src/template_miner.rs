use crate::config::TemplateMinerConfig;
use crate::drain::{Drain, LogCluster};
use crate::masking::LogMasker;
use crate::persistence::PersistenceHandler;
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TemplateMiner {
    pub config: TemplateMinerConfig,
    pub drain: Drain,
    pub masker: LogMasker,
    persistence_handler: Option<Box<dyn PersistenceHandler>>,
    last_save_time: u64,
}

impl TemplateMiner {
    pub fn new(
        config: TemplateMinerConfig,
        persistence_handler: Option<Box<dyn PersistenceHandler>>,
    ) -> Self {
        let param_str = format!("{}*{}", config.mask_prefix, config.mask_suffix);
        
        let drain = Drain::new(
            config.drain_depth,
            config.drain_sim_th,
            config.drain_max_children,
            config.drain_max_clusters,
            config.drain_extra_delimiters.clone(),
            param_str,
            config.parametrize_numeric_tokens,
        );

        let masker = LogMasker::new(
            &config.masking_instructions,
            &config.mask_prefix,
            &config.mask_suffix,
        );

        let mut miner = Self {
            config,
            drain,
            masker,
            persistence_handler,
            last_save_time: Self::current_time_sec(),
        };

        if let Err(e) = miner.load_state() {
             eprintln!("Failed to load state: {}", e);
        }

        miner
    }

    fn current_time_sec() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    pub fn add_log_message(&mut self, log_message: &str) -> (LogCluster, String) {
        let masked_content = self.masker.mask(log_message);
        let (cluster, change_type) = self.drain.add_log_message(&masked_content);

        let change_type_owned = change_type.clone();
        if self.persistence_handler.is_some() {
             if let Some(reason) = self.get_snapshot_reason(&change_type_owned) {
                 if let Err(e) = self.save_state(&reason) {
                     eprintln!("Failed to save state: {}", e);
                 }
            }
        }

        (cluster, change_type)
    }

    // TODO: match function

    fn get_snapshot_reason(&self, change_type: &str) -> Option<String> {
        if change_type != "none" {
            return Some(format!("{} (cluster_id=?)", change_type)); // Simplify for now
        }

        let diff_time = Self::current_time_sec() - self.last_save_time;
        if diff_time >= self.config.snapshot_interval_minutes * 60 {
            return Some("periodic".to_string());
        }
        None
    }

    fn save_state(&mut self, _snapshot_reason: &str) -> Result<()> {
         if let Some(handler) = &mut self.persistence_handler {
             // We only save the Drain state, not configuration or masking which are static/init-time
             let state = serde_json::to_vec(&self.drain)?;
             // Compression logic would go here if enabled
             handler.save_state(&state)?;
             self.last_save_time = Self::current_time_sec();
         }
         Ok(())
    }

    fn load_state(&mut self) -> Result<()> {
        if let Some(handler) = &mut self.persistence_handler {
            if let Some(state) = handler.load_state()? {
                // Decompression logic would go here
                let loaded_drain: Drain = serde_json::from_slice(&state)?;
                self.drain = loaded_drain;
            }
        }
        Ok(())
    }
}
