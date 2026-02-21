use std::{
    collections::HashMap,
    io::{self, Write},
};

use serde::{Deserialize, Serialize};
use strum_macros::Display;
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
            .zip(self.log_template_tokens.iter())
            .map(|(t1, t2)| {
                if t1 == t2 || is_token(t2) {
                    t2.clone()
                } else {
                    get_next_token()
                }
            })
            .collect();

        self.size += 1;
        if new_tokens != self.log_template_tokens {
            self.log_template_tokens = new_tokens;
            UpdateType::Updated
        } else {
            UpdateType::None
        }
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
    pub cluster_ids: Vec<usize>,
    children: HashMap<String, Box<Node>>,
    param_child: Option<Box<Node>>,
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    pub fn new() -> Self {
        Self {
            cluster_ids: Vec::new(),
            children: HashMap::new(),
            param_child: None,
        }
    }

    pub fn get(&self, token: &String) -> Option<&Box<Node>> {
        self.children.get(token)
    }

    pub fn param(&self) -> Option<&Box<Node>> {
        self.param_child.as_ref()
    }

    pub fn find_next(&self, token: &String) -> Option<&Box<Node>> {
        self.children
            .get(token)
            .or_else(|| self.param_child.as_ref())
    }

    pub fn first_cluster_id(&self) -> Option<usize> {
        self.cluster_ids.first().copied()
    }

    pub fn get_or_insert(&mut self, token: &str) -> &mut Node {
        self.children
            .entry(token.to_owned())
            .or_insert_with(|| Box::new(Node::new()))
            .as_mut()
    }
    pub fn has_child(&self, token: &str) -> bool {
        self.children.contains_key(token)
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
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

    pub fn get_or_insert_param(&mut self) -> &mut Node {
        self.param_child
            .get_or_insert_with(|| Box::new(Node::new()))
            .as_mut()
    }

    pub fn has_param(&self) -> bool {
        self.param_child.is_some()
    }

    pub fn get_param_mut(&mut self) -> Option<&mut Node> {
        self.param_child.as_deref_mut()
    }

    pub fn children(&self) -> Vec<&Node> {
        let mut result: Vec<&Node> = self.children.values().map(|n| n.as_ref()).collect();
        if let Some(param_child) = &self.param_child {
            result.push(param_child.as_ref());
        }
        result
    }

    pub fn print<W: Write>(
        &self,
        token: &str,
        depth: usize,
        writer: &mut W,
        max_clusters: usize,
        id_to_cluster: &HashMap<usize, LogCluster>,
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

        if !self.cluster_ids.is_empty() {
            out_str += &format!(" (cluster_count={})", self.cluster_ids.len());
        }

        writeln!(writer, "{}", out_str)?;

        for (child_token, child_node) in &self.children {
            child_node.print(child_token, depth + 1, writer, max_clusters, id_to_cluster)?;
        }

        for cid in self.cluster_ids.iter().take(max_clusters) {
            if let Some(cluster) = id_to_cluster.get(cid) {
                let cluster_str = format!("{}\t{}", "\t".repeat(depth + 1), cluster);
                writeln!(writer, "{}", cluster_str)?;
            }
        }

        Ok(())
    }
}
