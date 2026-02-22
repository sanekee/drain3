use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use strum_macros::Display;

static CLUSTER_MAP: LazyLock<Mutex<HashMap<usize, Arc<Mutex<LogCluster>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub enum SearchStrategy {
    Full,
    Fast,
    Fallback,
}

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq)]
pub enum UpdateType {
    None,
    Created,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCluster {
    pub tokens: Vec<String>,
    pub cluster_id: usize,
    pub size: usize,
}

impl LogCluster {
    pub fn new(tokens: &Vec<String>, cluster_id: usize) -> Self {
        Self {
            tokens: tokens.clone(),
            cluster_id,
            size: 1,
        }
    }

    pub fn get_tokens(&self) -> Vec<String> {
        self.tokens.clone()
    }

    pub fn get_cluster_id(&self) -> usize {
        self.cluster_id
    }

    pub fn get_template(&self) -> String {
        self.tokens.join(" ")
    }

    pub fn update_template<F1, F2>(
        &mut self,
        tokens: &[String],
        mut is_token: F1,
        mut get_next_token: F2,
    ) -> UpdateType
    where
        F1: FnMut(&String) -> bool,
        F2: FnMut() -> String,
    {
        let mut updated = false;
        let new_tokens = tokens
            .iter()
            .zip(self.tokens.iter())
            .map(|(t1, t2)| {
                if t1 == t2 || is_token(t2) {
                    t2.clone()
                } else {
                    get_next_token()
                }
            })
            .collect();

        self.size += 1;
        if new_tokens != self.tokens {
            self.tokens = new_tokens;
            UpdateType::Updated
        } else {
            UpdateType::None
        }
    }

