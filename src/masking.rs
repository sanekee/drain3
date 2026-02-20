use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait AbstractMaskingInstruction {
    fn mask_with(&self) -> &str;
    fn mask(&self, content: &str, mask_prefix: &str, mask_suffix: &str) -> String;
    fn pattern(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingInstructionConfig {
    #[serde(rename = "regex_pattern")]
    pub pattern: String,
    pub mask_with: String,
}

#[derive(Clone)]
pub struct MaskingInstruction {
    pub pattern: String,
    pub mask_with: String,
    pub regex: Regex,
}

impl MaskingInstruction {
    pub fn new(config: &MaskingInstructionConfig) -> Self {
        let re = match Regex::new(config.pattern.as_str()) {
            Ok(x) => x,
            Err(e) => {
                panic!("failed to compile regex {}, {}", config.pattern, e);
            }
        };
        Self {
            pattern: config.pattern.to_string(),
            mask_with: config.mask_with.to_string(),
            regex: re,
        }
    }
}

impl AbstractMaskingInstruction for MaskingInstruction {
    fn mask_with(&self) -> &str {
        &self.mask_with
    }

    fn mask(&self, content: &str, mask_prefix: &str, mask_suffix: &str) -> String {
        let replacement = format!("{}{}{}", mask_prefix, self.mask_with, mask_suffix);
        self.regex
            .replace_all(content, replacement.as_str())
            .to_string()
    }

    fn pattern(&self) -> &str {
        &self.pattern
    }
}

pub type RegexMaskingInstruction = MaskingInstruction;

pub struct LogMasker {
    instructions: Vec<Box<dyn AbstractMaskingInstruction>>,
    pub mask_prefix: String,
    pub mask_suffix: String,
    mask_name_to_instructions: HashMap<String, Vec<usize>>, // indexes into `instructions`
}

impl LogMasker {
    pub fn new(
        instructions: Vec<Box<dyn AbstractMaskingInstruction>>,
        mask_prefix: &str,
        mask_suffix: &str,
    ) -> Self {
        let mut mask_name_to_instructions: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, mi) in instructions.iter().enumerate() {
            mask_name_to_instructions
                .entry(mi.mask_with().to_string())
                .or_default()
                .push(i);
        }

        Self {
            instructions,
            mask_prefix: mask_prefix.to_string(),
            mask_suffix: mask_suffix.to_string(),
            mask_name_to_instructions,
        }
    }

    pub fn mask(&self, content: &str) -> String {
        let mut masked = content.to_string();
        for mi in &self.instructions {
            masked = mi.mask(&masked, &self.mask_prefix, &self.mask_suffix);
        }
        masked
    }

    pub fn mask_names(&self) -> Vec<String> {
        self.mask_name_to_instructions.keys().cloned().collect()
    }

    pub fn instructions_by_mask_name(
        &self,
        mask_name: &str,
    ) -> Vec<&dyn AbstractMaskingInstruction> {
        if let Some(indices) = self.mask_name_to_instructions.get(mask_name) {
            indices
                .iter()
                .map(|&i| self.instructions[i].as_ref())
                .collect()
        } else {
            vec![]
        }
    }
}
