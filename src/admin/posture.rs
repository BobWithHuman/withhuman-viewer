//! Whether each machine's agent keeps to the setup 정책 › 설정 기준 asks, as far as its Calls
//! show: the engine blocking there, no Calls run undecided (the hook missed or bypassed), MCP
//! servers and web hosts only from the allowlists, no attempt by the agent to change its own
//! safeguards. 정책 previews it; a page of its own waits until the engine collects each
//! machine's settings (the permission mode, the hooks, the agent's files).

use super::main_page::machine_of_fact;
use super::policy::{Posture, host_of};
use crate::metro::mcp_parts;
use crate::model::CallFact;

/// One rule's outcome on a machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mark {
    Kept,
    Broken,
    /// The rule is off, or has nothing to check against (an empty allowlist).
    Off,
}

/// A machine as 설정 점검 checks it: each rule's outcome and the Calls that broke it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Check {
    pub(super) name: String,
    /// The mode of its latest decision.
    pub(super) mode: Option<String>,
    pub(super) calls: usize,
    pub(super) last: Option<i64>,
    pub(super) undecided: Vec<CallFact>,
    pub(super) off_mcp: Vec<CallFact>,
    pub(super) off_hosts: Vec<CallFact>,
    pub(super) safeguard: Vec<CallFact>,
    pub(super) marks: [Option<Mark>; 5],
}

impl Check {
    pub(super) fn failing(&self) -> bool {
        self.marks.contains(&Some(Mark::Broken))
    }
}

/// Every machine with Calls, checked against the setup and the allowlists; the machines that
/// break the most first.
pub(super) fn checks(facts: &[CallFact], p: &Posture, mcp: &[String], hosts: &[String]) -> Vec<Check> {
    let listed = |list: &[String], v: &str| list.iter().any(|a| a.eq_ignore_ascii_case(v));
    let mut by: std::collections::BTreeMap<String, Check> = std::collections::BTreeMap::new();
    let mut latest: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for f in facts {
        let name = machine_of_fact(f);
        let c = by.entry(name.clone()).or_insert_with(|| Check { name: name.clone(), ..Check::default() });
        c.calls += 1;
        c.last = c.last.max(f.at);
        if let (Some(mode), Some(at)) = (&f.mode, f.at)
            && latest.get(&name).is_none_or(|t| at >= *t)
        {
            latest.insert(name.clone(), at);
            c.mode = Some(mode.clone());
        }
        if f.decision.is_none() {
            c.undecided.push(f.clone());
        }
        if let Some((server, _)) = f.tool.as_deref().and_then(mcp_parts)
            && !mcp.is_empty()
            && !listed(mcp, server)
        {
            c.off_mcp.push(f.clone());
        }
        if let Some(host) = f.target.as_deref().and_then(host_of)
            && !hosts.is_empty()
            && !listed(hosts, &host)
        {
            c.off_hosts.push(f.clone());
        }
        if f.rule.as_deref() == Some("safeguard::change") {
            c.safeguard.push(f.clone());
        }
    }
    let mark = |on: bool, broken: bool| Some(if !on { Mark::Off } else if broken { Mark::Broken } else { Mark::Kept });
    let mut list: Vec<Check> = by
        .into_values()
        .map(|mut c| {
            c.marks = [
                mark(p.enforce, c.mode.as_deref() != Some("enforce")),
                Some(if c.undecided.len() > p.undecided as usize { Mark::Broken } else { Mark::Kept }),
                mark(p.mcp_listed && !mcp.is_empty(), !c.off_mcp.is_empty()),
                mark(p.hosts_listed && !hosts.is_empty(), !c.off_hosts.is_empty()),
                mark(p.no_safeguard, !c.safeguard.is_empty()),
            ];
            c
        })
        .collect();
    let broken = |c: &Check| c.marks.iter().filter(|m| **m == Some(Mark::Broken)).count();
    list.sort_by(|a, b| broken(b).cmp(&broken(a)).then(b.last.cmp(&a.last)));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(machine: &str, at: i64, decision: Option<&str>, mode: Option<&str>) -> CallFact {
        CallFact {
            session: "s".into(),
            client: Some(machine.into()),
            at: Some(at),
            decision: decision.map(str::to_owned),
            mode: mode.map(str::to_owned),
            ..CallFact::default()
        }
    }

    #[test]
    fn each_machine_is_checked_against_the_setup() {
        let mut mcp = call("a", 30, Some("ALLOW"), Some("enforce"));
        mcp.tool = Some("mcp__slack__post_message".into());
        let mut guard = call("b", 40, Some("DENY"), Some("enforce"));
        guard.rule = Some("safeguard::change".into());
        let facts = [
            call("a", 10, Some("ALLOW"), Some("shadow")),
            mcp,
            call("a", 20, None, None),
            guard,
            call("c", 50, Some("ALLOW"), Some("enforce")),
        ];
        let rules = Posture::default();
        let list = checks(&facts, &rules, &["github".into()], &[]);
        let a = list.iter().find(|c| c.name == "a").unwrap();
        assert_eq!(a.marks[0], Some(Mark::Kept), "its latest decision was enforced");
        assert_eq!(a.marks[1], Some(Mark::Broken), "one undecided Call, none allowed");
        assert_eq!(a.marks[2], Some(Mark::Broken), "slack is not on the list");
        assert_eq!(a.marks[3], Some(Mark::Off), "the host rule is off by default");
        let b = list.iter().find(|c| c.name == "b").unwrap();
        assert_eq!(b.marks[4], Some(Mark::Broken));
        assert!(!list.iter().find(|c| c.name == "c").unwrap().failing());
        assert_eq!(list.last().unwrap().name, "c", "the machine that kept everything last");
        // An empty allowlist checks nothing.
        assert_eq!(checks(&facts, &rules, &[], &[])[0].marks[2], Some(Mark::Off));
        // One undecided Call allowed.
        let loose = Posture { undecided: 1, ..Posture::default() };
        assert_eq!(checks(&facts, &loose, &[], &[]).iter().find(|c| c.name == "a").unwrap().marks[1], Some(Mark::Kept));
    }
}
