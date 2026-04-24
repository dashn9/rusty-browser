use std::collections::{HashMap, HashSet};

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
    let before_map: HashMap<i64, &UiNode> =
        before.iter().filter(|n| n.id != 0).map(|n| (n.id, n)).collect();
    let after_ids: HashSet<i64> =
        after.iter().filter(|n| n.id != 0).map(|n| n.id).collect();
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
        if !after_ids.contains(&n.id) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: i64, role: &str) -> UiNode {
        UiNode { id, role: role.to_string(), name: None, parent_id: None, value: None, properties: None }
    }

    fn node_named(id: i64, role: &str, name: &str) -> UiNode {
        UiNode { id, role: role.to_string(), name: Some(name.to_string()), parent_id: None, value: None, properties: None }
    }

    fn node_valued(id: i64, role: &str, value: &str) -> UiNode {
        UiNode { id, role: role.to_string(), name: None, parent_id: None, value: Some(value.to_string()), properties: None }
    }

    // ---- diff: empty inputs ----

    #[test]
    fn diff_both_empty() {
        let result = diff(&[], &[]);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
    }

    #[test]
    fn diff_before_empty_all_added() {
        let after = vec![node(1, "button"), node(2, "input")];
        let result = diff(&[], &after);
        assert_eq!(result.added.len(), 2);
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
        let ids: Vec<i64> = result.added.iter().map(|n| n.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn diff_after_empty_all_removed() {
        let before = vec![node(1, "button"), node(2, "input")];
        let result = diff(&before, &[]);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert_eq!(result.removed.len(), 2);
    }

    // ---- diff: added ----

    #[test]
    fn diff_detects_added_node() {
        let before = vec![node(1, "button")];
        let after = vec![node(1, "button"), node(2, "input")];
        let result = diff(&before, &after);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].id, 2);
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
    }

    // ---- diff: removed ----

    #[test]
    fn diff_detects_removed_node() {
        let before = vec![node(1, "button"), node(2, "input")];
        let after = vec![node(1, "button")];
        let result = diff(&before, &after);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].id, 2);
    }

    // ---- diff: changed ----

    #[test]
    fn diff_detects_value_change() {
        let before = vec![node_valued(1, "input", "old")];
        let after = vec![node_valued(1, "input", "new")];
        let result = diff(&before, &after);
        assert!(result.added.is_empty());
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].value.as_deref(), Some("new"));
        assert!(result.removed.is_empty());
    }

    #[test]
    fn diff_detects_name_change() {
        let before = vec![node_named(1, "button", "Submit")];
        let after = vec![node_named(1, "button", "Save")];
        let result = diff(&before, &after);
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].name.as_deref(), Some("Save"));
    }

    #[test]
    fn diff_detects_role_change() {
        let before = vec![node(1, "button")];
        let after = vec![node(1, "link")];
        let result = diff(&before, &after);
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].role, "link");
    }

    #[test]
    fn diff_no_change_when_identical() {
        let nodes = vec![node_named(1, "button", "OK"), node_valued(2, "input", "hello")];
        let result = diff(&nodes, &nodes);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
    }

    // ---- diff: id == 0 filtering ----

    #[test]
    fn diff_ignores_node_id_zero_in_after() {
        let before = vec![node(1, "button")];
        let after = vec![node(1, "button"), node(0, "ignored")];
        let result = diff(&before, &after);
        assert!(result.added.is_empty(), "node id=0 must not appear in added");
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
    }

    #[test]
    fn diff_ignores_node_id_zero_in_before() {
        let before = vec![node(0, "ignored"), node(1, "button")];
        let after = vec![node(1, "button")];
        let result = diff(&before, &after);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty(), "node id=0 must not appear in removed");
    }

    #[test]
    fn diff_all_zero_ids_produces_empty_diff() {
        let before = vec![node(0, "a"), node(0, "b")];
        let after = vec![node(0, "c")];
        let result = diff(&before, &after);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.removed.is_empty());
    }

    // ---- diff: multiple changes at once ----

    #[test]
    fn diff_mixed_added_changed_removed() {
        let before = vec![node(1, "button"), node(2, "input"), node(3, "link")];
        let after = vec![
            node_named(1, "button", "Click me"), // changed: name added
            node(2, "input"),                    // unchanged
            node(4, "div"),                      // added (new id)
            // node 3 removed
        ];
        let result = diff(&before, &after);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].id, 4);
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].id, 1);
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].id, 3);
    }

    // ---- format_compact ----

    #[test]
    fn format_compact_empty() {
        assert_eq!(format_compact(&[]), "");
    }

    #[test]
    fn format_compact_single_node_minimal() {
        let n = node(42, "button");
        let out = format_compact(&[n]);
        assert_eq!(out, "42 button");
    }

    #[test]
    fn format_compact_includes_name() {
        let n = node_named(1, "link", "Click here");
        let out = format_compact(&[n]);
        assert!(out.contains("\"Click here\""), "formatted output: {out}");
        assert!(out.contains("link"));
    }

    #[test]
    fn format_compact_includes_value() {
        let n = node_valued(5, "input", "hello");
        let out = format_compact(&[n]);
        assert!(out.contains("value=\"hello\""), "formatted output: {out}");
    }

    #[test]
    fn format_compact_includes_parent_id() {
        let n = UiNode {
            id: 7,
            role: "span".to_string(),
            name: None,
            parent_id: Some(3),
            value: None,
            properties: None,
        };
        let out = format_compact(&[n]);
        assert!(out.contains("(parent:3)"), "formatted output: {out}");
    }

    #[test]
    fn format_compact_multiple_nodes_newline_separated() {
        let nodes = vec![node(1, "button"), node(2, "input"), node(3, "div")];
        let out = format_compact(&nodes);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("1 button"));
        assert!(lines[1].starts_with("2 input"));
        assert!(lines[2].starts_with("3 div"));
    }

    #[test]
    fn format_compact_all_optional_fields() {
        let n = UiNode {
            id: 99,
            role: "combobox".to_string(),
            name: Some("Country".to_string()),
            parent_id: Some(10),
            value: Some("US".to_string()),
            properties: None,
        };
        let out = format_compact(&[n]);
        assert!(out.contains("99 combobox"));
        assert!(out.contains("(parent:10)"));
        assert!(out.contains("\"Country\""));
        assert!(out.contains("value=\"US\""));
    }
}
