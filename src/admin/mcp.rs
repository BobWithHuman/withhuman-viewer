//! MCP 사용: the console's second product beside 이상 행위 탐지, read from the same Calls. Which
//! MCP servers the agents called, through which tools, how the engine decided on them, which
//! first appeared in the period, and which the policy draft's allowlist leaves out; then the web
//! hosts the Calls named. A server picked shows its tools and its latest Calls, each opening on
//! the route map.

use std::collections::BTreeMap;

use dioxus::prelude::*;

use super::main_page::{
    Axis, Card, Empty, clock, decision_key, decision_label, machine_of_fact, now_ms, period_label, scale, stamp,
    DAY_MS,
};
use super::policy::{allowlists, host_of};
use crate::metro::mcp_parts;
use crate::model::CallFact;
use crate::ui::home::big;
use crate::ui::{AppState, Screen};

/// How many of a server's latest Calls its panel lists.
const LATEST: usize = 12;

/// A server, or a web host, as the page counts it over the period.
#[derive(Clone, Debug, Default, PartialEq)]
struct Use {
    name: String,
    /// Its tools and how often each was called (servers only).
    tools: Vec<(String, usize)>,
    calls: usize,
    denied: usize,
    held: usize,
    /// Held, then let run by a person.
    approved: usize,
    /// When it was first and last called, over all time.
    first: Option<i64>,
    last: Option<i64>,
    /// First called inside the period.
    new: bool,
    /// On the policy draft's allowlist; none while the draft lists nothing.
    listed: Option<bool>,
}

impl Use {
    fn worry(&self) -> (usize, usize) {
        (self.denied + self.held + usize::from(self.listed == Some(false)), self.calls)
    }
}

/// The MCP server a Call used, if it called an MCP tool.
fn server_of(f: &CallFact) -> Option<(String, String)> {
    mcp_parts(f.tool.as_deref()?).map(|(s, op)| (s.to_owned(), op.to_owned()))
}

/// What the page counts, by a key each Call may have (its server, its host): over the period,
/// with when each key was first seen at all.
fn uses(all: &[CallFact], since: Option<i64>, key: impl Fn(&CallFact) -> Option<String>, allow: &[String], with_tools: bool) -> Vec<Use> {
    let mut by: BTreeMap<String, Use> = BTreeMap::new();
    let mut tools: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for f in all {
        let Some(k) = key(f) else { continue };
        let u = by.entry(k.clone()).or_insert_with(|| Use { name: k.clone(), ..Use::default() });
        if let Some(at) = f.at {
            u.first = Some(u.first.map_or(at, |t| t.min(at)));
            u.last = Some(u.last.map_or(at, |t| t.max(at)));
        }
        if since.is_some_and(|t| f.at.is_none_or(|at| at < t)) {
            continue;
        }
        u.calls += 1;
        match decision_key(f) {
            "DENY" => u.denied += 1,
            "REVIEW" => {
                u.held += 1;
                if f.ran {
                    u.approved += 1;
                }
            }
            _ => {}
        }
        if with_tools && let Some((_, op)) = server_of(f) {
            *tools.entry(k).or_default().entry(op).or_default() += 1;
        }
    }
    let mut list: Vec<Use> = by
        .into_values()
        .filter(|u| u.calls > 0)
        .map(|mut u| {
            u.new = since.is_none_or(|t| u.first.is_some_and(|f| f >= t)) && since.is_some();
            u.listed = (!allow.is_empty()).then(|| allow.iter().any(|a| a.eq_ignore_ascii_case(&u.name)));
            let mut t: Vec<(String, usize)> = tools.remove(&u.name).unwrap_or_default().into_iter().collect();
            t.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            u.tools = t;
            u
        })
        .collect();
    // What needs a look first: stopped, held or off the allowlist; then the most used.
    list.sort_by(|a, b| b.worry().cmp(&a.worry()).then(a.name.cmp(&b.name)));
    list
}

