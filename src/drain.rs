use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCluster {
    pub log_template_tokens: Vec<String>,
    pub cluster_id: usize,
    pub size: usize,
}

impl LogCluster {
    pub fn new(log_template_tokens: Vec<String>, cluster_id: usize) -> Self {
        Self {
            log_template_tokens,
            cluster_id,
            size: 1,
        }
    }

    pub fn get_template(&self) -> String {
        self.log_template_tokens.join(" ")
    }
}

impl std::fmt::Display for LogCluster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ID={:<5} : size={:<10}: {}",
            self.cluster_id,
            self.size,
            self.get_template()
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub key_to_child_node: HashMap<String, Node>,
    pub cluster_ids: Vec<usize>,
}

impl Node {
    pub fn new() -> Self {
        Self {
            key_to_child_node: HashMap::new(),
            cluster_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Drain {
    pub log_cluster_depth: usize,
    pub sim_th: f64,
    pub max_children: usize,
    pub max_clusters: Option<usize>,
    pub extra_delimiters: Vec<String>,
    pub param_str: String,
    pub parametrize_numeric_tokens: bool,

    pub root_node: Node,
    pub id_to_cluster: HashMap<usize, LogCluster>, // TODO: implement LRU if max_clusters is set
    pub clusters_counter: usize,
}

impl Drain {
    pub fn new(
        depth: usize,
        sim_th: f64,
        max_children: usize,
        max_clusters: Option<usize>,
        extra_delimiters: Vec<String>,
        param_str: String,
        parametrize_numeric_tokens: bool,
    ) -> Self {
        if depth < 3 {
            panic!("depth argument must be at least 3");
        }

        Self {
            log_cluster_depth: depth,
            sim_th,
            max_children,
            max_clusters,
            extra_delimiters,
            param_str,
            parametrize_numeric_tokens,
            root_node: Node::new(),
            id_to_cluster: HashMap::new(),
            clusters_counter: 0,
        }
    }

    // Helper to check if string has numbers
    fn has_numbers(s: &str) -> bool {
        s.chars().any(|c| c.is_digit(10))
    }

    pub fn get_content_as_tokens(&self, content: &str) -> Vec<String> {
        let mut content = content.trim().to_string();
        for delimiter in &self.extra_delimiters {
            content = content.replace(delimiter, " ");
        }
        content.split_whitespace().map(|s| s.to_string()).collect()
    }

    pub fn add_log_message(&mut self, content: &str) -> (LogCluster, String) {
        let content_tokens = self.get_content_as_tokens(content);

        let match_result = Self::tree_search(
            &self.root_node,
            &self.id_to_cluster,
            &content_tokens,
            self.sim_th,
            false,
            self.log_cluster_depth,
            &self.param_str,
        );

        match match_result {
            Some(cluster_id) => {
                let cluster = self.id_to_cluster.get_mut(&cluster_id).unwrap();
                let new_template_tokens = Self::create_template(
                    &content_tokens,
                    &cluster.log_template_tokens,
                    &self.param_str,
                );
                let mut update_type = "none";
                if new_template_tokens != cluster.log_template_tokens {
                    cluster.log_template_tokens = new_template_tokens;
                    update_type = "cluster_template_changed";
                }
                cluster.size += 1;
                return (cluster.clone(), update_type.to_string());
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
                    &self.param_str,
                    self.parametrize_numeric_tokens,
                );

                return (cluster, "cluster_created".to_string());
            }
        }
    }

    // Associated functions (static-like) to avoid borrow checker issues with `self`

    fn tree_search(
        root_node: &Node,
        id_to_cluster: &HashMap<usize, LogCluster>,
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        log_cluster_depth: usize,
        param_str: &str,
    ) -> Option<usize> {
        let token_count = tokens.len();

        // At first level, children are grouped by token count
        let cur_node = root_node.key_to_child_node.get(&token_count.to_string())?;

        if token_count == 0 {
            return cur_node.cluster_ids.first().copied();
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

            if let Some(node) = cur_node.key_to_child_node.get(token) {
                cur_node = node;
            } else if let Some(node) = cur_node.key_to_child_node.get(param_str) {
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
            param_str,
        )
    }

    fn fast_match(
        id_to_cluster: &HashMap<usize, LogCluster>,
        cluster_ids: &[usize],
        tokens: &[String],
        sim_th: f64,
        include_params: bool,
        param_str: &str,
    ) -> Option<usize> {
        let mut max_sim = -1.0;
        let mut max_param_count = -1;
        let mut max_cluster: Option<&LogCluster> = None;

        for &cluster_id in cluster_ids {
            if let Some(cluster) = id_to_cluster.get(&cluster_id) {
                let (cur_sim, param_count) = Self::get_seq_distance(
                    &cluster.log_template_tokens,
                    tokens,
                    include_params,
                    param_str,
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
        include_params: bool,
        param_str: &str,
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
            if token1 == param_str {
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

    fn create_template(seq1: &[String], seq2: &[String], param_str: &str) -> Vec<String> {
        seq1.iter()
            .zip(seq2.iter())
            .map(|(t1, t2)| {
                if t1 == t2 {
                    t2.clone()
                } else {
                    param_str.to_string()
                }
            })
            .collect()
    }

    fn add_seq_to_prefix_tree(
        root_node: &mut Node,
        cluster: &LogCluster,
        log_cluster_depth: usize,
        max_children: usize,
        param_str: &str,
        parametrize_numeric_tokens: bool,
    ) {
        let token_count = cluster.log_template_tokens.len();
        let token_count_str = token_count.to_string();

        let first_layer_node = root_node
            .key_to_child_node
            .entry(token_count_str)
            .or_insert_with(Node::new);

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

            // logic to choose next node
            if cur_node.key_to_child_node.contains_key(token) {
                cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
            } else {
                let has_numbers = parametrize_numeric_tokens && Self::has_numbers(token);
                if has_numbers {
                    if !cur_node.key_to_child_node.contains_key(param_str) {
                        cur_node
                            .key_to_child_node
                            .insert(param_str.to_string(), Node::new());
                    }
                    cur_node = cur_node.key_to_child_node.get_mut(param_str).unwrap();
                } else {
                    if cur_node.key_to_child_node.contains_key(param_str) {
                        if cur_node.key_to_child_node.len() < max_children {
                            cur_node
                                .key_to_child_node
                                .insert(token.clone(), Node::new());
                            cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
                        } else {
                            cur_node = cur_node.key_to_child_node.get_mut(param_str).unwrap();
                        }
                    } else {
                        if cur_node.key_to_child_node.len() + 1 < max_children {
                            cur_node
                                .key_to_child_node
                                .insert(token.clone(), Node::new());
                            cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
                        } else if cur_node.key_to_child_node.len() + 1 == max_children {
                            cur_node
                                .key_to_child_node
                                .insert(param_str.to_string(), Node::new());
                            cur_node = cur_node.key_to_child_node.get_mut(param_str).unwrap();
                        } else {
                            if !cur_node.key_to_child_node.contains_key(param_str) {
                                cur_node
                                    .key_to_child_node
                                    .insert(param_str.to_string(), Node::new());
                            }
                            cur_node = cur_node.key_to_child_node.get_mut(param_str).unwrap();
                        }
                    }
                }
            }

            current_depth += 1;
        }
    }
}
