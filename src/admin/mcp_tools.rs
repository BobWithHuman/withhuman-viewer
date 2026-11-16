//! What an MCP tool does, told from its name with the registry's tables
//! (`data/mcp/tool_actions.json`, made by `scripts/mcp_registry.py actions`): the action the
//! registry gives a tool of that name, or else the one most registry tools starting with the
//! same word have. The actions are the registry's: `read`, `search`, `create`, `update`,
//! `delete`, `send`, `transact`, `execute`, `access`, `browse`, `compute`, `plan`, `delegate`.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct Table {
    /// A tool name's first word to the action most such tools have.
    verbs: HashMap<String, String>,
    /// The tool names whose action differs from their first word's.
    names: HashMap<String, String>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../data/mcp/tool_actions.json"))
        .expect("tool_actions.json is the table the script writes")
});

/// A tool name's first word, lowercased: `sendEmail`, `send_email` and `send-email` give `send`.
fn first_word(name: &str) -> String {
    let mut word = String::new();
    let mut last_lower = false;
    for c in name
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .chars()
    {
        if !c.is_ascii_alphanumeric() || (c.is_ascii_uppercase() && last_lower) {
            break;
        }
        last_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        word.push(c.to_ascii_lowercase());
    }
    word
}

/// The action of an MCP tool by its name (without the server's), when the registry tells.
pub(super) fn action(tool: &str) -> Option<&'static str> {
    let table = &*TABLE;
    table
        .names
        .get(&tool.to_lowercase())
        .or_else(|| table.verbs.get(&first_word(tool)))
        .map(String::as_str)
}

/// Whether a tool puts what it is given somewhere outside: it sends, pays, or writes to the
/// service. A tool that reads, searches or deletes takes its arguments in without keeping them
/// as content, and one the registry cannot tell is taken as not sending.
pub(super) fn sends_out(tool: &str) -> bool {
    matches!(
        action(tool),
        Some("send" | "transact" | "create" | "update" | "delegate")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tools_action_is_told_from_its_name_or_its_first_word() {
        assert_eq!(first_word("sendEmail"), "send");
        assert_eq!(first_word("get-balance"), "get");
        assert_eq!(action("send_email"), Some("send"));
        assert_eq!(action("get_balance"), Some("read"));
        assert_eq!(action("delete_file"), Some("delete"));
        // No registry tool has these names; their first words tell.
        assert_eq!(action("post_message"), Some("send"));
        assert_eq!(action("update_scheduled_transaction"), Some("update"));
        assert!(sends_out("send_money"));
        assert!(sends_out("create_issue"));
        assert!(!sends_out("read_file"));
        assert!(!sends_out("search_code"));
        assert!(!sends_out("zzz_unknown"));
    }
}
