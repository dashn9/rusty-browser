use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
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

pub fn format_compact(nodes: &[UiNode]) -> String {
    let mut lines = Vec::with_capacity(nodes.len());
    for n in nodes {
        let mut line = format!("{} {}", n.id, n.role);
        if let Some(pid) = n.parent_id {
            line.push_str(&format!(" (parent:{pid})"));
        }
        if let Some(name) = &n.name {
            line.push_str(&format!(" \"{name}\""));
        }
        if let Some(value) = &n.value {
            line.push_str(&format!(" ={value:?}"));
        }
        lines.push(line);
    }
    lines.join("\n")
}
