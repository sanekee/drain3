use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use crate::cluster;
use crate::cluster::{LogCluster, Node, SearchStrategy, UpdateType};

#[derive(Debug)]
pub struct Drain {
    pub log_cluster_depth: usize,
    pub sim_th: f64,
    pub max_children: usize,
    pub max_clusters: Option<usize>,
    pub extra_delimiters: Vec<String>,
    pub parametrize_numeric_tokens: bool,

    pub root_node: Node,
    pub clusters_counter: usize,

    // #[serde(skip)]
    token_prefix: String,
    // #[serde(skip)]
    token_suffix: String,
    // #[serde(skip)]
    token_template: String,
    // #[serde(skip)]
    token_template_counter: usize,
    // #[serde(skip)]
    token_template_check: String,
}

impl Drain {
    pub fn new(
        depth: usize,
        sim_th: f64,
        max_children: usize,
        max_clusters: Option<usize>,
        extra_delimiters: Vec<String>,
        parametrize_numeric_tokens: bool,
        token_template: &str,
        token_prefix: &str,
        token_suffix: &str,
    ) -> Self {
        if depth < 3 {
            panic!("depth argument must be at least 3");
        }

        let token_template = token_template
            .is_empty()
            .then(|| "TOKEN")
            .unwrap_or(token_template);

        let token_template_check = format!("{}{}", token_prefix, token_template);

        Self {
            log_cluster_depth: depth,
            sim_th,
            max_children,
            max_clusters,
            extra_delimiters,
            parametrize_numeric_tokens,
            root_node: Node::new(),
            clusters_counter: 0,
            token_template: token_template.to_string(),
            token_template_counter: 0,
            token_prefix: token_prefix.to_string(),
            token_suffix: token_suffix.to_string(),
            token_template_check: token_template_check,
        }
    }

    // Helper to check if string has numbers
    fn has_numbers(s: &str) -> bool {
        s.chars().any(|c| c.is_ascii_digit())
    }

    pub fn get_content_as_tokens(&self, content: &str) -> Vec<String> {
        let mut content = content.trim().to_string();
        for delimiter in &self.extra_delimiters {
            content = content.replace(delimiter, " ");
        }
        content.split_whitespace().map(|s| s.to_string()).collect()
    }

    pub fn add_log_message(
        &mut self,
        content: &str,
    ) -> (Option<Arc<Mutex<LogCluster>>>, UpdateType) {
        let content_tokens = self.get_content_as_tokens(content);

        let match_result = Self::tree_search(
            &self.root_node,
            &content_tokens,
            self.sim_th,
            true,
            self.log_cluster_depth,
            &self.token_template,
        );

        match match_result {
            Some(cluster_id) => {
                let mut counter = self.token_template_counter;
                let cluster_ref = cluster::LogCluster::get_cluster_by_id(&cluster_id);

                if cluster_ref.is_none() {
                    println!("failed to get cluster by id {}", cluster_id);
                    return (None, UpdateType::None);
                }

                let cluster = cluster_ref.unwrap();
                let update_type = cluster.lock().unwrap().update_template(
                    &content_tokens,
                    |t| -> bool { Self::is_token(&self.token_template_check, t) },
                    || {
                        counter += 1;
                        format!(
                            "{}{}{}{}",
                            self.token_prefix, self.token_template, counter, self.token_suffix
                        )
                    },
                );

                self.token_template_counter = counter;

                (Some(cluster), update_type)
            }
            None => {
                self.clusters_counter += 1;
                let cluster_id = self.clusters_counter;

                let cluster_ref = Self::add_seq_to_prefix_tree(
                    &mut self.root_node,
                    cluster_id,
                    &content_tokens,
                    self.log_cluster_depth,
                    self.max_children,
                    self.parametrize_numeric_tokens,
                );

                if cluster_ref.is_none() {
                    return (None, UpdateType::None);
                }

                (cluster_ref, UpdateType::Created)
            }
        }
    }

    fn tree_search(
        root_node: &Node,
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        log_cluster_depth: usize,
        token_template: &String,
    ) -> Option<usize> {
        let token_count = tokens.len();

        // At first level, children are grouped by token count
        let cur_node = root_node.get(&token_count.to_string())?;

        if token_count == 0 {
            return cur_node.get_first_cluster_id();
        }

        let mut cur_node = cur_node.search(tokens, log_cluster_depth)?;

        Self::fast_match(cur_node, tokens, sim_th, include_params, token_template)
    }

