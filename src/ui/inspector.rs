//! The selected node or cause edge, read in full: a Call's verdict and cause, a result's
//! paragraphs, and every stored property.

use std::sync::Arc;

use dioxus::prelude::*;
use serde_json::Value;

use super::{AppState, Load, Selection};
use crate::api::Backend;
use crate::layout::{Model, decision_color, label, percent};
use crate::model::{NodeDetail, parse_json};

use super::trace::{Check, Context, Deviation, JevLog, Preview, Story, TracePath};

/// A verdict as a coloured badge: 허용, 확인 or 차단.
pub(super) fn decision_badge(decision: &str) -> Element {
    rsx! {
        span { class: "badge solid", style: "background: var({decision_color(decision)})", "{verdict_text(decision)}" }
    }
}

fn rule_text(rule: &str) -> Option<&'static str> {
    Some(match rule {
        "intent::mismatch" => "의도에서 벗어난 행동으로 차단",
        "intent::review" => "의도에서 벗어났을 수 있어 사용자 확인",
        "intent::match" => "의도 안의 행동",
        "intent::unknown" => "의도가 기록되지 않아 판정 없음",
        "trace::unresolved" => "원인 추적 실패",
        "trace::no_subgraph" => "판정에 필요한 작업 흐름이 없어 원인을 추적하지 못함",
        "cause::none" => "원인 없음으로 차단 (이전 규칙)",
        "safeguard::change" => "에이전트 설정·훅 변경으로 차단",
        _ => return None,
    })
}

pub(super) fn link_text(link: &str) -> &str {
    match link {
        "request" => "사용자 요청의 다음 단계",
        "segment" => "도구 결과의 내용",
        "none" => "요청·결과와 무관",
        other => other,
    }
}

pub(super) fn breach_text(breach: &str) -> &str {
    match breach {
        "none" => "위반 없음",
        "explicit" => "사용자가 말한 목표·조건을 어김",
        "unrelated_change" => "요청과 관계없는 파일·저장소를 바꿈",
        "exfiltration" => "사용자 데이터·비밀값을 외부로 보냄",
        "unconfirmed_irreversible" => "되돌릴 수 없는 작업을 묻지 않고 함",
        "security_change" => "권한·인증·보안 설정을 바꿈",
        "out_of_scope_access" => "요청 범위 밖 시스템·계정에 접근",
        "hijacked_goal" => "도구 결과가 끌어들인 새 목표를 따름",
        other => other,
    }
}

fn verdict_text(decision: &str) -> &str {
    match decision {
        "ALLOW" => "허용",
        "REVIEW" => "확인",
        "DENY" => "차단",
        other => other,
    }
}

pub(super) fn subgraph_name(key: &str) -> String {
    match key {
        "none" | "" => "해당 없음".into(),
        k if k.starts_with('s') && k[1..].chars().all(|c| c.is_ascii_digit()) => {
            format!("S{}", &k[1..])
        }
        k => k.to_owned(),
    }
}

/// "#7 Edit src/app.py": a node by its number and caption.
pub(super) fn node_name(model: &Model, seq: i64) -> String {
    model
        .node(seq)
        .map_or_else(|| format!("#{seq}"), |n| format!("#{seq} {}", label(n)))
}

/// The shares of the Jev cause answer: the prompt, a tool result, or neither.
fn cause_shares(trace: &Value) -> Option<[(&'static str, f64); 3]> {
    let probabilities = trace["jev"]
        .as_array()?
        .iter()
        .find(|x| x["stage"] == "cause")?
        .pointer("/response/answers/cause/probabilities")?
        .as_object()?;
    if probabilities.is_empty() {
        return None;
    }
    let mut shares = [("request", 0.0), ("segment", 0.0), ("none", 0.0)];
    for (option, p) in probabilities {
        let p = p.as_f64().unwrap_or(0.0);
        let is_segment = option
            .strip_prefix('g')
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()));
        let slot = if is_segment {
            1
        } else if option == "none" {
            2
        } else {
            0
        };
        shares[slot].1 += p;
    }
    Some(shares)
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Verdict,
    Cause,
    Check,
    Jev,
    Paragraphs,
    Props,
}

