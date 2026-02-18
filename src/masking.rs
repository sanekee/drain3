use fancy_regex::Regex;
use crate::config::MaskingInstruction;

pub struct LogMasker {
    instructions: Vec<(Regex, String)>,
    mask_prefix: String,
    mask_suffix: String,
}

impl LogMasker {
    pub fn new(instructions: &[MaskingInstruction], mask_prefix: &str, mask_suffix: &str) -> Self {
        let compiled_instructions = instructions.iter().map(|i| {
            (Regex::new(&i.pattern).unwrap(), i.mask_with.clone())
        }).collect();

        Self {
            instructions: compiled_instructions,
            mask_prefix: mask_prefix.to_string(),
            mask_suffix: mask_suffix.to_string(),
        }
    }

    pub fn mask(&self, content: &str) -> String {
        let mut masked_content = content.to_string();
        for (regex, mask_with) in &self.instructions {
            let replacement = format!("{}{}{}", self.mask_prefix, mask_with, self.mask_suffix);
            masked_content = regex.replace_all(&masked_content, replacement.as_str()).to_string();
        }
        masked_content
    }
}