    fn fast_match(
        node: &Node,
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        token_template: &String,
    ) -> Option<usize> {
        let mut max_sim = -1.0;
        let mut max_param_count = -1;
        let mut max_cluster: Option<Arc<Mutex<LogCluster>>> = None;

        let clusters = node.get_clusters();
        for cluster in clusters {
            let (cur_sim, param_count) = Self::get_seq_distance(
                &cluster.lock().unwrap().get_tokens(),
                tokens,
                token_template,
                include_params,
            );
            if cur_sim > max_sim || (cur_sim == max_sim && param_count > max_param_count) {
                max_sim = cur_sim;
                max_param_count = param_count;
                max_cluster = Some(cluster);
            }
        }

        if max_sim >= sim_th {
            max_cluster.map(|c| c.lock().unwrap().get_cluster_id())
        } else {
            None
        }
    }

    fn full_match(
        node: &Node,
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        token_template: &String,
    ) -> Option<usize> {
        if let Some(id) = Self::fast_match(node, tokens, sim_th, include_params, token_template) {
            return Some(id);
        }

        for n in node.children() {
            if let Some(id) = Self::full_match(n, tokens, sim_th, include_params, token_template) {
                return Some(id);
            }
        }

        None
    }

    fn get_seq_distance(
        seq1: &[String],
        seq2: &[String],
        token_template: &String,
        include_params: bool,
    ) -> (f64, i32) {
        if seq1.len() != seq2.len() {
            return (0.0, 0);
        }

        if seq1.is_empty() {
            return (1.0, 0);
        }

        let mut sim_tokens = 0;
        let mut param_count = 0;

        for (token1, token2) in seq1.iter().zip(seq2.iter()) {
            if Self::is_token(token_template, token1) {
                param_count += 1;
                continue;
            }
            if token1 == token2 {
                sim_tokens += 1;
            }
        }

        if include_params {
            sim_tokens += param_count;
        }

        let ret_val = sim_tokens as f64 / seq1.len() as f64;
        (ret_val, param_count)
    }

    fn is_token(template: &String, token: &String) -> bool {
        token.starts_with(template)
    }

    fn create_template(&mut self, seq1: &[String], seq2: &[String]) -> Vec<String> {
        seq1.iter()
            .zip(seq2.iter())
            .map(|(t1, t2)| {
                if t1 == t2 {
                    t2.clone()
                } else {
                    self.get_next_token()
                }
            })
            .collect()
    }

    fn get_next_token(&mut self) -> String {
        self.token_template_counter += 1;
        format!(
            "{}{}{}{}",
            self.token_prefix, self.token_template, self.token_template_counter, self.token_suffix
        )
    }

    fn add_seq_to_prefix_tree(
        root_node: &mut Node,
        cluster_id: usize,
        tokens: &Vec<String>,
        log_cluster_depth: usize,
        max_children: usize,
        parametrize_numeric_tokens: bool,
    ) -> Option<Arc<Mutex<LogCluster>>> {
        let token_count = tokens.len();
        let token_count_str = token_count.to_string();

        let first_layer_node = root_node.get_or_insert_child(&token_count_str);

        let mut cur_node = first_layer_node;

        cur_node.add_cluster(
            cluster_id,
            tokens,
            log_cluster_depth,
            max_children,
            parametrize_numeric_tokens,
        )
    }

    pub fn match_cluster(
        &self,
        content: &str,
        strategy: SearchStrategy,
    ) -> Option<Arc<Mutex<LogCluster>>> {
        let required_sim_th = 1.0;

        let tokens = self.get_content_as_tokens(content);

        let full_search = || {
            let token_count = tokens.len();

            // At first level, children are grouped by token count
            let cur_node = self.root_node.get(&token_count.to_string())?;

            Self::full_match(
                cur_node,
                &tokens,
                required_sim_th,
                true,
                &self.token_template_check,
            )
            .and_then(|id| cluster::LogCluster::get_cluster_by_id(&id))
        };

        match strategy {
            SearchStrategy::Full => full_search(),

            SearchStrategy::Fast => Self::tree_search(
                &self.root_node,
                &tokens,
                required_sim_th,
                true,
                self.log_cluster_depth,
                &self.token_template_check,
            )
            .and_then(|id| cluster::LogCluster::get_cluster_by_id(&id)),

            SearchStrategy::Fallback => Self::tree_search(
                &self.root_node,
                &tokens,
                required_sim_th,
                true,
                self.log_cluster_depth,
                &self.token_template_check,
            )
            .and_then(|id| cluster::LogCluster::get_cluster_by_id(&id))
            .or_else(full_search),
        }
    }

    pub fn print_tree<W: Write>(&self, writer: &mut W, max_clusters: usize) -> io::Result<()> {
        // self.print_node("root", &self.root_node, 0, writer, max_clusters)
        self.root_node.print("root", 0, writer, max_clusters)
    }

    pub fn get_clusters(&self) -> Vec<LogCluster> {
        let mut clusters = Vec::new();
        let mut append_clusters = |n: &Node| {
            for c in n.get_clusters() {
                clusters.push(c.lock().unwrap().clone());
            }
        };

        append_clusters(&self.root_node);

        for n in self.root_node.children() {
            append_clusters(n);
        }

        clusters
    }
}
