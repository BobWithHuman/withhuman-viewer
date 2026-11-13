//! 메인: the security team's first page, a hub. Its middle is a model of who is connected to
//! what: people, their machines, the agents on them, and the places outside those agents reached
//! (hosts values were sent to, MCP servers), stood as blocks on a floor and joined by lines that
//! run the way the Calls went. It turns when dragged; a block under the pointer lights what it
//! is joined to, and a block picked is told in the card beside. Beneath, four cards say how
//! each of the other pages stands and open it: 이상 행위 탐지, MCP 사용, 민감 정보, 인벤토리.
//!
//! This file works out the model and where its blocks stand; three.js draws it
//! (`web/hub-scene/scene.js`, bundled into `hub_scene.js` and loaded with the page).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use chrono::{Local, TimeZone, Timelike};
use dioxus::prelude::*;

use super::main_page::{Axis, DAY_MS, decision_key, machine_of_fact, now_ms, period_label};
use super::secrets::{destination, held, in_call, outward};
use crate::api::Backend;
use crate::model::{CallFact, Inventory, SecretMention};
use crate::ui::{AppState, Screen};

/// Where a block stands, from the people on the left to the world outside on the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Person,
    Device,
    Agent,
    Outside,
}

impl Tier {
    const ALL: [Tier; 4] = [Tier::Person, Tier::Device, Tier::Agent, Tier::Outside];

    fn name(self) -> &'static str {
        match self {
            Tier::Person => "사용자",
            Tier::Device => "기기",
            Tier::Agent => "에이전트",
            Tier::Outside => "바깥",
        }
    }
}

/// How a line went: only Calls, a send that was stopped, or a value that got out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tone {
    Plain,
    Held,
    Left,
}

/// A block of the model: a person, a machine, an agent on one, or a place outside.
#[derive(Clone, Debug, PartialEq)]
struct Node {
    tier: Tier,
    label: String,
    sub: String,
    /// The machines it stands for: a person's, its own, or those that reached a place.
    clients: Vec<String>,
    calls: usize,
    /// Calls the engine stopped or held.
    stopped: usize,
    /// Sends of a sensitive value: kept back, and let through.
    held: usize,
    left: usize,
    /// An agent running without the hooks.
    shadow: bool,
}

