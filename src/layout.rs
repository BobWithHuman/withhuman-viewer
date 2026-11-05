//! A session's graph read for the inspector and the map: nodes by number, each result's Call,
//! the cause edges, and the chain a Call was traced along. `crate::metro` places the map.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::model::{GraphData, Node};

/// Drawn as what came before them, so every edge lands on a node that is on screen.
const HIDDEN: &[&str] = &["ToolBatch", "ToolDescriptions"];

/// A cause edge, from a Call to the node it was traced to.
#[derive(Clone, Debug, PartialEq)]
pub struct Cause {
    pub from: i64,
    pub to: i64,
    /// The node the edge points at before hidden nodes are folded into what came before.
    pub via: i64,
    pub paragraph: Option<i64>,
    pub probability: Option<f64>,
}

impl Cause {
    pub fn id(&self) -> String {
        format!(
            "c{}-{}-{}",
            self.from,
            self.via,
            self.paragraph.map(|p| p.to_string()).unwrap_or_default()
        )
    }
}

/// A session's graph, read for drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub nodes: Vec<Node>,
    index: HashMap<i64, usize>,
    /// A result's Call.
    pub call_of: HashMap<i64, i64>,
    results_of: HashMap<i64, Vec<i64>>,
    group_of: HashMap<i64, i64>,
    subgraph_parent: HashMap<i64, String>,
    /// The shown nodes, in the API's order.
    shown: Vec<i64>,
    flows: Vec<(i64, i64)>,
    pub causes: Vec<Cause>,
}

/// What a selection picks out: element ids to paint, the rest fades.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Highlight {
    pub nodes: Vec<String>,
    pub edges: Vec<String>,
}

impl Model {
    pub fn build(data: &GraphData) -> Self {
        let nodes = data.nodes.clone();
        let index: HashMap<i64, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.seq(), i))
            .collect();
        let mut call_of = HashMap::new();
        let mut previous = HashMap::new();
        let mut call_order = Vec::new();
        for e in &data.edges {
            let (Some(source), Some(target)) = (e.source, e.target) else {
                continue;
            };
            if e.kind == "RESULT_OF"
                && index.contains_key(&target)
                && call_of.insert(source, target).is_none()
            {
                call_order.push(source);
            }
            if e.kind == "NEXT" && !e.is_cause() {
                previous.insert(target, source);
            }
        }
        let mut results_of: HashMap<i64, Vec<i64>> = HashMap::new();
        for result in call_order {
            results_of.entry(call_of[&result]).or_default().push(result);
        }
        let shown = |seq: i64| {
            index
                .get(&seq)
                .is_some_and(|&i| !HIDDEN.contains(&nodes[i].kind.as_str()))
        };
        let rep = |seq: i64| {
            let mut seq = seq;
            for _ in 0..=50 {
                if !index.contains_key(&seq) {
                    return None;
                }
                if shown(seq) {
                    return Some(seq);
                }
                seq = *previous.get(&seq)?;
            }
            None
        };
        let mut flows = Vec::new();
        let mut seen = HashSet::new();
        for e in data
            .edges
            .iter()
            .filter(|e| e.kind == "NEXT" && !e.is_cause())
        {
            let (Some(a), Some(b)) = (e.source.and_then(rep), e.target.and_then(rep)) else {
                continue;
            };
            if a != b && seen.insert((a, b)) {
                flows.push((a, b));
            }
        }
        let causes = data
            .edges
            .iter()
            .filter(|e| e.is_cause())
            .filter_map(|e| {
                let (from, to) = (e.source.and_then(rep)?, e.target.and_then(rep)?);
                (from != to).then(|| Cause {
                    from,
                    to,
                    via: e.target.unwrap_or(to),
                    paragraph: e.paragraph,
                    probability: e.probability(),
                })
            })
            .collect();
        let mut group_of = HashMap::new();
        let mut subgraph_parent = HashMap::new();
        for s in &data.subgraphs {
            let Some(number) = s.number() else { continue };
            for seq in s.members.iter().flatten() {
                group_of.insert(*seq, number);
            }
            subgraph_parent.insert(number, s.parent().unwrap_or("").to_owned());
        }
        let shown_list = nodes.iter().map(Node::seq).filter(|&s| shown(s)).collect();
        Self {
            nodes,
            index,
            call_of,
            results_of,
            group_of,
            subgraph_parent,
            shown: shown_list,
            flows,
            causes,
        }
    }

    pub fn node(&self, seq: i64) -> Option<&Node> {
        self.index.get(&seq).map(|&i| &self.nodes[i])
    }

    pub fn subgraph_of(&self, seq: i64) -> Option<i64> {
        self.group_of.get(&seq).copied()
    }

    /// The chain a Call was traced along: the Call, the result that caused it, the Call that
    /// result came from, and so on up to the prompt.
    pub fn chain(&self, seq: i64) -> Highlight {
        let mut out = Highlight::default();
        let mut seen = HashSet::new();
        let mut stack = vec![seq];
        while let Some(call) = stack.pop() {
            if !seen.insert(call) {
                continue;
            }
            out.nodes.push(format!("n{call}"));
            for c in self.causes.iter().filter(|c| c.from == call) {
                out.edges.push(c.id());
                out.nodes.push(format!("n{}", c.to));
                if let Some(origin) = self.call_of.get(&c.to) {
                    out.edges.push(format!("f{origin}-{}", c.to));
                    stack.push(*origin);
                }
            }
        }
        if out.edges.is_empty() {
            out.nodes.truncate(1);
        }
        out
    }

    /// The causes traced to the paragraphs of a tool result, by paragraph index.
    pub fn paragraph_causes(&self, seq: i64) -> HashMap<i64, Vec<&Cause>> {
        let mut by: HashMap<i64, Vec<&Cause>> = HashMap::new();
        for c in self.causes.iter().filter(|c| c.via == seq) {
            if let Some(p) = c.paragraph {
                by.entry(p).or_default().push(c);
            }
        }
        by
    }
}

