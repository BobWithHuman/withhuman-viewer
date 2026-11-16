//! One Call worth a look, laid out for the security team to judge: what the person asked, what
//! the agent was about to do, what made it (the Call whose result told it, and the words), and
//! where else the same thing was read. Built from the session's graph, as the route map reads
//! it, and from every Call's facts.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Value, json};

use super::main_page::{Severity, decision_key, machine_of_fact, prompt_line};
use super::secrets_web::Owners;
use crate::api::Backend;
use crate::layout::Model;
use crate::metro::Metro;
use crate::model::CallFact;

/// The sessions listed as having read the same thing, and the machine's other Calls.
const LISTED: usize = 30;

/// The case of Call `seq` of `session`, or why it could not be read.
pub async fn case(
    backend: &Backend,
    session: &str,
    seq: i64,
    facts: &[CallFact],
    owners: &Owners,
) -> Value {
    let mut out = json!({ "session": session, "seq": seq });
    let graph = match backend.graph(session).await {
        Ok(graph) => graph,
        Err(e) => {
            out["error"] = json!(e);
            return out;
        }
    };
    laid_out(&graph, session, seq, facts, owners)
}

/// The case from the session's graph once read.
fn laid_out(
    graph: &crate::model::GraphData,
    session: &str,
    seq: i64,
    facts: &[CallFact],
    owners: &Owners,
) -> Value {
    let mut out = json!({ "session": session, "seq": seq });
    let metro = Metro::build_with(&Model::build(graph), &HashSet::new());
    let station = |seq: i64| {
        metro
            .stations
            .iter()
            .find(|s| s.seq == seq || s.members.contains(&seq))
    };
    // What the person asked last before the Call.
    let ask = metro
        .asks
        .iter()
        .rfind(|a| a.seq < seq && a.agent.is_none());
    out["request"] =
        json!(ask.map(|a| json!({ "seq": a.seq, "text": a.text, "intent": a.intent, "at": a.at })));
    let Some(st) = station(seq) else {
        return out;
    };
    // What told the agent to: the Call whose result it was, and the words.
    let told = st.source.as_ref().map(|src| {
        let by = station(src.by);
        json!({
            "seq": src.by,
            "tool": by.map(|s| s.tool.clone()),
            "target": by.map(|s| s.what.clone()),
            "at": by.and_then(|s| s.at),
            "quote": src.quote,
            "paragraph": src.paragraph,
            "probability": src.probability,
            "error": src.error,
        })
    });
    let read = st
        .source
        .as_ref()
        .and_then(|src| station(src.by))
        .map(|s| s.what.clone())
        .filter(|w| !w.trim().is_empty());
    out["told"] = json!(told);
    out["traced"] = json!(!st.loose);

    // Where else the same thing was read: other sessions with a Call on the same target.
    if let Some(read) = &read {
        let mut by: BTreeMap<&str, (&CallFact, usize)> = BTreeMap::new();
        for f in facts
            .iter()
            .filter(|f| f.session != session && f.target.as_deref() == Some(read))
        {
            by.entry(f.session.as_str()).or_insert((f, 0));
        }
        for f in facts.iter().filter(|f| Severity::of(f).is_some()) {
            if let Some(row) = by.get_mut(f.session.as_str()) {
                row.1 += 1;
            }
        }
        let mut rows: Vec<_> = by.into_values().collect();
        rows.sort_by_key(|(f, flagged)| (std::cmp::Reverse(*flagged), std::cmp::Reverse(f.at)));
        out["elsewhere_total"] = json!(rows.len());
        out["elsewhere"] = rows
            .into_iter()
            .take(LISTED)
            .map(|(f, flagged)| {
                json!({
                    "session": f.session, "seq": f.seq, "machine": machine_of_fact(f), "at": f.at,
                    "prompt": f.prompt.as_deref().map(prompt_line), "flagged": flagged,
                })
            })
            .collect();
    }
    // Whose session it is, and that person's other Calls worth a look on any machine.
    let whose = |f: &CallFact| owners.user(&f.session, f.client.as_deref().unwrap_or_default());
    let this = facts.iter().find(|f| f.session == session && f.seq == seq);
    if let Some(user) = this.and_then(whose) {
        out["user"] = json!(user);
        let mut other: Vec<&CallFact> = facts
            .iter()
            .filter(|f| f.session != session && Severity::of(f).is_some() && whose(f) == Some(user))
            .collect();
        other.sort_by_key(|f| std::cmp::Reverse(f.at));
        out["user_recent"] = other
            .into_iter()
            .take(LISTED)
            .map(|f| {
                json!({
                    "session": f.session, "seq": f.seq, "at": f.at, "tool": f.tool, "target": f.target,
                    "verdict": decision_key(f).to_lowercase(), "machine": machine_of_fact(f),
                })
            })
            .collect();
    }
    // The machine's other Calls worth a look, the newest first.
    let machine = facts
        .iter()
        .find(|f| f.session == session && f.seq == seq)
        .map(machine_of_fact);
    if let Some(machine) = machine {
        let mut other: Vec<&CallFact> = facts
            .iter()
            .filter(|f| {
                !(f.session == session && f.seq == seq)
                    && Severity::of(f).is_some()
                    && machine_of_fact(f) == machine
            })
            .collect();
        other.sort_by_key(|f| std::cmp::Reverse(f.at));
        out["machine_recent"] = other
            .into_iter()
            .take(LISTED)
            .map(|f| {
                json!({
                    "session": f.session, "seq": f.seq, "at": f.at, "tool": f.tool, "target": f.target,
                    "verdict": decision_key(f).to_lowercase(),
                })
            })
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In the demo, a session follows a README into sending `.env` out: the Call stopped is
    /// laid out with the request before it and the Call whose result told it.
    #[test]
    fn a_call_a_tool_result_made_is_laid_out_with_what_told_it() {
        let demo = crate::demo::Demo::load();
        let facts: Vec<CallFact> = serde_json::from_value(demo.call_facts()).unwrap();
        let told = facts
            .iter()
            .find(|f| f.link.as_deref() == Some("segment") && f.decision.as_deref() == Some("DENY"))
            .expect("the demo has a Call a tool result made");
        let graph = serde_json::from_value(demo.graph(&told.session).unwrap()).unwrap();
        let case = laid_out(&graph, &told.session, told.seq, &facts, &Owners::default());
        assert!(
            case["request"]["text"]
                .as_str()
                .is_some_and(|t| !t.is_empty())
        );
        assert!(case["told"]["seq"].as_i64().is_some_and(|by| by < told.seq));
        assert!(case["told"]["probability"].as_f64().is_some());
        assert!(case["machine_recent"].is_array());
    }
}
