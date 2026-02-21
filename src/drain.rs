use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{self, Write},
};

use crate::cluster::{LogCluster, Node, SearchStrategy, UpdateType};

#[derive(Debug, Serialize, Deserialize)]
pub struct Drain {
    pub log_cluster_depth: usize,
    pub sim_th: f64,
    pub max_children: usize,
    pub max_clusters: Option<usize>,
    pub extra_delimiters: Vec<String>,
    pub parametrize_numeric_tokens: bool,

    pub root_node: Node,
    pub id_to_cluster: HashMap<usize, LogCluster>, // TODO: implement LRU if max_clusters is set
    pub clusters_counter: usize,

    #[serde(skip)]
    token_prefix: String,
    #[serde(skip)]
    token_suffix: String,
    #[serde(skip)]
    token_template: String,
    #[serde(skip)]
    token_template_counter: usize,
    #[serde(skip)]
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
            id_to_cluster: HashMap::new(),
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

    pub fn add_log_message(&mut self, content: &str) -> (LogCluster, UpdateType) {
        let content_tokens = self.get_content_as_tokens(content);

        let match_result = Self::tree_search(
            &self.root_node,
            &self.id_to_cluster,
            &content_tokens,
            self.sim_th,
            true,
            self.log_cluster_depth,
            &self.token_template,
        );

        match match_result {
            Some(cluster_id) => {
                let mut counter = self.token_template_counter;
                let cluster = self.id_to_cluster.get_mut(&cluster_id).unwrap();
                let update_type = cluster.update_template(
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

                (cluster.clone(), update_type)
            }
            None => {
                self.clusters_counter += 1;
                let cluster_id = self.clusters_counter;
                let cluster = LogCluster::new(content_tokens.clone(), cluster_id);
                self.id_to_cluster.insert(cluster_id, cluster.clone());

                Self::add_seq_to_prefix_tree(
                    &mut self.root_node,
                    &cluster,
                    self.log_cluster_depth,
                    self.max_children,
                    self.parametrize_numeric_tokens,
                );

                (cluster, UpdateType::Created)
            }
        }
    }

    fn tree_search(
        root_node: &Node,
        id_to_cluster: &HashMap<usize, LogCluster>,
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
            return cur_node.first_cluster_id();
        }

        let mut cur_node = cur_node;
        let max_node_depth = log_cluster_depth - 2;

        let mut cur_node_depth = 1;
        for token in tokens {
            if cur_node_depth >= max_node_depth {
                break;
            }
            if cur_node_depth == token_count {
                break;
            }

            if let Some(node) = cur_node.find_next(token) {
                cur_node = node;
            } else {
                return None;
            }
            cur_node_depth += 1;
        }

        Self::fast_match(
            id_to_cluster,
            &cur_node.cluster_ids,
            tokens,
            sim_th,
            include_params,
            token_template,
        )
    }

    fn fast_match(
        id_to_cluster: &HashMap<usize, LogCluster>,
        cluster_ids: &[usize],
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        token_template: &String,
    ) -> Option<usize> {
        let mut max_sim = -1.0;
        let mut max_param_count = -1;
        let mut max_cluster: Option<&LogCluster> = None;

        for &cluster_id in cluster_ids {
            if let Some(cluster) = id_to_cluster.get(&cluster_id) {
                let (cur_sim, param_count) = Self::get_seq_distance(
                    &cluster.log_template_tokens,
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
        }

        if max_sim >= sim_th {
            max_cluster.map(|c| c.cluster_id)
        } else {
            None
        }
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
        cluster: &LogCluster,
        log_cluster_depth: usize,
        max_children: usize,
        parametrize_numeric_tokens: bool,
    ) {
        let token_count = cluster.log_template_tokens.len();
        let token_count_str = token_count.to_string();

        let first_layer_node = root_node.get_or_insert(&token_count_str);

        let mut cur_node = first_layer_node;

        if token_count == 0 {
            cur_node.cluster_ids.push(cluster.cluster_id);
            return;
        }

        let max_node_depth = log_cluster_depth - 2;
        let mut current_depth = 1;

        for token in &cluster.log_template_tokens {
            if current_depth >= max_node_depth || current_depth >= token_count {
                cur_node.cluster_ids.push(cluster.cluster_id);
                break;
            }

            if cur_node.has_child(token) {
                cur_node = cur_node.get_child_mut(token).unwrap();
            } else {
                let has_numbers = parametrize_numeric_tokens && Self::has_numbers(token);

                if has_numbers {
                    cur_node = cur_node.get_or_insert_param();
                } else if cur_node.has_param() {
                    if cur_node.child_count() < max_children {
                        cur_node = cur_node.get_or_insert_child(token);
                    } else {
                        cur_node = cur_node.get_param_mut().unwrap();
                    }
                } else if cur_node.child_count() + 1 < max_children {
                    cur_node = cur_node.get_or_insert_child(token);
                } else if cur_node.child_count() + 1 == max_children {
                    cur_node = cur_node.get_or_insert_param();
                } else {
                    cur_node = cur_node.get_or_insert_param();
                }
            }
            current_depth += 1;
        }
    }

    pub fn match_cluster(&self, content: &str, strategy: SearchStrategy) -> Option<LogCluster> {
        let required_sim_th = 1.0;

        let content_tokens = self.get_content_as_tokens(content);

        let full_search = || {
            let all_ids = self.get_clusters_ids_for_seq_len(content_tokens.len());

            Self::fast_match(
                &self.id_to_cluster,
                &all_ids,
                &content_tokens,
                required_sim_th,
                true,
                &self.token_template_check,
            )
            .and_then(|id| self.id_to_cluster.get(&id).cloned())
        };

        match strategy {
            SearchStrategy::Full => full_search(),

            SearchStrategy::Fast => Self::tree_search(
                &self.root_node,
                &self.id_to_cluster,
                &content_tokens,
                required_sim_th,
                true,
                self.log_cluster_depth,
                &self.token_template,
            )
            .and_then(|id| self.id_to_cluster.get(&id).cloned()),

            SearchStrategy::Fallback => Self::tree_search(
                &self.root_node,
                &self.id_to_cluster,
                &content_tokens,
                required_sim_th,
                true,
                self.log_cluster_depth,
                &self.token_template,
            )
            .and_then(|id| self.id_to_cluster.get(&id).cloned())
            .or_else(full_search),
        }
    }

    pub fn get_clusters_ids_for_seq_len(&self, seq_fir: impl ToString) -> Vec<usize> {
        fn append_clusters_recursive(node: &Node, target: &mut Vec<usize>) {
            target.extend(&node.cluster_ids);

            for child in node.children() {
                append_clusters_recursive(child, target);
            }
        }

        let key = seq_fir.to_string();

        let Some(cur_node) = self.root_node.get(&key) else {
            return Vec::new();
        };

        let mut target = Vec::new();
        append_clusters_recursive(cur_node, &mut target);

        target
    }

    pub fn print_tree<W: Write>(&self, writer: &mut W, max_clusters: usize) -> io::Result<()> {
        // self.print_node("root", &self.root_node, 0, writer, max_clusters)
        self.root_node
            .print("root", 0, writer, max_clusters, &self.id_to_cluster)
    }
}
