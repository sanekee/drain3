#[cfg(test)]
mod tests {
    use crate::cluster::UpdateType;
    use crate::drain::Drain;

    #[test]
    fn test_drain_parsing() {
        let mut drain = Drain::new(4, 0.4, 100, None, vec![], true, "TOKEN", "<", ">");

        let log1 = "Connected to 10.0.0.1";
        let (cluster1, type1) = drain.add_log_message(log1);
        assert_eq!(type1, UpdateType::Created);
        assert_eq!(cluster1.cluster_id, 1);
        assert_eq!(cluster1.get_template(), "Connected to 10.0.0.1");

        let log2 = "Connected to 10.0.0.2";
        let (cluster2, type2) = drain.add_log_message(log2);
        assert_eq!(type2, UpdateType::Updated);
        assert_eq!(cluster2.cluster_id, 1);
        assert_eq!(cluster2.get_template(), "Connected to <TOKEN1>");

        let log3 = "Disconnect from 10.0.0.1";
        let (cluster3, type3) = drain.add_log_message(log3);
        assert_eq!(type3, UpdateType::Created);
        assert_eq!(cluster3.cluster_id, 2);
    }

    #[test]
    fn test_drain_max_children() {
        let mut drain = Drain::new(4, 0.4, 2, None, vec![], true, "TOKEN", "<", ">");
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
}