impl Node {
    fn tone(&self) -> Tone {
        if self.left > 0 {
            Tone::Left
        } else if self.held > 0 || self.stopped > 0 {
            Tone::Held
        } else {
            Tone::Plain
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Link {
    from: usize,
    to: usize,
    tone: Tone,
}

/// The model: its blocks, tier by tier, and the lines between them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Web {
    nodes: Vec<Node>,
    links: Vec<Link>,
}

/// An agent's reach to a place outside: the agent's block, its machine, and how it went.
type Reach = (usize, String, Tone);

/// How many machines and places the model stands up.
const MOST_DEVICES: usize = 7;
const MOST_PLACES: usize = 6;

fn agent_name(agent: Option<&str>) -> String {
    match agent {
        Some("claude-code") => "Claude Code".into(),
        Some("codex") => "Codex".into(),
        Some(other) if !other.is_empty() => other.to_owned(),
        _ => "에이전트".into(),
    }
}

/// The MCP server a tool belongs to, as `mcp__server__tool` names it.
fn mcp_server(tool: &str) -> Option<&str> {
    tool.strip_prefix("mcp__")?.split("__").next()
}

/// Whether the engine stopped a Call or held it for a person.
fn stopped(f: &CallFact) -> bool {
    matches!(decision_key(f), "DENY" | "REVIEW")
}

/// The model over the Calls and the sensitive values given (the caller cuts both to the period):
/// the machines with the most to look at, their people, an agent on each (and any found without
/// hooks), and the places outside those agents sent values to or called through MCP.
fn web(inv: Option<&Inventory>, facts: &[CallFact], secrets: &[SecretMention]) -> Web {
    #[derive(Default)]
    struct Seen {
        calls: usize,
        stopped: usize,
        held: usize,
        left: usize,
    }
    let mut seen: BTreeMap<String, Seen> = BTreeMap::new();
    for f in facts {
        let s = seen.entry(machine_of_fact(f)).or_default();
        s.calls += 1;
        s.stopped += usize::from(stopped(f));
    }
    // The places outside, each with the machines that reached it and how.
    let mut places: BTreeMap<String, BTreeMap<String, Tone>> = BTreeMap::new();
    let mut reach = |place: String, client: String, tone: Tone| {
        let t = places
            .entry(place)
            .or_default()
            .entry(client)
            .or_insert(tone);
        *t = (*t).max(tone);
    };
    for m in secrets.iter().filter(|m| in_call(m) && outward(m)) {
        let client = m.client.clone().unwrap_or_default();
        let s = seen.entry(client.clone()).or_default();
        let tone = if held(m) {
            s.held += 1;
            Tone::Held
        } else {
            s.left += 1;
            Tone::Left
        };
        reach(destination(m), client, tone);
    }
    for f in facts {
        if let Some(server) = f.tool.as_deref().and_then(mcp_server) {
            let tone = if stopped(f) && !f.ran {
                Tone::Held
            } else {
                Tone::Plain
            };
            reach(format!("{server} (MCP)"), machine_of_fact(f), tone);
        }
    }

    // The machines: the registered ones and any other heard from, the most to look at first.
    let known: HashMap<&str, &crate::model::Device> = inv
        .map(|i| {
            i.clients
                .iter()
                .map(|d| (d.client_id.as_str(), d))
                .collect()
        })
        .unwrap_or_default();
    let mut ids: BTreeSet<String> = seen.keys().filter(|k| !k.is_empty()).cloned().collect();
    ids.extend(
        known
            .values()
            .filter(|d| d.registered_at.is_some())
            .map(|d| d.client_id.clone()),
    );
    let mut devices: Vec<String> = ids.into_iter().collect();
    let weight = |id: &String| {
        let s = seen.get(id);
        (
            s.map_or(0, |s| s.left),
            s.map_or(0, |s| s.held + s.stopped),
            s.map_or(0, |s| s.calls),
        )
    };
    devices.sort_by(|a, b| weight(b).cmp(&weight(a)).then(a.cmp(b)));
    devices.truncate(MOST_DEVICES);
    let owner = |id: &str| known.get(id).and_then(|d| d.user_email.clone());
    // Machines side by side under their person, so the lines to them do not cross.
    devices.sort_by_key(|id| {
        (
            owner(id).is_none(),
            owner(id),
            std::cmp::Reverse(weight(id)),
        )
    });

    let mut w = Web::default();
    let stats = |n: &mut Node, id: &str| {
        if let Some(s) = seen.get(id) {
            n.calls += s.calls;
            n.stopped += s.stopped;
            n.held += s.held;
            n.left += s.left;
        }
    };
    let blank = |tier: Tier, label: String, sub: String| Node {
        tier,
        label,
        sub,
        clients: Vec::new(),
        calls: 0,
        stopped: 0,
        held: 0,
        left: 0,
        shadow: false,
    };

    // People, in the order their machines stand.
    let mut person_at: BTreeMap<String, usize> = BTreeMap::new();
    for id in &devices {
        if let Some(email) = owner(id)
            && !person_at.contains_key(&email)
        {
            person_at.insert(email.clone(), w.nodes.len());
            let name = email.split('@').next().unwrap_or(&email).to_owned();
            w.nodes.push(blank(Tier::Person, name, email));
        }
    }
    // Machines, each with its agent, and any agent found on it without the hooks.
    let mut agent_at: HashMap<String, usize> = HashMap::new();
    let mut device_nodes = Vec::new();
    let mut agent_nodes = Vec::new();
    for id in &devices {
        let d = known.get(id.as_str());
        let host = d
            .and_then(|d| d.hostname.clone())
            .unwrap_or_else(|| id.clone());
        let mut device = blank(Tier::Device, host.clone(), id.clone());
        device.clients = vec![id.clone()];
        stats(&mut device, id);
        let mut agent = blank(
            Tier::Agent,
            agent_name(d.and_then(|d| d.agent.as_deref())),
            host.clone(),
        );
        agent.clients = vec![id.clone()];
        stats(&mut agent, id);
        device_nodes.push((id.clone(), device));
        agent_nodes.push((id.clone(), agent, false));
        for a in inv
            .and_then(|i| i.shadow.as_ref())
            .into_iter()
            .flatten()
            .filter(|a| a.client_id.as_deref() == Some(id.as_str()))
        {
            let mut n = blank(
                Tier::Agent,
                agent_name(Some(&a.agent)),
                a.project.clone().unwrap_or_else(|| host.clone()),
            );
            n.clients = vec![id.clone()];
            n.shadow = true;
            agent_nodes.push((id.clone(), n, true));
        }
    }
    let mut device_at: HashMap<String, usize> = HashMap::new();
    for (id, n) in device_nodes {
        device_at.insert(id, w.nodes.len());
        w.nodes.push(n);
    }
    for (id, n, shadow) in agent_nodes {
        let at = w.nodes.len();
        if !shadow {
            agent_at.insert(id.clone(), at);
        }
        w.links.push(Link {
            from: device_at[&id],
            to: at,
            tone: if shadow { Tone::Plain } else { n.tone() },
        });
        w.nodes.push(n);
    }
    for (id, &at) in &device_at {
        if let Some(&p) = owner(id).and_then(|e| person_at.get(&e)) {
            let tone = w.nodes[at].tone();
            w.links.push(Link {
                from: p,
                to: at,
                tone,
            });
            let d = w.nodes[at].clone();
            let person = &mut w.nodes[p];
            person.clients.push(id.clone());
            person.calls += d.calls;
            person.stopped += d.stopped;
            person.held += d.held;
            person.left += d.left;
        }
    }
    // Places outside that a standing machine reached, the worst first.
    let mut outside: Vec<(String, Vec<Reach>)> = places
        .into_iter()
        .map(|(place, by)| {
            let from = by
                .into_iter()
                .filter_map(|(client, tone)| Some((*agent_at.get(&client)?, client, tone)))
                .collect::<Vec<_>>();
            (place, from)
        })
        .filter(|(_, from)| !from.is_empty())
        .collect();
    outside.sort_by_key(|(place, from)| {
        (
            std::cmp::Reverse(from.iter().map(|f| f.2).max()),
            std::cmp::Reverse(from.len()),
            place.clone(),
        )
    });
    outside.truncate(MOST_PLACES);
    for (place, from) in outside {
        let at = w.nodes.len();
        let mut n = blank(
            Tier::Outside,
            place.clone(),
            if place.ends_with("(MCP)") {
                "MCP 서버".into()
            } else {
                "외부 호스트".into()
            },
        );
        for (agent, client, tone) in from {
            n.clients.push(client);
            match tone {
                Tone::Left => n.left += 1,
                Tone::Held => n.held += 1,
                Tone::Plain => n.calls += 1,
            }
            w.links.push(Link {
                from: agent,
                to: at,
                tone,
            });
        }
        w.nodes.push(n);
    }
    w
}

/// Where each tier stands along the floor, as the scene's lanes are laid.
const TIER_X: [f64; 4] = [-3.3, -1.1, 1.1, 3.3];

/// Where each block stands on the floor: its tier's line, spread along the depth.
fn stand(w: &Web) -> Vec<(f64, f64)> {
    let mut at = vec![(0.0, 0.0); w.nodes.len()];
    for (t, tier) in Tier::ALL.into_iter().enumerate() {
        let of: Vec<usize> = (0..w.nodes.len())
            .filter(|i| w.nodes[*i].tier == tier)
            .collect();
        let step = (6.8 / of.len().max(1) as f64).min(1.3);
        for (k, i) in of.iter().enumerate() {
            at[*i] = (TIER_X[t], (k as f64 - (of.len() as f64 - 1.0) / 2.0) * step);
        }
    }
    at
}

fn tone_name(t: Tone) -> &'static str {
    match t {
        Tone::Left => "left",
        Tone::Held => "held",
        Tone::Plain => "plain",
    }
}

/// The model as the scene takes it: the tiers' names, each block with where it stands and what
/// to say of it under the pointer, and the lines.
fn scene_json(w: &Web) -> String {
    let at = stand(w);
    let nodes: Vec<serde_json::Value> = w
        .nodes
        .iter()
        .zip(&at)
        .map(|(n, (x, z))| {
            let tip = if n.shadow {
                "훅 없이 도는 에이전트".to_owned()
            } else if n.tier == Tier::Outside {
                format!("값을 보낸 기기 {}대, 막힌 기기 {}대", n.left, n.held)
            } else {
                format!(
                    "호출 {}건, 막거나 확인 {}건, 유출 {}건",
                    n.calls, n.stopped, n.left
                )
            };
            serde_json::json!({
                "tier": Tier::ALL.iter().position(|t| *t == n.tier),
                "label": n.label,
                "tone": tone_name(n.tone()),
                "shadow": n.shadow,
                "x": x,
                "z": z,
                "tip": tip,
            })
        })
        .collect();
    let links: Vec<serde_json::Value> = w
        .links
        .iter()
        .map(|l| serde_json::json!({"from": l.from, "to": l.to, "tone": tone_name(l.tone)}))
        .collect();
    serde_json::json!({
        "tiers": Tier::ALL.map(Tier::name),
        "nodes": nodes,
        "links": links,
    })
    .to_string()
}

/// What the model lights when nothing is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Layer {
    All,
    Stopped,
    Leaks,
    Shadow,
}