pub fn decision_color(decision: &str) -> &'static str {
    match decision {
        "ALLOW" => "--allow",
        "DENY" => "--deny",
        "REVIEW" => "--review",
        _ => "--other",
    }
}

pub fn icon(kind: &str) -> &'static str {
    match kind {
        "Request" => "👤",
        "Intent" => "🎯",
        "Message" => "💬",
        "Segment" => "📄",
        "Error" => "⚠",
        "ConfigChange" => "⚙",
        "SubagentStart" => "⤵",
        "SubagentStop" => "⤴",
        _ => "",
    }
}

pub fn label(node: &Node) -> String {
    let s = &node.summary;
    let text = |v: &Option<String>| v.clone().unwrap_or_default();
    let tool = text(&s.tool_name);
    match node.kind.as_str() {
        "Request" => format!("{} {}", icon("Request"), clip(&text(&s.prompt), 26)),
        "Intent" => format!("{} {}", icon("Intent"), clip(&text(&s.intent), 26)),
        "Message" => format!("{} {}", icon("Message"), clip(&text(&s.text), 26)),
        "Call" => clip(
            &format!(
                "{} {}",
                if tool.is_empty() { "Call" } else { &tool },
                short_path(&text(&s.brief))
            ),
            28,
        ),
        "Segment" => format!("{} {}", icon("Segment"), clip(&format!("{tool} 결과"), 26)),
        "Error" => format!(
            "{} {}",
            icon("Error"),
            clip(&format!("{tool} 오류 {}", text(&s.error)), 26)
        ),
        "ConfigChange" => format!("{} {}", icon("ConfigChange"), clip(&text(&s.file_path), 24)),
        kind => format!("{} {kind}", icon(kind)).trim().to_owned(),
    }
}

/// An absolute path as its last two parts (`src/refund.py`), so a node's label shows the file
/// rather than the folders every call shares; anything else as it is.
pub fn short_path(text: &str) -> String {
    if !text.starts_with('/') || text.contains(char::is_whitespace) {
        return text.to_owned();
    }
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

/// Whitespace folded to single spaces, cut to `n` characters with an ellipsis.
pub fn clip(text: &str, n: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > n {
        format!("{}…", flat.chars().take(n).collect::<String>())
    } else {
        flat
    }
}

/// A probability with at most two decimals and no trailing zeros.
pub fn percent(p: f64) -> String {
    let text = format!("{p:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::Demo;

    /// Hand-made sessions whose counts were checked against the page's own code; the demo
    /// fixture is re-exported from the engine and changes.
    const CASES: &str = include_str!("../mock/layout_cases.json");

    fn fixture(session: &str) -> Model {
        let graph = Demo::parse(CASES).graph(session).expect("fixture session");
        Model::build(&serde_json::from_value(graph).expect("graph shape"))
    }

    #[test]
    fn chain_runs_from_a_call_up_to_the_prompt() {
        let chain = fixture("demo-injection-0001").chain(7);
        assert_eq!(chain.nodes, ["n7", "n4", "n3", "n1"]);
        assert_eq!(chain.edges, ["c7-4-2", "f3-4", "c3-1-"]);
    }

    #[test]
    fn paragraphs_know_the_calls_traced_to_them() {
        let model = fixture("demo-injection-0001");
        let readme = model.paragraph_causes(4);
        // The hidden ¶2 led to the denied Edit, ¶1 to the reviewed Write.
        assert_eq!(readme[&2].iter().map(|c| c.from).collect::<Vec<_>>(), [7]);
        assert_eq!(readme[&1].iter().map(|c| c.from).collect::<Vec<_>>(), [8]);
        assert!(!readme.contains_key(&0));
    }

    #[test]
    fn labels_and_numbers_read_like_the_page() {
        assert_eq!(clip("a  b\nc", 3), "a b…");
        assert_eq!(
            short_path("/Users/dev/src/payments-service/src/refund.py"),
            "src/refund.py"
        );
        assert_eq!(short_path("/README.md"), "README.md");
        assert_eq!(short_path("make test"), "make test");
        assert_eq!(short_path("/bin/ls -la /tmp"), "/bin/ls -la /tmp");
        assert_eq!(percent(0.70), "0.7");
        assert_eq!(percent(1.0), "1");
        assert_eq!(percent(0.0), "0");
    }
}