    pub fn get_cluster_by_id(id: &usize) -> Option<Arc<Mutex<LogCluster>>> {
        CLUSTER_MAP.lock().unwrap().get(&id).cloned()
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

#[derive(Debug)]
pub struct Node {
    clusters: Vec<Arc<Mutex<LogCluster>>>,
    children: HashMap<String, Box<Node>>,
    wildcard_child: Option<Box<Node>>,
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    pub fn new() -> Self {
        Self {
            clusters: Vec::new(),
            children: HashMap::new(),
            wildcard_child: None,
        }
    }

    pub fn get(&self, token: &String) -> Option<&Node> {
        self.children.get(token).map(Box::as_ref)
    }

    pub fn get_wildcard_child(&self) -> Option<&Node> {
        self.wildcard_child.as_ref().map(|b| b.as_ref())
    }

    pub fn find_next(&self, token: &String) -> Option<&Node> {
        self.children
            .get(token)
            .map(Box::as_ref)
            .or_else(|| self.wildcard_child.as_ref().map(Box::as_ref))
    }

    pub fn first_cluster(&self) -> Option<Arc<Mutex<LogCluster>>> {
        self.clusters.first().cloned()
    }

    pub fn has_child(&self, token: &str) -> bool {
        self.children.contains_key(token)
    }

    pub fn get_clusters(&self) -> Vec<Arc<Mutex<LogCluster>>> {
        self.clusters.clone()
    }

    pub fn child_count(&self) -> usize {
        if self.wildcard_child.is_some() {
            return 1 + self.children.len();
        }
        1 + self.children.len()
    }

    pub fn get_child_mut(&mut self, token: &str) -> Option<&mut Node> {
        self.children.get_mut(token).map(|n| n.as_mut())
    }

    pub fn get_or_insert_child(&mut self, token: &str) -> &mut Node {
        self.children
            .entry(token.to_owned())
            .or_insert_with(|| Box::new(Node::new()))
            .as_mut()
    }

    pub fn get_or_insert_wildcard(&mut self) -> &mut Node {
        self.wildcard_child
            .get_or_insert_with(|| Box::new(Node::new()))
            .as_mut()
    }

    pub fn has_wildcard(&self) -> bool {
        self.wildcard_child.is_some()
    }

    pub fn get_wildcard_mut(&mut self) -> Option<&mut Node> {
        self.wildcard_child.as_deref_mut()
    }

    pub fn children(&self) -> Vec<&Node> {
        let mut result: Vec<&Node> = self.children.values().map(|n| n.as_ref()).collect();
        if let Some(wildcard_child) = &self.wildcard_child {
            result.push(wildcard_child.as_ref());
        }
        result
    }

    fn has_numbers(s: &str) -> bool {
        s.chars().any(|c| c.is_ascii_digit())
    }

    pub fn add_cluster(
        &mut self,
        cluster_id: usize,
        tokens: &Vec<String>,
        log_cluster_depth: usize,
        max_children: usize,
        wildcardetrize_numeric_tokens: bool,
    ) -> Option<Arc<Mutex<LogCluster>>> {
        let cluster = Arc::new(Mutex::new(LogCluster::new(tokens, cluster_id)));
        CLUSTER_MAP
            .lock()
            .unwrap()
            .insert(cluster_id, cluster.clone());

        let token_count = tokens.len();
        let max_node_depth = log_cluster_depth - 2;
        let mut current_depth = 1;

        if token_count == 0 {
            self.clusters.push(cluster.clone());
            return self.clusters.last().cloned();
        }

        let mut cur_node = self;

        for token in tokens {
            if current_depth >= max_node_depth || current_depth >= token_count {
                cur_node.clusters.push(cluster.clone());
                return cur_node.clusters.last().cloned();
            }

            if cur_node.has_child(token) {
                cur_node = cur_node.get_child_mut(token).unwrap();
            } else {
                let has_numbers = wildcardetrize_numeric_tokens && Self::has_numbers(token);

                if has_numbers {
                    cur_node = cur_node.get_or_insert_wildcard();
                    continue;
                }

                if cur_node.has_wildcard() {
                    if cur_node.child_count() < max_children {
                        cur_node = cur_node.get_or_insert_child(token);
                    } else {
                        cur_node = cur_node.get_wildcard_mut().unwrap();
                    }
                } else if cur_node.child_count() + 1 < max_children {
                    cur_node = cur_node.get_or_insert_child(token);
                } else if cur_node.child_count() + 1 == max_children {
                    cur_node = cur_node.get_or_insert_wildcard();
                } else {
                    cur_node = cur_node.get_or_insert_wildcard();
                }
            }
            current_depth += 1;
        }

        None
    }

    pub fn get_first_cluster_id(&self) -> Option<usize> {
        let cluster = self.clusters.first()?;
        Some(cluster.lock().unwrap().cluster_id)
    }

    pub fn search(&self, tokens: &[String], log_cluster_depth: usize) -> Option<&Node> {
        let token_count = tokens.len();

        let mut cur_node = self;
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

        Some(cur_node)
    }

    pub fn print<W: Write>(
        &self,
        token: &str,
        depth: usize,
        writer: &mut W,
        max_clusters: usize,
    ) -> io::Result<()> {
        let mut out_str = "\t".repeat(depth);

        if depth == 0 {
            out_str += &format!("<{}>", token);
        } else if depth == 1 {
            if token.chars().all(|c| c.is_ascii_digit()) {
                out_str += &format!("<L={}>", token);
            } else {
                out_str += &format!("<{}>", token);
            }
        } else {
            out_str += &format!("\"{}\"", token);
        }

        if !self.clusters.is_empty() {
            out_str += &format!(" (cluster_count={})", self.clusters.len());
        }

        writeln!(writer, "{}", out_str)?;

        for (child_token, child_node) in &self.children {
            child_node.print(child_token, depth + 1, writer, max_clusters)?;
        }

        for c in self.clusters.iter().take(max_clusters) {
            let cluster_str = format!("{}\t{}", "\t".repeat(depth + 1), c.lock().unwrap());
            writeln!(writer, "{}", cluster_str)?;
        }

        Ok(())
    }
}