impl Layer {
    fn shows(self, n: &Node) -> bool {
        match self {
            Layer::All => true,
            Layer::Stopped => n.stopped > 0,
            Layer::Leaks => n.left + n.held > 0,
            Layer::Shadow => n.shadow,
        }
    }
}

#[component]
pub fn Hub() -> Element {
    let mut state = use_context::<AppState>();
    let backend = use_context::<Arc<Backend>>();
    let super::Facts(found, _) = use_context::<super::Facts>();
    let extra = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            async move {
                (
                    b.inventory().await.ok().flatten(),
                    b.secrets().await.unwrap_or_default(),
                )
            }
        }
    });
    let mut days = use_signal(|| Some(7_i64));
    let mut layer = use_signal(|| Layer::All);
    let mut picked = use_signal(|| None::<usize>);
    // The scene says which block was picked.
    use_future(move || async move {
        let mut said = document::eval(
            "window.__hubPick = (i) => dioxus.send(i == null ? -1 : i); await new Promise(() => {});",
        );
        while let Ok(i) = said.recv::<i64>().await {
            picked.set(usize::try_from(i).ok());
        }
    });

    let now = now_ms();
    let since = days().map(|d| now - d * DAY_MS);
    let in_period = |at: Option<i64>| since.is_none_or(|s| at.is_some_and(|a| a >= s));
    let facts: Vec<CallFact> = match &*found.read() {
        Some(Ok(list)) => list.iter().filter(|f| in_period(f.at)).cloned().collect(),
        _ => Vec::new(),
    };
    let (inv, all_secrets): (Option<Inventory>, Vec<SecretMention>) =
        extra.read().clone().unwrap_or_default();
    let secrets: Vec<SecretMention> = all_secrets
        .into_iter()
        .filter(|m| in_period(m.at))
        .collect();
    let loading = found.read().is_none() || extra.read().is_none();
    let w = web(inv.as_ref(), &facts, &secrets);

    // The scene is given the model whenever it changes, then what to light and what is picked.
    let model = serde_json::to_string(&scene_json(&w)).unwrap_or_else(|_| "\"{}\"".into());
    let lit = match layer() {
        Layer::All => "null".to_owned(),
        l => serde_json::json!(
            (0..w.nodes.len())
                .filter(|i| l.shows(&w.nodes[*i]))
                .collect::<Vec<_>>()
        )
        .to_string(),
    };
    let chosen = picked()
        .filter(|i| *i < w.nodes.len())
        .map_or("null".to_owned(), |i| i.to_string());
    use_effect(use_reactive!(|(model, lit, chosen)| {
        document::eval(&format!(
            r#"const el = document.getElementById("hub-scene");
            if (el && window.WHScene) {{
                if (!window.__hub || window.__hub.container !== el) {{
                    window.__hub?.dispose();
                    window.__hub = window.WHScene.mount(el);
                    window.__hubModel = null;
                }}
                const h = window.__hub;
                h.onPick = (i) => window.__hubPick?.(i);
                const model = {model};
                if (window.__hubModel !== model) {{
                    window.__hubModel = model;
                    h.setData(JSON.parse(model));
                }}
                h.setLit({lit});
                h.setPicked({chosen});
            }}"#
        ));
    }));

    // The block the card beside tells: the one picked, else the one with the most to look at.
    let told = picked().filter(|i| *i < w.nodes.len()).or_else(|| {
        (0..w.nodes.len())
            .filter(|i| w.nodes[*i].tier != Tier::Outside)
            .max_by_key(|i| {
                let n = &w.nodes[*i];
                (n.left, n.held + n.stopped, n.tier == Tier::Person, n.calls)
            })
    });

    // The figures for the four cards.
    let count = |d: &str| facts.iter().filter(|f| decision_key(f) == d).count();
    let (denied, reviewed) = (count("DENY"), count("REVIEW"));
    let risky = facts
        .iter()
        .filter(|f| decision_key(f) == "ALLOW" && f.irreversible)
        .count();
    let axis = Axis::new(days(), now);
    let mut per_day = vec![[0usize; 3]; axis.labels().len()];
    for f in &facts {
        if let Some(i) = f.at.and_then(|a| axis.index(a)) {
            match decision_key(f) {
                "DENY" => per_day[i][0] += 1,
                "REVIEW" => per_day[i][1] += 1,
                "ALLOW" if f.irreversible => per_day[i][2] += 1,
                _ => {}
            }
        }
    }
    let mcp: Vec<&CallFact> = facts
        .iter()
        .filter(|f| f.tool.as_deref().and_then(mcp_server).is_some())
        .collect();
    let mcp_stopped = mcp.iter().filter(|f| stopped(f) && !f.ran).count();
    let mcp_ran = mcp.len() - mcp_stopped;
    let values: BTreeSet<(&str, &str)> = secrets
        .iter()
        .map(|m| (m.session.as_str(), m.key.as_str()))
        .collect();
    let leaked: BTreeSet<(&str, &str)> = secrets
        .iter()
        .filter(|m| in_call(m) && outward(m) && !held(m))
        .map(|m| (m.session.as_str(), m.key.as_str()))
        .collect();
    let fleet = inv.as_ref().map(|i| {
        let registered: Vec<&str> = i
            .clients
            .iter()
            .filter(|d| d.registered_at.is_some())
            .map(|d| d.client_id.as_str())
            .collect();
        let running = registered
            .iter()
            .filter(|id| {
                i.sessions.iter().any(|r| {
                    r.client_id.as_deref() == Some(**id)
                        && r.ended_at.is_none()
                        && r.last_seen_at.is_some_and(|t| t >= now - 10 * 60_000)
                })
            })
            .count();
        (
            running,
            registered.len(),
            i.shadow.as_ref().map_or(0, Vec::len),
        )
    });
    let period = period_label(days());

    rsx! {
        div { class: "hub",
            section { class: "hub-stage",
                div { id: "hub-scene" }
                if w.nodes.is_empty() {
                    p { class: "hub-none", if loading { "불러오는 중…" } else { "이 기간에는 에이전트의 활동이 없습니다. 기간을 넓혀 보세요." } }
                }
                div { class: "hub-lead",
                    h1 { "에이전트" br {} "연결 지도" }
                    p { "누가 어떤 기기와 에이전트로 어디까지 닿았는지 보여 줍니다. 끌어서 돌리고, 블록을 눌러 따라가 보세요." }
                    div { class: "hub-tools",
                        for (l, name, d) in [
                            (Layer::All, "전체 보기", "M4 6.5h16M4 12h16M4 17.5h16"),
                            (Layer::Stopped, "막거나 확인한 호출이 있는 곳", "M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z"),
                            (Layer::Leaks, "민감 값을 밖으로 보내려 한 곳", "M12 4.5l8.5 14.5h-17zM12 10.5v4M12 17.2v.3"),
                            (Layer::Shadow, "훅 없이 도는 에이전트", "M9.2 9.2a2.9 2.9 0 1 1 4.3 2.6c-1 .6-1.5 1.2-1.5 2.4M12 17.4v.3M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17z"),
                        ] {
                            button { key: "{name}", class: if layer() == l { "on" } else { "" }, title: "{name}", onclick: move |_| layer.set(l),
                                svg { width: "17", height: "17", view_box: "0 0 24 24",
                                    path { d, fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round" }
                                }
                            }
                        }
                    }
                    div { class: "hub-period",
                        for (v, l) in [(Some(1), "24시간"), (Some(7), "7일"), (Some(30), "30일"), (None, "전체")] {
                            button { key: "{l}", class: if days() == v { "on" } else { "" }, onclick: move |_| { days.set(v); picked.set(None); }, "{l}" }
                        }
                    }
                }
                if let Some(i) = told {
                    {
                        let n = w.nodes[i].clone();
                        let mut hours = [(0usize, 0usize); 24];
                        for f in facts.iter().filter(|f| n.clients.contains(&machine_of_fact(f))) {
                            if let Some(t) = f.at.and_then(|a| Local.timestamp_millis_opt(a).single()) {
                                let h = &mut hours[t.hour() as usize];
                                h.0 += 1;
                                h.1 += usize::from(stopped(f));
                            }
                        }
                        let top = hours.iter().map(|h| h.0).max().unwrap_or(0).max(1);
                        rsx! {
                            aside { class: "hub-card hub-told",
                                header {
                                    h3 { if picked().is_some() { "고른 {n.tier.name()}" } else { "가장 살펴볼 {n.tier.name()}" } }
                                    button { class: "go", title: "인벤토리에서 보기", onclick: move |_| state.screen.set(Screen::Inventory), {arrow()} }
                                }
                                p { class: "who", strong { "{n.label}" } span { "{n.sub}" } }
                                div { class: "nums",
                                    div { b { "{n.calls}" } small { "호출" } }
                                    div { b { class: if n.stopped > 0 { "warn" } else { "" }, "{n.stopped}" } small { "막거나 확인" } }
                                    div { b { class: if n.left > 0 { "bad" } else { "" }, "{n.left}" } small { "밖으로 나감" } }
                                }
                                div { class: "ticks", title: "시간대별 호출, 주황은 막거나 확인한 호출이 있던 시간",
                                    for (h, (calls, stops)) in hours.iter().enumerate() {
                                        i { key: "{h}", class: if *stops > 0 { "hot" } else if *calls > 0 { "on" } else { "" },
                                            style: "bottom:{*calls as f64 / top as f64 * 86.0:.0}%" }
                                    }
                                }
                                div { class: "axis", span { "0시" } span { "6시" } span { "12시" } span { "18시" } span { "23시" } }
                            }
                        }
                    }
                }
            }
            div { class: "hub-cards",
                button { class: "hub-card", onclick: move |_| state.screen.set(Screen::Detect),
                    header { h3 { "이상 행위 탐지" } span { class: "go", {arrow()} } }
                    div { class: "trio",
                        div { b { "{denied}" } small { "차단" } }
                        div { b { "{reviewed}" } small { "확인 요청" } }
                        div { b { "{risky}" } small { "되돌리기 어려운 허용" } }
                    }
                    div { class: "combs",
                        for (k, tone) in ["ink", "hot", "sun"].into_iter().enumerate() {
                            {
                                let top = per_day.iter().map(|d| d[k]).max().unwrap_or(0).max(1);
                                rsx! {
                                    div { key: "{tone}", class: "comb {tone}",
                                        for (j, d) in per_day.iter().enumerate() {
                                            i { key: "{j}", style: "height:{(d[k] as f64 / top as f64 * 100.0).max(3.0):.0}%" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                button { class: "hub-card", onclick: move |_| state.screen.set(Screen::Mcp),
                    header { h3 { "MCP 사용" } span { class: "go", {arrow()} } }
                    div { class: "keys", span { i { class: "hot" } "실행됨" } span { i { class: "sun" } "막힘" } }
                    {arc(mcp_ran, mcp_stopped)}
                    div { class: "ends", span { b { "{mcp_ran}" } small { "건 실행" } } span { b { "{mcp_stopped}" } small { "건 막힘" } } }
                }
                div { class: "hub-pair",
                    button { class: "hub-card", onclick: move |_| state.screen.set(Screen::Secrets),
                        header { h3 { "민감 정보" } span { class: "go", {arrow()} } }
                        div { class: "line", i { class: "mark" } span { b { "{values.len()}" } small { "개 드러남" } } em { "{period}" } }
                    }
                    button { class: "hub-card", onclick: move |_| state.screen.set(Screen::Secrets),
                        header { h3 { "밖으로 나간 값" } span { class: "go", {arrow()} } }
                        div { class: "line", i { class: if leaked.is_empty() { "mark" } else { "mark hot" } } span { b { "{leaked.len()}" } small { "개" } } em { "{period}" } }
                    }
                }
                button { class: "hub-card", onclick: move |_| state.screen.set(Screen::Inventory),
                    header { h3 { "인벤토리" } span { class: "go", {arrow()} } }
                    if let Some((running, registered, shadow)) = fleet {
                        div { class: "trio", div { b { "{running}" small { " / {registered}" } } small { "지금 도는 기기" } } div { b { class: if shadow > 0 { "warn" } else { "" }, "{shadow}" } small { "섀도 AI" } } }
                        {gauge(running, registered)}
                    } else {
                        p { class: "none", "이 엔진은 아직 인벤토리를 모으지 않습니다." }
                    }
                }
            }
        }
    }
}

fn arrow() -> Element {
    rsx! {
        svg { width: "14", height: "14", view_box: "0 0 24 24",
            path { d: "M7 17L17 7M9 7h8v8", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

/// A point on a half circle over (cx, cy), `t` of the way from its left end to its right.
fn on_arc(cx: f64, cy: f64, r: f64, t: f64) -> (f64, f64) {
    let a = std::f64::consts::PI * (1.0 - t);
    (cx + r * a.cos(), cy - r * a.sin())
}

/// Two counts as one arc: the first from the left, the second after it.
fn arc(first: usize, second: usize) -> Element {
    let (cx, cy, r) = (150.0, 150.0, 132.0);
    let total = (first + second).max(1) as f64;
    let cut = first as f64 / total;
    let path = |from: f64, to: f64| {
        let (x0, y0) = on_arc(cx, cy, r, from);
        let (x1, y1) = on_arc(cx, cy, r, to);
        format!("M{x0:.1},{y0:.1} A{r},{r} 0 0 1 {x1:.1},{y1:.1}")
    };
    let gap = if first > 0 && second > 0 { 0.012 } else { 0.0 };
    rsx! {
        svg { class: "hub-arc", view_box: "0 0 300 122", preserve_aspect_ratio: "xMidYMin slice",
            path { class: "rail", d: "{path(0.0, 1.0)}" }
            if first > 0 { path { class: "hot", d: "{path(0.0, (cut - gap).max(0.01))}" } }
            if second > 0 { path { class: "sun", d: "{path((cut + gap).min(0.99), 1.0)}" } }
        }
    }
}

/// A share as a dial: ticks round a half circle, lit up to the share, a hand at its end.
fn gauge(part: usize, whole: usize) -> Element {
    let (cx, cy, r) = (150.0, 150.0, 128.0);
    let share = if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    };
    let ticks = 72;
    let (hx, hy) = on_arc(cx, cy, r - 30.0, share.clamp(0.02, 0.98));
    rsx! {
        svg { class: "hub-gauge", view_box: "0 0 300 122", preserve_aspect_ratio: "xMidYMin slice",
            for k in 0..=ticks {
                {
                    let t = k as f64 / ticks as f64;
                    let (x0, y0) = on_arc(cx, cy, r, t);
                    let (x1, y1) = on_arc(cx, cy, r - 11.0, t);
                    rsx! { line { key: "{k}", class: if t <= share { "on" } else { "" }, x1: "{x0:.1}", y1: "{y0:.1}", x2: "{x1:.1}", y2: "{y1:.1}" } }
                }
            }
            text { x: "150", y: "84", text_anchor: "middle", "{share * 100.0:.0}" tspan { "%" } }
            circle { class: "hand", cx: "{hx:.1}", cy: "{hy:.1}", r: "5" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Device, ShadowAgent};

    fn call(client: &str, decision: &str, tool: &str) -> CallFact {
        CallFact {
            session: "s".into(),
            client: Some(client.into()),
            decision: Some(decision.into()),
            tool: Some(tool.into()),
            ..CallFact::default()
        }
    }

    fn send(client: &str, url: &str, decision: &str, ran: bool) -> SecretMention {
        SecretMention {
            session: "s".into(),
            key: "k".into(),
            node: Some("Call".into()),
            slot: Some("agent".into()),
            client: Some(client.into()),
            tool: Some("WebFetch".into()),
            target: Some(url.into()),
            decision: Some(decision.into()),
            ran,
            ..SecretMention::default()
        }
    }

    fn inventory() -> Inventory {
        let device = |id: &str, host: &str, user: &str| Device {
            client_id: id.into(),
            hostname: Some(host.into()),
            agent: Some("claude-code".into()),
            user_email: Some(user.into()),
            registered_at: Some(1),
            ..Device::default()
        };
        Inventory {
            clients: vec![
                device("a", "mac-a", "kim@x.io"),
                device("b", "mac-b", "kim@x.io"),
                device("c", "mac-c", "lee@x.io"),
            ],
            shadow: Some(vec![ShadowAgent {
                client_id: Some("c".into()),
                agent: "codex".into(),
                ..ShadowAgent::default()
            }]),
            ..Inventory::default()
        }
    }

    #[test]
    fn the_model_joins_people_to_the_places_their_agents_reached() {
        let facts = [
            call("a", "ALLOW", "Bash"),
            call("a", "DENY", "Bash"),
            call("b", "ALLOW", "mcp__slack__post"),
        ];
        let secrets = [
            send("a", "https://paste.example/x", "ALLOW", true),
            send("b", "https://hook.example/y", "DENY", false),
        ];
        let w = web(Some(&inventory()), &facts, &secrets);
        let named = |label: &str| {
            w.nodes
                .iter()
                .position(|n| n.label == label)
                .unwrap_or_else(|| panic!("no {label}"))
        };
        let tiers: Vec<usize> = Tier::ALL
            .iter()
            .map(|t| w.nodes.iter().filter(|n| n.tier == *t).count())
            .collect();
        assert_eq!(
            tiers,
            [2, 3, 4, 3],
            "two people, three machines, an agent each and one without hooks, three places"
        );
        let kim = &w.nodes[named("kim")];
        assert_eq!(
            (kim.calls, kim.stopped, kim.left, kim.held),
            (3, 1, 1, 1),
            "a person sums their machines"
        );
        assert_eq!(w.nodes[named("paste.example")].tone(), Tone::Left);
        assert_eq!(w.nodes[named("hook.example")].tone(), Tone::Held);
        assert!(w.nodes[named("Codex")].shadow);
        // A line to a place wears how the value went; the lines before it wear the machine's worst.
        let to_paste = w
            .links
            .iter()
            .find(|l| l.to == named("paste.example"))
            .unwrap();
        assert_eq!(to_paste.tone, Tone::Left);
        assert!(
            w.links
                .iter()
                .any(|l| l.from == named("kim") && l.to == named("mac-a") && l.tone == Tone::Left)
        );
    }

    #[test]
    fn without_an_inventory_the_machines_heard_from_stand_alone() {
        let w = web(None, &[call("a", "ALLOW", "Bash")], &[]);
        assert_eq!(
            w.nodes.iter().map(|n| n.tier).collect::<Vec<_>>(),
            [Tier::Device, Tier::Agent]
        );
        assert_eq!(w.links.len(), 1);
        assert!(web(None, &[], &[]).nodes.is_empty());
    }

    #[test]
    fn blocks_stand_in_their_tiers_and_the_scene_is_told_each() {
        let w = web(Some(&inventory()), &[call("a", "DENY", "Bash")], &[]);
        for (n, (x, _)) in w.nodes.iter().zip(stand(&w)) {
            assert_eq!(
                x,
                TIER_X[Tier::ALL.iter().position(|t| *t == n.tier).unwrap()]
            );
        }
        let told: serde_json::Value = serde_json::from_str(&scene_json(&w)).unwrap();
        assert_eq!(told["tiers"][0], "사용자");
        assert_eq!(told["nodes"].as_array().unwrap().len(), w.nodes.len());
        assert_eq!(told["links"].as_array().unwrap().len(), w.links.len());
        let a = told["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == "mac-a")
            .unwrap();
        assert_eq!(
            (a["tier"].as_u64(), a["tone"].as_str()),
            (Some(1), Some("held"))
        );
    }
}
