use crate::cluster::{LogCluster, SearchStrategy, UpdateType};
use crate::config::TemplateMinerConfig;
use crate::drain::{Drain, SerializableDrain};
use crate::masking::{AbstractMaskingInstruction, LogMasker, MaskingInstruction};
use crate::persistence::PersistenceHandler;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct ExtractedParameter {
    pub value: String,
    pub mask_name: String,
}

impl ExtractedParameter {
    pub fn new(value: String, mask_name: String) -> Self {
        Self { value, mask_name }
    }
}

pub struct TemplateMiner<'a> {
    pub config: &'a TemplateMinerConfig,
    pub drain: Drain,
    pub masker: LogMasker,
    persistence_handler: Option<Box<dyn PersistenceHandler>>,
    last_save_time: u64,
    state_dirty: bool,
}

impl<'a> TemplateMiner<'a> {
    pub fn new(
        config: &'a TemplateMinerConfig,
        persistence_handler: Option<Box<dyn PersistenceHandler>>,
    ) -> Self {
        let drain = Drain::new(
            config.drain_depth,
            config.drain_sim_th,
            config.drain_max_children,
            config.drain_max_clusters,
            config.drain_extra_delimiters.clone(),
            config.parametrize_numeric_tokens,
            &config.token_template,
            &config.mask_prefix,
            &config.mask_suffix,
        );

        let masking_instructions = config
            .masking_instructions
            .iter()
            .map(|config| {
                Box::new(MaskingInstruction::new(config)) as Box<dyn AbstractMaskingInstruction>
            })
            .collect();

        let masker = LogMasker::new(
            masking_instructions,
            &config.mask_prefix,
            &config.mask_suffix,
        );

        let mut miner = Self {
            config,
            drain,
            masker,
            persistence_handler,
            last_save_time: Self::current_time_sec(),
            state_dirty: false,
        };

        if let Err(e) = miner.load_state() {
            eprintln!("Failed to load state: {}", e);
        }

        miner
    }

    fn current_time_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub fn add_log_message(
        &mut self,
        log_message: &str,
    ) -> (Option<Arc<Mutex<LogCluster>>>, UpdateType) {
        let masked_content = self.masker.mask(log_message);
        let (cluster, change_type) = self.drain.add_log_message(&masked_content);

        self.state_dirty = self.state_dirty || change_type != UpdateType::None;
        if self.persistence_handler.is_some()
            && self.should_save_state()
            && let Err(e) = self.save_state()
        {
            eprintln!("Failed to save state: {}", e);
        }

        (cluster, change_type)
    }

    pub fn match_cluster(
        &self,
        content: &str,
        strategy: SearchStrategy,
    ) -> Option<Arc<Mutex<LogCluster>>> {
        let masked_content = self.masker.mask(content);
        self.drain.match_cluster(masked_content.as_str(), strategy)
    }

    fn should_save_state(&self) -> bool {
        Self::current_time_sec() - self.last_save_time >= self.config.snapshot_interval_minutes * 60
            && self.state_dirty
    }

    pub fn save_state(&mut self) -> Result<()> {
        if let Some(handler) = &mut self.persistence_handler {
            let ser_drain = SerializableDrain::from(&self.drain);
            let state = serde_json::to_vec(&ser_drain)?;
            handler.save_state(&state)?;
            self.last_save_time = Self::current_time_sec();
        }
        Ok(())
    }

    fn load_state(&mut self) -> Result<()> {
        if let Some(handler) = &mut self.persistence_handler
            && let Some(state) = handler.load_state()?
        {
            // Decompression logic would go here
            // let loaded_drain: SerializableDrain = serde_json::from_slice(&state)?;
            // self.drain = Drain::from(loaded_drain);
        }
        Ok(())
    }

    pub fn get_parameter_list(&self, log_template: &str, log_message: &str) -> Vec<String> {
        match self.extract_parameters(log_template, log_message, false) {
            Some(params) => params.into_iter().map(|p| p.value).collect(),
            None => Vec::new(),
        }
    }

    pub fn extract_parameters(
        &self,
        log_template: &str,
        log_message: &str,
        exact_matching: bool,
    ) -> Option<Vec<ExtractedParameter>> {
        let mut normalized = log_message.to_string();

        for delimiter in &self.config.drain_extra_delimiters {
            let re = Regex::new(delimiter).ok()?;
            normalized = re.replace_all(&normalized, " ").into_owned();
        }

        let (template_regex, param_map) =
            self.get_template_parameter_extraction_regex(log_template, exact_matching);

        let re = Regex::new(&template_regex).ok()?;

        let captures = re.captures(&normalized)?;

        let mut extracted = Vec::new();

        for (group_name, mask_name) in param_map {
            if let Some(value) = captures.name(&group_name) {
                extracted.push(ExtractedParameter::new(
                    value.as_str().to_string(),
                    mask_name,
                ));
            }
        }

        Some(extracted)
    }

    pub fn get_template_parameter_extraction_regex(
        &self,
        log_template: &str,
        exact_matching: bool,
    ) -> (String, HashMap<String, String>) {
        let mut param_map: HashMap<String, String> = HashMap::new();
        let mut counter: usize = 0;

        let mut get_next_param_name = || {
            let name = format!("p_{}", counter);
            counter += 1;
            name
        };

        let mut create_capture_regex = |mask_name: &str| -> String {
            let mut allowed_patterns: Vec<String> = Vec::new();

            if exact_matching {
                let instructions = self.masker.instructions_by_mask_name(mask_name);

                for mi in instructions {
                    let mut pattern = mi.pattern().to_string();

                    let unnamed_backref =
                        Regex::new(r"\\[1-9]\d?").expect("failed to compile unnamed backref regex");

                    pattern = unnamed_backref.replace_all(&pattern, "(?:.+?)").to_string();

                    allowed_patterns.push(pattern);
                }
            }

            if !exact_matching || mask_name == "*" {
                allowed_patterns.push(".+?".to_string());
            }

            let param_name = get_next_param_name();

            param_map.insert(param_name.clone(), mask_name.to_string());

            let joined = allowed_patterns.join("|");

            format!("(?P<{}>{})", param_name, joined)
        };

        let mut mask_names: HashSet<String> = self.masker.mask_names().iter().cloned().collect();

        mask_names.insert("*".to_string());

        let escaped_prefix = regex::escape(&self.masker.mask_prefix);

        let escaped_suffix = regex::escape(&self.masker.mask_suffix);

        let mut template_regex = regex::escape(log_template);

        for mask_name in mask_names {
            let search_str = format!(
                "{}{}{}",
                escaped_prefix,
                regex::escape(&mask_name),
                escaped_suffix
            );

            loop {
                let rep = create_capture_regex(&mask_name);

                let new = template_regex.replacen(&search_str, &rep, 1);

                if new == template_regex {
                    break;
                }

                template_regex = new;
            }
        }

        let space_regex = Regex::new(r"\\ ").unwrap();

        template_regex = space_regex
            .replace_all(&template_regex, r"\\s+")
            .into_owned();

        template_regex = format!("^{}$", template_regex);

        (template_regex, param_map)
    }
}