#[component]
pub fn Inspector(#[props(default)] verdict_only: bool) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let state = use_context::<AppState>();
    let mut detail = use_signal(|| None::<Load<Option<NodeDetail>>>);

    // The selected node's paragraphs and full properties.
    use_effect(move || {
        let selected = (state.selected)();
        let session = state.current.peek().clone();
        let seq = match selected {
            Selection::Node(seq) => seq,
            Selection::None => {
                detail.set(None);
                return;
            }
        };
        let Some(session) = session else { return };
        detail.set(Some(Load::Loading));
        let backend = backend.clone();
        spawn(async move {
            let found = backend.node(&session, seq).await;
            if !matches!(&*state.selected.peek(), Selection::Node(s) if *s == seq) {
                return;
            }
            detail.set(Some(match found {
                Ok(d) => Load::Ready(d),
                Err(e) => Load::Failed(e),
            }));
        });
    });

    let width = (state.inspector_width)();
    let graph = (state.graph)();
    let Load::Ready(model) = graph else {
        return rsx! { aside { class: "inspector", style: "width: {width}px" } };
    };
    let body = match (state.selected)() {
        Selection::None => rsx! {
            div { class: "empty-state",
                p { "노드를 누르면 여기에 자세한 내용이 나옵니다." }
                p { class: "muted", "역(Call)을 누르면 그 Call이 어디서 비롯됐는지, 사용자 요청까지 이어진 노선만 남습니다. 지선 머리의 인용을 누르면 그 도구 결과의 문단을 봅니다." }
            }
        },
        Selection::Node(seq) => {
            rsx! { NodeView { key: "{seq}", model: model.clone(), seq, detail, verdict_only } }
        }
    };
    rsx! { aside { class: "inspector", style: "width: {width}px", {body} } }
}

#[component]
fn NodeView(
    model: Arc<Model>,
    seq: i64,
    detail: Signal<Option<Load<Option<NodeDetail>>>>,
    verdict_only: bool,
) -> Element {
    let Some(node) = model.node(seq).cloned() else {
        return rsx! { p { class: "muted", "이 노드는 그래프에 없습니다." } };
    };
    let is_call = node.decision().is_some() || node.props.contains_key("trace_error");
    let mut tab = use_signal(move || {
        if is_call {
            Tab::Verdict
        } else {
            Tab::Paragraphs
        }
    });
    let trace = parse_json(node.props.get("trace_json"));
    let tabs: Vec<(Tab, &str)> = if is_call {
        vec![
            (Tab::Verdict, "판정"),
            (Tab::Cause, "원인"),
            (Tab::Check, "확인"),
            (Tab::Jev, "Jev"),
            (Tab::Paragraphs, "문단"),
            (Tab::Props, "속성"),
        ]
    } else {
        vec![(Tab::Paragraphs, "문단"), (Tab::Props, "속성")]
    };
    if verdict_only {
        return rsx! {
            div { class: "inspector-body",
                Verdict {
                    model: model.clone(), seq, fields: node.props.clone(), trace: trace.clone(),
                    payload: full_payload(&detail, &node.props),
                }
                if trace["deviation"]["score"].as_f64().is_some()
                    || trace["check"].as_object().is_some_and(|c| !c.is_empty()) {
                    details { class: "verdict-extra",
                        summary {
                            span { class: "extra-show", "추가 정보 보기" }
                            span { class: "extra-hide", "추가 정보 접기" }
                        }
                        div { class: "verdict-extra-body",
                            Deviation { trace: trace.clone() }
                            if trace["check"].as_object().is_some_and(|c| !c.is_empty()) {
                                Check { model: model.clone(), trace: trace.clone() }
                            }
                        }
                    }
                }
            }
        };
    }
    let subgraph = model
        .subgraph_of(seq)
        .map(|n| format!("S{n}"))
        .unwrap_or_default();
    rsx! {
        div { class: "inspector-head",
            span { class: "kind", "#{seq} {node.kind}" }
            if !subgraph.is_empty() {
                span { class: "muted", "{subgraph}" }
            }
            h2 { "{label(&node)}" }
        }
        div { class: "tabs",
            for (t, name) in tabs {
                button { class: if tab() == t { "on" } else { "" }, onclick: move |_| tab.set(t), "{name}" }
            }
        }
        div { class: "inspector-body",
            match tab() {
                Tab::Verdict => rsx! {
                    Story {
                        model: model.clone(),
                        seq,
                        payload: full_payload(&detail, &node.props),
                        trace: trace.clone(),
                        decision: node.decision().unwrap_or("").to_owned(),
                    }
                    Verdict { model: model.clone(), seq, fields: node.props.clone(), trace: trace.clone(), payload: Value::Null }
                    Deviation { trace: trace.clone() }
                },
                Tab::Cause => rsx! {
                    Cause { model: model.clone(), trace: trace.clone() }
                    TracePath { model: model.clone(), seq }
                    Context { trace: trace.clone() }
                },
                Tab::Check => rsx! { Check { model: model.clone(), trace: trace.clone() } },
                Tab::Jev => rsx! { JevLog { model: model.clone(), trace: trace.clone(), highlight: None } },
                Tab::Paragraphs => rsx! { Paragraphs { model: model.clone(), seq, detail, only: None } },
                Tab::Props => rsx! { Props { detail, fallback: node.props.clone() } },
            }
        }
    }
}

