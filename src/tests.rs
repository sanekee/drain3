#[cfg(test)]
mod tests {
    use crate::cluster::UpdateType;
    use crate::drain::Drain;

    #[test]
    fn test_drain_parsing() {
        let mut drain = Drain::new(&crate::drain::DrainConfig {
            log_cluster_depth: 4,
            sim_th: 0.4,
            max_children: 100,
            max_clusters: None,
            extra_delimiters: vec![],
            parametrize_numeric_tokens: true,
            token_prefix: "<".to_string(),
            token_suffix: ">".to_string(),
            token_template: "TOKEN".to_string(),
        });

        let log1 = "Connected to 10.0.0.1";
        let (cluster1, type1) = drain.add_log_message(log1);
        assert_eq!(type1, UpdateType::Created);
        let cluster1_ref = cluster1.unwrap();
        assert_eq!(cluster1_ref.lock().unwrap().get_cluster_id(), 1);
        assert_eq!(
            cluster1_ref.lock().unwrap().get_template(),
            "Connected to 10.0.0.1"
        );

        let log2 = "Connected to 10.0.0.2";
        let (cluster2, type2) = drain.add_log_message(log2);
        assert_eq!(type2, UpdateType::Updated);
        let cluster2_ref = cluster2.unwrap();
        assert_eq!(cluster2_ref.lock().unwrap().get_cluster_id(), 1);
        assert_eq!(
            cluster2_ref.lock().unwrap().get_template(),
            "Connected to <TOKEN1>"
        );

        let log3 = "Disconnect from 10.0.0.1";
        let (cluster3, type3) = drain.add_log_message(log3);
        assert_eq!(type3, UpdateType::Created);
        assert_eq!(cluster3.unwrap().lock().unwrap().get_cluster_id(), 2);
    }

    #[test]
    fn test_drain_max_children() {
        let mut drain = Drain::new(&crate::drain::DrainConfig {
            log_cluster_depth: 4,
            sim_th: 0.4,
            max_children: 2,
            max_clusters: None,
            extra_delimiters: vec![],
            parametrize_numeric_tokens: true,
            token_prefix: "<".to_string(),
            token_suffix: ">".to_string(),
            token_template: "TOKEN".to_string(),
        });
        // Simulate filling up a node
        drain.add_log_message("A");
        drain.add_log_message("B");
        drain.add_log_message("C"); // Should go to param

        // Verification of internal structure would be ideal, but for now black-box testing
        // "A" -> cluster 1
        // "B" -> cluster 2
        // "C" -> cluster 3.
        // If C went to param, then "D" should match C's cluster if they are similar?
        // No, C and D are just tokens.

        // Let's rely on templates.
        // A -> "A"
        // B -> "B"
        // C -> "<*>" (if it went to param branch)
    }
    #[test]
    fn test_masking() {
        use crate::masking::LogMasker;
        use crate::masking::MaskingInstruction;
        use crate::masking::MaskingInstructionConfig;

        let instructions: Vec<Box<dyn crate::masking::AbstractMaskingInstruction>> =
            vec![Box::new(MaskingInstruction::new(
                &MaskingInstructionConfig {
                    pattern: r"\d+".to_string(),
                    mask_with: "NUM".to_string(),
                },
            ))];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("User 123 logged in");
        assert_eq!(masked, "User <NUM> logged in");
    }

    #[test]
    fn test_id_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"(?:[0-9a-f]{2,}:){3,}[0-9a-f]{2,}".to_string(),
                mask_with: "ID".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("device aa:bb:cc:dd:ee connected");

        assert_eq!(masked, "device <ID> connected");
    }

    #[test]
    fn test_ip_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"(\d{1,3}(\.\d{1,3}){3})".to_string(),
                mask_with: "IP".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("connect 10.1.1.0 success");

        assert_eq!(masked, "connect <IP> success");
    }

    #[test]
    fn test_host_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"([A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+)".to_string(),
                mask_with: "HOST".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("connect server.example.com now");

        assert_eq!(masked, "connect <HOST> now");
    }

    #[test]
    fn test_seq_lower_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"(([0-9a-f]{6,} ?){2,}([0-9a-f]{6,}))".to_string(),
                mask_with: "SEQ".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("seq abcdef 123456 fedcba done");

        assert_eq!(masked, "seq <SEQ> done");
    }

    #[test]
    fn test_seq_upper_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"(([0-9A-F]{4} ?){3,}([0-9A-F]{4}))".to_string(),
                mask_with: "SEQ".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("seq ABCD 0123 4567 89AB done");

        assert_eq!(masked, "seq <SEQ> done");
    }

    #[test]
    fn test_hex_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"(0x[a-fA-F0-9]+)".to_string(),
                mask_with: "HEX".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("value 0xdeadbeef found");

        assert_eq!(masked, "value <HEX> found");
    }

    #[test]
    fn test_num_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"([-+]?\d+)".to_string(),
                mask_with: "NUM".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("value -42 found");

        assert_eq!(masked, "value <NUM> found");
    }

    #[test]
    fn test_cmd_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r#"(executed cmd )(".+?")"#.to_string(),
                mask_with: "CMD".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask(r#"executed cmd "rm -rf /""#);

        assert_eq!(masked, "<CMD>");
    }

    #[test]
    fn test_str_single_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r"'[^']*'".to_string(),
                mask_with: "STR".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask("user 'john' logged in");

        assert_eq!(masked, "user <STR> logged in");
    }

    #[test]
    fn test_str_double_masking() {
        use crate::masking::{
            AbstractMaskingInstruction, LogMasker, MaskingInstruction, MaskingInstructionConfig,
        };

        let instructions: Vec<Box<dyn AbstractMaskingInstruction>> = vec![Box::new(
            MaskingInstruction::new(&MaskingInstructionConfig {
                pattern: r#""[^"]*""#.to_string(),
                mask_with: "STR".to_string(),
            }),
        )];

        let masker = LogMasker::new(instructions, "<", ">");
        let masked = masker.mask(r#"user "john" logged in"#);

        assert_eq!(masked, "user <STR> logged in");
    }
}