/// MCP Calls per day (or hour), split by how the engine decided: stopped, held, let through.
fn by_day(facts: &[CallFact], days: Option<i64>, now: i64) -> (Vec<String>, Vec<[usize; 3]>) {
    let axis = Axis::new(days, now);
    let labels = axis.labels();
    let mut bars = vec![[0usize; 3]; labels.len()];
    for f in facts.iter().filter(|f| server_of(f).is_some()) {
        let Some(i) = f.at.and_then(|at| axis.index(at)) else { continue };
        let k = match decision_key(f) {
            "DENY" => 0,
            "REVIEW" => 1,
            _ => 2,
        };
        bars[i][k] += 1;
    }
    (labels, bars)
}

#[component]
pub fn Mcp() -> Element {
    let state = use_context::<AppState>();
    let found = use_context::<super::Facts>().0;
    let refreshed = use_context::<super::Facts>().1;
    let mut days = use_signal(|| Some(7_i64));
    let mut picked = use_signal(|| None::<String>);
    let now = now_ms();
    let p = (state.public)();
    let (all, failed): (Vec<CallFact>, Option<String>) = match &*found.read() {
        Some(Ok(list)) => (list.clone(), None),
        Some(Err(e)) => (Vec::new(), Some(e.clone())),
        None => (Vec::new(), None),
    };
    let loading = found.read().is_none();
    let since = days().map(|d| now - d * DAY_MS);
    let in_period: Vec<CallFact> = all.iter().filter(|f| since.is_none_or(|t| f.at.is_some_and(|at| at >= t))).cloned().collect();
    let (allow_servers, allow_hosts) = allowlists();
    let servers = uses(&all, since, |f| server_of(f).map(|s| s.0), &allow_servers, true);
    let hosts = uses(&all, since, |f| host_of(f.target.as_deref()?), &allow_hosts, false);
    let mcp_calls: Vec<&CallFact> = in_period.iter().filter(|f| server_of(f).is_some()).collect();
    let count = |d: &str| mcp_calls.iter().filter(|f| decision_key(f) == d).count();
    let (denied, held) = (count("DENY"), count("REVIEW"));
    let approved = mcp_calls.iter().filter(|f| decision_key(f) == "REVIEW" && f.ran).count();
    let fresh: Vec<Use> = servers.iter().filter(|u| u.new).cloned().collect();
    let off_list = servers.iter().filter(|u| u.listed == Some(false)).count();
    let period = period_label(days());
    let at = clock(refreshed());
    let none = if loading { "불러오는 중…" } else { "데이터 없음" };
    let source = p.store();
    // The server looked at: the one picked while it is still in view, else the first.
    let current: Option<Use> = picked()
        .and_then(|n| servers.iter().find(|u| u.name == n).cloned())
        .or_else(|| servers.first().cloned());
    let latest: Vec<CallFact> = current.as_ref().map_or(Vec::new(), |c| {
        let mut v: Vec<CallFact> = in_period.iter().filter(|f| server_of(f).is_some_and(|s| s.0 == c.name)).cloned().collect();
        v.sort_by_key(|f| std::cmp::Reverse((f.at, f.seq)));
        v.truncate(LATEST);
        v
    });
    let (labels, bars) = by_day(&in_period, days(), now);
    let host_max = hosts.first().map_or(1, |h| h.calls).max(1);

    rsx! {
        div { class: "home admin mcp",
            header { class: "cs-head",
                div { class: "cs-crumb",
                    strong { "MCP 사용" }
                    span { class: "sep" }
                    span { "현황" }
                }
                span { class: "cs-src",
                    span { class: if p.demo { "dot demo" } else { "dot" } }
                    "{source}"
                }
                label { class: "admin-pick-field",
                    span { "기간" }
                    select {
                        onchange: move |e| days.set(e.value().parse().ok()),
                        for (v, l) in [("1", "최근 24시간"), ("7", "최근 7일"), ("30", "최근 30일"), ("all", "전체 기간")] {
                            option { key: "{v}", value: "{v}", selected: days().map_or("all".to_owned(), |d| d.to_string()) == v, "{l}" }
                        }
                    }
                }
            }
            if let Some(e) = failed {
                p { class: "admin-failed", "호출 기록을 불러오지 못했습니다: {e}" }
            }
            div { class: "cs-grid",
                div { class: "cs-row four",
                    Card { title: "쓰인 MCP 서버", note: period.clone(), stamp: at.clone(),
                        div { class: "cs-figure static",
                            span { class: "cs-big", if loading { "…" } else { "{servers.len()}" } }
                        }
                        div { class: "cs-sub",
                            span { "처음 보인 서버 " strong { class: "warn", "{fresh.len()}" } "개" }
                            if allow_servers.is_empty() {
                                span { "허용 목록이 없습니다 (정책에서 정하기)" }
                            } else {
                                span { "허용 목록에 없는 서버 " strong { class: "bad", "{off_list}" } "개" }
                            }
                        }
                    }
                    Card { title: "MCP 호출", note: period.clone(), stamp: at.clone(),
                        div { class: "cs-figure static",
                            span { class: "cs-big", if loading { "…" } else { "{big(mcp_calls.len() as i64)}" } }
                        }
                        div { class: "cs-sub",
                            span { "차단 " strong { class: "bad", "{denied}" } "건" }
                            span { "확인 요청 " strong { class: "warn", "{held}" } "건, 그중 사용자 승인 " strong { class: "good", "{approved}" } "건" }
                        }
                    }
                    Card { title: "처음 보인 서버", note: "기간 안에 처음 호출된 MCP 서버", stamp: at.clone(),
                        if fresh.is_empty() {
                            Empty { text: none }
                        } else {
                            div { class: "cs-list",
                                for u in fresh {
                                    {
                                        let name = u.name.clone();
                                        rsx! {
                                            button {
                                                key: "{u.name}",
                                                class: "cs-list-row pick fresh",
                                                title: "이 서버 보기",
                                                onclick: move |_| picked.set(Some(name.clone())),
                                                code { "{u.name}" }
                                                span { class: "meta", "{stamp(u.first)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Card { title: "웹 도메인", note: "WebFetch·명령에 나온 주소", stamp: at.clone(),
                        if hosts.is_empty() {
                            Empty { text: none }
                        } else {
                            div { class: "cs-list",
                                for u in hosts.iter().cloned() {
                                    {
                                        let width = u.calls as f64 / host_max as f64 * 100.0;
                                        rsx! {
                                            div { key: "{u.name}", class: "cs-list-row host", title: "{u.name}",
                                                span { class: "name",
                                                    if u.new { span { class: "badge new", "새로" } }
                                                    if u.listed == Some(false) { span { class: "badge off", "목록 밖" } }
                                                    "{u.name}"
                                                }
                                                span { class: "track", span { class: "fill", style: "width:{width:.1}%" } }
                                                span { class: "n", "{u.calls}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "cs-row split mcp-split",
                    Card { title: "서버별 사용", note: "막히거나 물은 것, 허용 목록 밖인 것이 위에", stamp: at.clone(),
                        div { class: "mcp-table",
                            div { class: "mcp-tr head",
                                span { "서버" }
                                span { "도구" }
                                span { "호출" }
                                span { "차단" }
                                span { "확인 요청" }
                                span { "처음 본 때" }
                                span { "마지막" }
                                span { "허용 목록" }
                            }
                            if servers.is_empty() {
                                Empty { text: none }
                            }
                            div { class: "mcp-tbody",
                                for u in servers.iter().cloned() {
                                    {
                                        let on = current.as_ref().is_some_and(|c| c.name == u.name);
                                        let name = u.name.clone();
                                        let listed = match u.listed {
                                            Some(true) => ("ok", "있음"),
                                            Some(false) => ("bad", "없음"),
                                            None => ("", "–"),
                                        };
                                        rsx! {
                                            button {
                                                key: "{u.name}",
                                                class: if on { "mcp-tr on" } else { "mcp-tr" },
                                                onclick: move |_| picked.set(Some(name.clone())),
                                                span { class: "name",
                                                    if u.new { span { class: "badge new", "새로" } }
                                                    code { "{u.name}" }
                                                }
                                                span { class: "num", "{u.tools.len()}" }
                                                span { class: "num", "{u.calls}" }
                                                span { class: if u.denied > 0 { "num bad" } else { "num" }, "{u.denied}" }
                                                span { class: if u.held > 0 { "num warn" } else { "num" }, "{u.held}" }
                                                span { class: "when", "{stamp(u.first)}" }
                                                span { class: "when", "{stamp(u.last)}" }
                                                span { class: "listed {listed.0}", "{listed.1}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Card { title: "도구와 최근 호출", note: current.as_ref().map_or("서버를 고르세요".to_owned(), |c| c.name.clone()), stamp: at.clone(),
                        if let Some(c) = current.clone() {
                            div { class: "mcp-tools",
                                for (t, n) in c.tools.iter().cloned() {
                                    span { key: "{t}", class: "mcp-tool", code { "{t}" } span { class: "n", "{n}" } }
                                }
                            }
                            div { class: "cs-list mcp-latest",
                                for f in latest {
                                    Latest { key: "{f.session}-{f.seq}", f }
                                }
                            }
                        } else {
                            Empty { text: none }
                        }
                    }
                }
                div { class: "cs-row",
                    Card { title: "날짜별 MCP 호출", note: "{period}", stamp: at.clone(),
                        DayBars { labels, bars }
                        div { class: "cs-legend",
                            span { class: "it", i { style: "background:#ff6b6b" } "차단" }
                            span { class: "it", i { style: "background:#f2c26b" } "확인 요청" }
                            span { class: "it", i { style: "background:#6cc6ec" } "허용·판정 없음" }
                        }
                    }
                }
            }
        }
    }
}

/// One of a server's latest Calls: when, which tool, how it was decided, on which machine; it
/// opens on the route map.
#[component]
fn Latest(f: CallFact) -> Element {
    let mut state = use_context::<AppState>();
    let op = server_of(&f).map(|s| s.1).unwrap_or_default();
    let d = decision_key(&f);
    let verdict = match (d, f.ran) {
        ("REVIEW", true) => "사용자 승인".to_owned(),
        ("REVIEW", false) => "확인 요청, 실행 안 됨".to_owned(),
        _ => decision_label(d).to_owned(),
    };
    let (session, seq) = (f.session.clone(), f.seq);
    rsx! {
        button {
            class: "cs-list-row pick mcp-call",
            title: "노선도에서 보기",
            onclick: move |_| {
                state.open_call(session.clone(), seq);
                state.screen.set(Screen::Work);
            },
            span { class: "meta", "{clock(f.at)}" }
            code { "{op}" }
            span { class: "verdict {d}", "{verdict}" }
            span { class: "meta", "{machine_of_fact(&f)}" }
        }
    }
}

/// MCP Calls over the period as stacked bars: stopped, held, let through.
#[component]
fn DayBars(labels: Vec<String>, bars: Vec<[usize; 3]>) -> Element {
    let (w, h, left, right, top, bottom) = (1200.0, 220.0, 30.0, 10.0, 10.0, 26.0);
    let plot_w = w - left - right;
    let plot_h = h - top - bottom;
    let n = labels.len().max(1);
    let (top_v, step) = scale(bars.iter().map(|b| b.iter().sum::<usize>()).max().unwrap_or(0));
    let y = move |v: usize| top + plot_h - v as f64 / top_v as f64 * plot_h;
    let slot = plot_w / n as f64;
    let bar = (slot * 0.6).max(2.0);
    let every = n.div_ceil(10);
    let ticks: Vec<usize> = (0..=top_v).step_by(step).collect();
    let colors = ["#ff6b6b", "#f2c26b", "#6cc6ec"];
    rsx! {
        svg { class: "cs-chart wide", view_box: "0 0 {w} {h}",
            for t in ticks {
                g { key: "tick-{t}",
                    line { x1: "{left}", x2: "{w - right}", y1: "{y(t):.1}", y2: "{y(t):.1}", class: "grid" }
                    text { x: "{left - 6.0}", y: "{y(t) + 3.0:.1}", text_anchor: "end", class: "axis", "{t}" }
                }
            }
            for (i, b) in bars.iter().enumerate() {
                {
                    let x = left + slot * i as f64 + (slot - bar) / 2.0;
                    let mut under = 0;
                    let parts: Vec<(f64, f64, &str)> = b
                        .iter()
                        .zip(colors)
                        .filter(|(v, _)| **v > 0)
                        .map(|(v, c)| {
                            let p = (y(under + v), y(under) - y(under + v), c);
                            under += v;
                            p
                        })
                        .collect();
                    rsx! {
                        g { key: "bar-{i}",
                            title { "{labels[i]}: 차단 {b[0]}, 확인 요청 {b[1]}, 허용 {b[2]}" }
                            for (j, (py, ph, c)) in parts.into_iter().enumerate() {
                                rect { key: "{j}", x: "{x:.1}", y: "{py:.1}", width: "{bar:.1}", height: "{ph:.1}", fill: "{c}" }
                            }
                            if i % every == 0 || i + 1 == n {
                                text { x: "{x + bar / 2.0:.1}", y: "{h - 8.0}", text_anchor: "middle", class: "axis", "{labels[i]}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tool: &str, at: i64, decision: Option<&str>) -> CallFact {
        CallFact {
            session: "s".into(),
            tool: Some(tool.into()),
            at: Some(at),
            decision: decision.map(str::to_owned),
            ..CallFact::default()
        }
    }

    #[test]
    fn servers_are_counted_over_the_period_and_new_ones_marked() {
        let all = [
            call("mcp__github__create_issue", 5, Some("ALLOW")),
            call("mcp__github__create_issue", 50, Some("DENY")),
            call("mcp__github__list_repos", 60, Some("ALLOW")),
            call("mcp__slack__post_message", 70, Some("REVIEW")),
            call("Bash", 80, Some("DENY")),
        ];
        let list = uses(&all, Some(40), |f| server_of(f).map(|s| s.0), &["github".into()], true);
        let names: Vec<&str> = list.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["slack", "github"], "held and off the list beats one stop");
        let gh = &list[1];
        assert_eq!((gh.calls, gh.denied, gh.new, gh.listed), (2, 1, false, Some(true)), "github was first seen before the period");
        assert_eq!(gh.tools[0].1 + gh.tools[1].1, 2);
        let slack = &list[0];
        assert_eq!((slack.held, slack.new, slack.listed), (1, true, Some(false)));
        // No allowlist yet: nothing is marked on or off it.
        assert!(uses(&all, None, |f| server_of(f).map(|s| s.0), &[], true).iter().all(|u| u.listed.is_none() && !u.new));
    }

    #[test]
    fn days_split_mcp_calls_by_decision() {
        let now = 10 * DAY_MS + 1;
        let (labels, bars) = by_day(
            &[call("mcp__a__x", now, Some("DENY")), call("mcp__a__x", now, None), call("Read", now, Some("DENY"))],
            Some(7),
            now,
        );
        assert_eq!(labels.len(), 7);
        assert_eq!(bars[6], [1, 0, 1], "only MCP Calls count");
    }
}