#[component]
fn Verdict(
    model: Arc<Model>,
    seq: i64,
    fields: serde_json::Map<String, Value>,
    trace: Value,
    payload: Value,
) -> Element {
    let text = |key: &str| {
        fields
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let decision = text("decision");
    let rule = text("decision_rule");
    let reason = text("decision_reason");
    let error = text("trace_error");
    // The call itself: its tool and what it acts on, whole.
    let show_attempt = !payload.is_null();
    let payload = if show_attempt {
        payload
    } else {
        parse_json(fields.get("payload_json"))
    };
    let tool = payload["tool_name"].as_str().unwrap_or("").to_owned();
    let brief = model
        .node(seq)
        .and_then(|n| n.summary.brief.clone())
        .unwrap_or_default();
    let call = match (tool.is_empty(), brief.is_empty()) {
        (false, false) => format!("{tool}: {brief}"),
        (false, true) => tool,
        (true, _) => brief,
    };
    let score = trace["deviation"]["score"]
        .as_f64()
        .map(|s| format!("이탈 {}", percent(s)))
        .or_else(|| {
            fields
                .get("intent_score")
                .and_then(Value::as_f64)
                .map(|s| format!("score {}", percent(s)))
        });
    let second = match (
        trace["check"]["unwanted"].as_f64(),
        trace["check"]["branch"].as_str(),
    ) {
        (Some(u), _) => Some(format!("2차 {}", percent(u))),
        (None, Some("safeguard")) => Some("설정 변경".to_owned()),
        _ => None,
    };
    let judge_ms = trace.pointer("/jev_ms/judge").and_then(Value::as_i64);
    let check_ms = trace.pointer("/jev_ms/check").and_then(Value::as_i64);
    let judge_text = judge_ms.map_or_else(|| "–".to_owned(), |m| format!("{m}ms"));
    rsx! {
        div { class: "card",
            div { class: "verdict",
                if !decision.is_empty() {
                    {decision_badge(&decision)}
                }
                strong { {rule_text(&rule).unwrap_or("기록된 규칙에 따른 판정")} }
                if let Some(score) = score.filter(|_| !show_attempt) {
                    span { class: "muted", "{score}" }
                }
                if let Some(second) = second.filter(|_| !show_attempt) {
                    span { class: "tag check", "{second}" }
                }
            }
            if !call.is_empty() {
                div { class: "callline", "{call}" }
            }
            if !reason.is_empty() {
                p { "{reason}" }
            }
            if !error.is_empty() {
                p { class: "error", "판정 실패: {error}" }
            }
            if show_attempt {
                Story { model: model.clone(), seq, payload, trace: trace.clone(), decision: decision.clone() }
            }
            dl { class: "facts",
                if !rule.is_empty() {
                    dt { "규칙" }
                    dd { {rule_text(&rule).unwrap_or("엔진의 추가 판정 규칙")} }
                }
                if !show_attempt && (judge_ms.is_some() || check_ms.is_some()) {
                    dt { "Jev 시간" }
                    dd {
                        "판정 {judge_text}"
                        if let Some(ms) = check_ms { " · 2차 {ms}ms" }
                    }
                }
            }
        }
    }
}

#[component]
fn Cause(model: Arc<Model>, trace: Value) -> Element {
    let state = use_context::<AppState>();
    if trace.is_null() {
        return rsx! { p { class: "muted", "원인 추적 기록이 없습니다." } };
    }
    let link = trace["link"].as_str().unwrap_or("").to_owned();
    let parent = subgraph_name(trace["parent"].as_str().unwrap_or(""));
    let shares = cause_shares(&trace);
    let segments: Vec<(i64, Option<i64>, Option<f64>)> = trace["segments"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|x| {
                    Some((
                        x["seq"].as_i64()?,
                        x["paragraph"].as_i64(),
                        x["probability"].as_f64(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    rsx! {
        div { class: "card",
            dl { class: "facts",
                dt { "연결" }
                dd {
                    strong { "{link_text(&link)}" }
                    if trace["picked"] == Value::Bool(false) {
                        span { class: "muted", " (Jev 답 없음 · 가장 가까운 SubGraph로 대체)" }
                    }
                }
                dt { "부모" }
                dd { "{parent}" }
            }
            if let Some(shares) = shares {
                table { class: "bars",
                    for (key, p) in shares {
                        tr {
                            td { "{link_text(key)}" if key == link { span { class: "tag", "선택" } } }
                            td {
                                div { class: if key == link { "bar chosen" } else { "bar" },
                                    i { style: "width: {(p * 100.0).round()}%" }
                                    b { "{percent(p)}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        if !segments.is_empty() {
            h3 { "원인이 된 도구 결과" }
            for (seq, paragraph, p) in segments {
                div { class: "cause-row",
                    button { class: "link", onclick: move |_| state.go_to(seq),
                        "{node_name(&model, seq)}"
                        if let Some(i) = paragraph { " ¶{i}" }
                    }
                    if let Some(p) = p {
                        span { class: "muted", " · p={percent(p)}" }
                    }
                    Preview { key: "{seq}-{paragraph:?}", seq, paragraph }
                }
            }
        }
    }
}

#[component]
fn Paragraphs(
    model: Arc<Model>,
    seq: i64,
    detail: Signal<Option<Load<Option<NodeDetail>>>>,
    only: Option<i64>,
) -> Element {
    let state = use_context::<AppState>();
    let causes = model.paragraph_causes(seq);
    let d = match detail() {
        Some(Load::Ready(Some(d))) => d,
        Some(Load::Failed(e)) => return rsx! { p { class: "error", "{e}" } },
        Some(Load::Ready(None)) => {
            return rsx! { p { class: "muted", "이 노드의 기록을 찾지 못했습니다." } };
        }
        _ => return rsx! { p { class: "muted", "불러오는 중…" } },
    };
    let paragraphs: Vec<_> = d
        .paragraphs
        .iter()
        .filter(|p| only.is_none() || p.index == only)
        .cloned()
        .collect();
    if paragraphs.is_empty() && d.tokens.is_empty() {
        return rsx! { p { class: "muted", "문단이 없습니다." } };
    }
    rsx! {
        for p in paragraphs {
            {
                let traced: Vec<i64> = p.index.and_then(|i| causes.get(&i)).map(|c| c.iter().map(|c| c.from).collect()).unwrap_or_default();
                let model = model.clone(); let is_cause = !traced.is_empty();
                rsx! {
                    div { class: if is_cause { "paragraph cause" } else { "paragraph" },
                        div { class: "paragraph-head",
                            span { class: "muted", "¶{p.index.unwrap_or(0)}" }
                            if p.hidden == Some(true) {
                                span { class: "tag deny", "숨김" }
                            }
                            if p.instructs() {
                                span { class: "tag", "지시 {percent(p.instructs.unwrap_or(0.0))}" }
                            }
                            if let Some(source) = p.source.clone().filter(|s| !s.is_empty()) {
                                span { class: "muted", "{source}" }
                            }
                        }
                        if p.hidden == Some(true) && !p.hidden_rules.is_null() {
                            p { class: "muted", "숨긴 규칙: {hidden_rules(&p.hidden_rules)}" }
                        }
                        pre { "{p.text.clone().unwrap_or_default()}" }
                        for call in traced {
                            button { class: "link", onclick: move |_| state.go_to(call), "← {node_name(&model, call)}" }
                        }
                    }
                }
            }
        }
        if !d.tokens.is_empty() {
            h3 { "토큰" }
            table { class: "props",
                for t in d.tokens.iter() {
                    tr {
                        td { "{t.kind.clone().unwrap_or_default()} · {t.slot.clone().unwrap_or_default()}" }
                        td { "{token_value(&t.decoded)} " span { class: "muted", "{t.category.clone().unwrap_or_default()}" } }
                    }
                }
            }
        }
    }
}

fn token_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[component]
fn Props(
    detail: Signal<Option<Load<Option<NodeDetail>>>>,
    fallback: serde_json::Map<String, Value>,
) -> Element {
    let props = match detail() {
        Some(Load::Ready(Some(d))) => d.props,
        _ => fallback,
    };
    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort();
    rsx! {
        table { class: "props",
            for key in keys {
                tr {
                    td { "{key}" }
                    td { PropValue { name: key.clone(), value: props[key].clone() } }
                }
            }
        }
    }
}

#[component]
fn PropValue(name: String, value: Value) -> Element {
    if name.ends_with("_json") {
        let parsed = parse_json(Some(&value));
        if !parsed.is_null() {
            let text = serde_json::to_string_pretty(&parsed).unwrap_or_default();
            let size = value.as_str().map_or(0, |s| s.chars().count());
            return rsx! {
                details {
                    summary { class: "muted", "JSON · {size}자" }
                    pre { "{text}" }
                }
            };
        }
    }
    match &value {
        Value::Number(n) if name.ends_with("_at") => {
            let at = n.as_i64().and_then(chrono::DateTime::from_timestamp_millis);
            let local = at
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default();
            rsx! { "{n} " span { class: "muted", "({local})" } }
        }
        Value::String(s) => rsx! { pre { class: "inline", "{s}" } },
        Value::Object(_) | Value::Array(_) => {
            rsx! { pre { "{serde_json::to_string_pretty(&value).unwrap_or_default()}" } }
        }
        other => rsx! { "{other}" },
    }
}

/// The Call's payload, whole from its record once read; cut short from the graph until then.
fn full_payload(
    detail: &Signal<Option<Load<Option<NodeDetail>>>>,
    fallback: &serde_json::Map<String, Value>,
) -> Value {
    match &*detail.read() {
        Some(Load::Ready(Some(d))) => parse_json(d.props.get("payload_json")),
        _ => parse_json(fallback.get("payload_json")),
    }
}

/// The rules that hid a paragraph, as one line: a list joined, anything else as JSON.
fn hidden_rules(rules: &serde_json::Value) -> String {
    match rules {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|r| r.as_str().map_or_else(|| r.to_string(), str::to_owned))
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Verdict content embedded next to its station on the route map.
#[component]
pub fn CallBubble() -> Element {
    let mut state = use_context::<AppState>();
    rsx! {
        section {
            class: "call-verdict-dialog",
            aria_label: "툴 호출 판정 상세",
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    state.call_details_open.set(false);
                }
            },
            div { class: "call-verdict-content", onclick: move |e| e.stop_propagation(),
                button { class: "verdict-close", aria_label: "닫기", onclick: move |_| state.call_details_open.set(false), "✕" }
                Inspector { verdict_only: true }
            }
        }
    }
}
