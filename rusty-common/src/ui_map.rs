use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiNode {
    pub id: i64,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiMapDiff {
    pub added: Vec<UiNode>,
    pub changed: Vec<UiNode>,
    pub removed: Vec<UiNode>,
}

pub fn diff(before: &[UiNode], after: &[UiNode]) -> UiMapDiff {
    use std::collections::HashMap;
    let before_map: HashMap<i64, &UiNode> = before.iter().filter(|n| n.id != 0).map(|n| (n.id, n)).collect();
    let after_map: HashMap<i64, &UiNode> = after.iter().filter(|n| n.id != 0).map(|n| (n.id, n)).collect();
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for n in after.iter().filter(|n| n.id != 0) {
        match before_map.get(&n.id) {
            None => added.push(n.clone()),
            Some(prev) => {
                if prev.value != n.value || prev.name != n.name || prev.role != n.role {
                    changed.push(n.clone());
                }
            }
        }
    }
    for n in before.iter().filter(|n| n.id != 0) {
        if !after_map.contains_key(&n.id) {
            removed.push(n.clone());
        }
    }

    UiMapDiff { added, changed, removed }
}

fn format_node(n: &UiNode) -> String {
    let mut s = format!("{} {}", n.id, n.role);
    if let Some(pid) = n.parent_id {
        s.push_str(&format!(" (parent:{pid})"));
    }
    if let Some(name) = &n.name {
        s.push_str(&format!(" \"{name}\""));
    }
    if let Some(value) = &n.value {
        s.push_str(&format!(" value=\"{value}\""));
    }
    s
}

pub fn format_compact(nodes: &[UiNode]) -> String {
    nodes.iter().map(format_node).collect::<Vec<_>>().join("\n")
}
