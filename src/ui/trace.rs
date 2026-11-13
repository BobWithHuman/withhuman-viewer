//! What a Call's trace says beyond the verdict, as the engine's page shows it: the deviation
//! on its gauge, the second check against the prompt, the chain it was traced along, previews of
//! the results that caused it, and every Jev exchange as it was sent and answered.

use std::sync::Arc;

use dioxus::prelude::*;
use serde_json::{Map, Value};

use super::AppState;
use super::inspector::{breach_text, decision_badge, link_text, node_name, subgraph_name};
use crate::api::Backend;
use crate::layout::{Model, clip, percent};
use crate::model::parse_json;

/// Breaches judged on how the call came about rather than what it does.
const PROCESS: [&str; 2] = ["out_of_scope_access", "hijacked_goal"];

/// The Decision Module's defaults, for traces stored before it recorded its thresholds:
/// (review, deny) by what the Call follows from.
fn default_thresholds(link: &str) -> (f64, f64) {
    match link {
        "segment" | "none" => (0.08, 0.15),
        _ => (0.15, 0.3),
    }
}

/// A probability, or a dash when there is none.
fn dash(p: Option<f64>) -> String {
    p.map_or_else(|| "–".to_owned(), percent)
}

fn harm_text(harm: f64) -> &'static str {
    if harm >= 0.7 {
        "큼"
    } else if harm >= 0.2 {
        "작음"
    } else {
        "없음"
    }
}

/// A probability as a bar with its value; `class` is `chosen` or `this` to stand out.
#[component]
pub fn Bar(p: f64, class: Option<&'static str>) -> Element {
    let width = (p.clamp(0.0, 1.0) * 100.0).round();
    rsx! {
        div { class: "bar {class.unwrap_or_default()}",
            i { style: "width: {width}%" }
            b { "{percent(p)}" }
        }
    }
}

/// A score on zones of colour: `zones` are (width, css colour variable) left to right.
#[component]
fn Gauge(score: f64, zones: Vec<(f64, &'static str)>) -> Element {
    let at = (score.clamp(0.0, 1.0) * 100.0).round();
    rsx! {
        div { class: "gauge",
            div { class: "marker", style: "left: {at}%", "{percent(score)}" }
            div { class: "zones",
                for (w, color) in zones {
                    i { style: "width: {(w.max(0.0) * 100.0).round()}%; background: var({color})" }
                }
            }
        }
    }
}

/// The deviation score between the review and deny thresholds, and what each breach added.
#[component]
pub fn Deviation(trace: Value) -> Element {
    let d = &trace["deviation"];
    let Some(score) = d["score"].as_f64() else {
        return rsx! {};
    };
    let link = trace["link"].as_str().unwrap_or("request");
    let recorded = trace["thresholds"]["review"]
        .as_f64()
        .zip(trace["thresholds"]["deny"].as_f64());
    let (review, deny) = recorded.unwrap_or_else(|| default_thresholds(link));
    let worst = d["worst"].as_str().unwrap_or("");
    let probability = |key: &str| {
        if key == "explicit" {
            d["explicit"].as_f64()
        } else {
            d["expected"][key].as_f64()
        }
    };
    // Each breach with its probability and the score it added, largest first.
    let mut keys: Vec<String> = d["contributions"]
        .as_object()
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_else(|| {
            std::iter::once("explicit".to_owned())
                .chain(
                    d["expected"]
                        .as_object()
                        .into_iter()
                        .flat_map(|e| e.keys().cloned()),
                )
                .collect()
        });
    keys.retain(|k| k != "none");
    let contribution = |key: &str| d["contributions"][key].as_f64();
    let mut rows: Vec<(String, Option<f64>, Option<f64>)> = keys
        .into_iter()
        .map(|k| (k.clone(), probability(&k), contribution(&k)))
        .filter(|(_, p, s)| s.unwrap_or(0.0) > 0.001 || p.unwrap_or(0.0) > 0.05)
        .collect();
    rows.sort_by(|a, b| b.2.unwrap_or(0.0).total_cmp(&a.2.unwrap_or(0.0)));
    let harm = d["harm"].as_f64().unwrap_or(0.0);
    let irreversible = d["irreversible"].as_f64().unwrap_or(0.0);
    let zones = vec![
        (review, "--allow"),
        (deny - review, "--review"),
        (1.0 - deny, "--deny"),
    ];
    let threshold_note = if recorded.is_some() {
        ""
    } else {
        " (기본값)"
    };
    rsx! {
        div { class: "card",
            h3 { "이탈 점수 " span { class: "muted", "(1차 질문)" } }
            Gauge { score, zones }
            p { class: "muted",
                "허용 < {percent(review)} ≤ 확인 < {percent(deny)} ≤ 차단 · {link_text(link)}에 이어진 Call의 기준{threshold_note}"
            }
            if rows.is_empty() {
                p { "어긴 것이 없습니다." }
            } else {
                table { class: "breach",
                    tr { td { class: "muted", "위반" } td { class: "num muted", "확률" } td { class: "num muted", "점수" } }
                    for (key, p, s) in rows {
                        tr { class: if key == worst { "worst" } else if s.unwrap_or(0.0) > 0.001 { "" } else { "zero" },
                            td {
                                "{breach_text(&key)}"
                                if PROCESS.contains(&key.as_str()) { span { class: "muted", " (과정형)" } }
                                if key == worst { span { class: "tag", "최대" } }
                            }
                            td { class: "num", "{dash(p)}" }
                            td { class: "num", "{dash(s)}" }
                        }
                    }
                }
            }
            dl { class: "facts",
                dt { "피해" }
                dd { "{harm_text(harm)} " span { class: "muted", "({percent(harm)})" } }
                dt { "되돌릴 수 없음" }
                dd { "{percent(irreversible)}" }
            }
        }
    }
}

fn branch_text(branch: &str) -> &str {
    match branch {
        "safeguard" => "에이전트 설정·훅을 바꾸는 호출 → 규칙으로 차단 (2차 질문 없음)",
        "clear" => "1차에서 위반 없음 · 의심 낮음 → 2차 질문 없이 허용",
        "run" => "세션이 본 프로그램 실행 → 의심과 상관없이 코드가 요청 밖의 일을 하는지 확인",
        "file" => "파일 수정 → 추가한 줄을 사용자가 요청했는지 확인",
        "purpose" => "그 외 호출 → 무엇을 위한 호출인지 확인",
        "none" => "1차 답이 없어 확인하지 않음",
        other => other,
    }
}

/// The exchange of the given stage (`cause`, `check`, …) among the trace's Jev requests.
fn exchange<'a>(trace: &'a Value, stage: &str) -> Option<&'a Value> {
    trace["jev"]
        .as_array()?
        .iter()
        .find(|x| x["stage"] == stage)
}

/// Below the cut the second question allows (or reviews, after a first-question breach); at or
/// above it, denies. A file change asked `requested` first is cut at 0.2.
fn check_cut(check: &Value, exchange: Option<&Value>) -> Option<f64> {
    let branch = check["branch"].as_str()?;
    let requested = exchange.is_some_and(|x| {
        !x.pointer("/request/questions/requested")
            .unwrap_or(&Value::Null)
            .is_null()
    });
    match branch {
        "file" if requested => Some(0.2),
        "file" => Some(0.5),
        "run" | "purpose" => Some(0.4),
        _ => None,
    }
}

/// The second check of the Call against the user's prompt: the branch the trace took, the
/// second score on its own gauge, and what that question saw and answered.
#[component]
pub fn Check(model: Arc<Model>, trace: Value) -> Element {
    let c = &trace["check"];
    if c.is_null() {
        return rsx! { p { class: "muted", "2차 확인 기록이 없습니다." } };
    }
    let found = exchange(&trace, "check").cloned();
    let cut = check_cut(c, found.as_ref());
    let branch = c["branch"].as_str().unwrap_or("").to_owned();
    let breach = c["breach"].as_bool();
    let doubt = c["doubt"].as_f64().unwrap_or(0.0);
    let verdict = c["verdict"].as_str().unwrap_or("").to_owned();
    let unwanted = c["unwanted"].as_f64();
    let label = match branch.as_str() {
        "file" => "요청 밖의 것을 추가할 확률",
        "run" => "코드가 요청 밖의 일을 할 확률",
        _ => "요청 밖 목표·사용자에 반할 확률",
    };
    let below = if breach == Some(true) {
        "--review"
    } else {
        "--allow"
    };
    rsx! {
        div { class: "card check",
            h3 { "2차 확인 " span { class: "muted", "(프롬프트 대조)" } }
            dl { class: "facts",
                dt { "분기" }
                dd { "{branch_text(&branch)}" }
                if let Some(breach) = breach {
                    dt { "1차 판단" }
                    dd {
                        if breach { "위반이 1등 (또는 사용자 조건 위반 ≥ 70%)" } else { "위반 없음이 1등" }
                        span { class: "muted", " · 의심 {percent(doubt)} (기준 0.15)" }
                    }
                }
                dt { "2차 판정" }
                dd {
                    if !verdict.is_empty() {
                        {decision_badge(&verdict)}
                    } else if cut.is_some() {
                        span { class: "muted", "2차 답 없음 → 1차 이탈 점수로 판정" }
                    } else {
                        span { class: "muted", "–" }
                    }
                }
            }
            if let (Some(cut), Some(unwanted)) = (cut, unwanted) {
                h4 { "2차 점수 · {label}" }
                Gauge { score: unwanted, zones: vec![(cut, below), (1.0 - cut, "--deny")] }
                p { class: "muted",
                    if breach == Some(true) { "확인" } else { "허용" }
                    " < {percent(cut)} ≤ 차단"
                    if breach == Some(true) { " · 1차가 위반이라 기준 미만이면 확인" }
                }
            }
            if let Some(x) = found {
                CheckExchange { model, exchange: x }
            }
        }
    }
}

/// `+`/`-` lines of a change, or nothing for an empty side.
fn change_lines(text: &Value, sign: char) -> Vec<String> {
    match text.as_str() {
        Some(t) if !t.is_empty() && t != "(nothing)" => {
            t.lines().map(|l| format!("{sign} {l}")).collect()
        }
        _ => Vec::new(),
    }
}

/// An answer option as a person reads it: `g12p3` is a node's paragraph, `s1` a SubGraph.
fn option_name(model: &Model, option: &str) -> String {
    let Some(rest) = option.strip_prefix('g') else {
        return subgraph_name(option);
    };
    let (seq, paragraph) = rest.split_once('p').unwrap_or((rest, ""));
    match seq.parse::<i64>() {
        Ok(seq) if paragraph.is_empty() => node_name(model, seq),
        Ok(seq) => format!("{} ¶{paragraph}", node_name(model, seq)),
        Err(_) => option.to_owned(),
    }
}

/// An option under its question: a breach of `expected` by what it means, others by
/// [`option_name`].
fn answer_option(model: &Model, question: &str, option: &str) -> String {
    if question == "expected" {
        breach_text(option).to_owned()
    } else {
        option_name(model, option)
    }
}

/// The node an option names (`g12`, `g12p3`), if it names one.
fn option_seq(option: &str) -> Option<i64> {
    let rest = option.strip_prefix('g')?;
    rest.split('p').next()?.parse().ok()
}

#[component]
fn CheckExchange(model: Arc<Model>, exchange: Value) -> Element {
    let state = &exchange["request"]["state"];
    let has_change = !state["added"].is_null();
    let lines: Vec<(bool, String)> = change_lines(&state["added"], '+')
        .into_iter()
        .map(|l| (true, l))
        .chain(
            change_lines(&state["removed"], '-')
                .into_iter()
                .map(|l| (false, l)),
        )
        .collect();
    // A run is judged by the code of each program it runs, as the session last saw it.
    let programs: Vec<(String, String)> = state["programs"]
        .as_object()
        .map(|p| {
            p.iter()
                .map(|(path, code)| (path.clone(), code.as_str().unwrap_or("").to_owned()))
                .collect()
        })
        .unwrap_or_default();
    let user_request = state["user_request"].as_str().unwrap_or("").to_owned();
    let answers = exchange["response"]["answers"].clone();
    rsx! {
        if has_change {
            h4 { "추가·삭제된 줄" }
            div { class: "lines",
                if lines.is_empty() { span { class: "muted", "(변경 없음)" } }
                for (added, line) in lines {
                    span { class: if added { "add" } else { "del" }, "{line}" }
                }
            }
        }
        if !programs.is_empty() {
            h4 { "실행되는 프로그램 코드" }
            for (path, code) in programs {
                details { open: code != "(not seen)",
                    summary {
                        "{path}"
                        if code == "(not seen)" { span { class: "muted", " (세션이 보지 못함)" } }
                    }
                    div { class: "lines", "{code}" }
                }
            }
        }
        if !user_request.is_empty() {
            details {
                summary { "user_request · 사용자 프롬프트 원문" }
                pre { "{user_request}" }
            }
        }
        Answers { model, answers, question_titles: true, highlight: None, trace: Value::Null }
    }
}

fn question_title(key: &str) -> String {
    let what = match key {
        "requested" => " · 사용자가 요청했는가",
        "unasked" => " · 요청 밖의 것을 추가하는가",
        "purpose" => " · 무엇을 위한 호출인가",
        "program" => " · 실행하면 요청 밖의 무엇을 하는가",
        "source" => " · 어느 문단의 지시인가",
        _ => "",
    };
    format!("{key}{what}")
}

/// Each answer of an exchange as bars: a noul as its probability, a choice as its options,
/// largest first. The chosen option, a cause of the Call, or the edge's target stand out.
#[component]
fn Answers(
    model: Arc<Model>,
    answers: Value,
    question_titles: bool,
    highlight: Option<i64>,
    trace: Value,
) -> Element {
    let Some(answers) = answers.as_object().cloned() else {
        return rsx! {};
    };
    let causes: Vec<(i64, Option<i64>)> = trace["segments"]
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(|x| Some((x["seq"].as_i64()?, x["paragraph"].as_i64())))
                .collect()
        })
        .unwrap_or_default();
    let worst = trace["deviation"]["worst"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    rsx! {
        for (key, answer) in answers {
            h4 { if question_titles { "{question_title(&key)}" } else { "{key}" } }
            if let Some(p) = answer["noul"].as_f64() {
                table { class: "bars", tr { td { "참일 확률" } td { Bar { p, class: "chosen" } } } }
            } else {
                table { class: "bars",
                    for (option, p) in sorted(&answer["probabilities"]) {
                        {
                            let seq = option_seq(&option);
                            let paragraph = option.split_once('p').and_then(|(_, i)| i.parse::<i64>().ok());
                            let is_cause = seq.is_some_and(|s| causes.contains(&(s, paragraph)));
                            let chosen = answer["choice"].as_str() == Some(option.as_str())
                                || is_cause
                                || (key == "expected" && option == worst);
                            let class = if seq.is_some() && seq == highlight { "this" } else if chosen { "chosen" } else { "" };
                            let tag = if is_cause { "원인" } else if key == "expected" && option == worst { "최대 위반" } else { "선택" };
                            rsx! {
                                tr {
                                    td { "{answer_option(&model, &key, &option)}" if chosen { span { class: "tag", "{tag}" } } }
                                    td { Bar { p, class } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A probability map's entries, largest first.
fn sorted(probabilities: &Value) -> Vec<(String, f64)> {
    let mut rows: Vec<(String, f64)> = probabilities
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0)))
                .collect()
        })
        .unwrap_or_default();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    rows
}

fn stage_title(model: &Model, x: &Value) -> String {
    match x["stage"].as_str().unwrap_or("") {
        "parent" => "parent 질문".into(),
        "cause" => "원인 질문 (1차)".into(),
        "check" => "2차 확인 질문".into(),
        "paragraph" => format!(
            "{} 문단 질문",
            node_name(model, x["seq"].as_i64().unwrap_or(0))
        ),
        _ => format!("{} 질문", subgraph_name(x["parent"].as_str().unwrap_or(""))),
    }
}

/// Every Jev request behind the verdict as it was sent (state, questions), with its answers.
/// `highlight` marks the options that name the node a cause edge points at.
#[component]
pub fn JevLog(model: Arc<Model>, trace: Value, highlight: Option<i64>) -> Element {
    let exchanges: Vec<Value> = trace["jev"].as_array().cloned().unwrap_or_default();
    if exchanges.is_empty() {
        return rsx! { p { class: "muted", "이 판정에는 Jev state·question이 저장되어 있지 않습니다 (기록을 남기기 전 판정)." } };
    }
    rsx! {
        for x in exchanges {
            {
                let title = stage_title(&model, &x);
                let state: Map<String, Value> = x["request"]["state"].as_object().cloned().unwrap_or_default();
                let questions: Map<String, Value> = x["request"]["questions"].as_object().cloned().unwrap_or_default();
                let answers = x["response"]["answers"].clone();
                let raw = serde_json::to_string_pretty(&answers).unwrap_or_default();
                let is_check = x["stage"] == "check";
                rsx! {
                    div { class: if is_check { "card check" } else { "card" },
                        h3 { "{title}" }
                        h4 { "입력 · state" }
                        for (key, value) in state {
                            details {
                                summary { "{key}" }
                                pre { "{text_of(&value)}" }
                            }
                        }
                        h4 { "입력 · question" }
                        for (key, q) in questions {
                            details {
                                summary { "{key} " span { class: "muted", "({question_kind(&q)})" } }
                                pre { "{question_text(&q)}" }
                            }
                        }
                        h4 { "결과" }
                        Answers { model: model.clone(), answers, question_titles: false, highlight, trace: trace.clone() }
                        details {
                            summary { "answers 원본" }
                            pre { "{raw}" }
                        }
                    }
                }
            }
        }
    }
}

fn question_kind(q: &Value) -> String {
    q["type"].as_str().unwrap_or("").to_owned()
}

/// A question's instructions, then its options.
fn question_text(q: &Value) -> String {
    let criteria = serde_json::to_string_pretty(&q["criteria"]).unwrap_or_default();
    format!("{}\n\n{criteria}", q["instructions"].as_str().unwrap_or(""))
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// The chain the Call was traced along, as the red path on the graph shows it.
#[component]
pub fn TracePath(model: Arc<Model>, seq: i64) -> Element {
    let state = use_context::<AppState>();
    let chain = model.chain(seq);
    if chain.edges.is_empty() {
        return rsx! {};
    }
    let mut seen = Vec::new();
    let steps: Vec<i64> = chain
        .nodes
        .iter()
        .filter_map(|n| n.strip_prefix('n')?.parse().ok())
        .filter(|s| {
            let fresh = !seen.contains(s);
            seen.push(*s);
            fresh
        })
        .collect();
    rsx! {
        div { class: "card",
            h3 { "경로" }
            for (depth, step) in steps.into_iter().enumerate() {
                div { style: "padding-left: {depth.min(8) * 12}px",
                    if depth > 0 { "↑ " }
                    button { class: "link", onclick: move |_| state.go_to(step), "{node_name(&model, step)}" }
                }
            }
        }
    }
}

/// The start of a result that caused the Call: the paragraph it was traced to, or the result.
#[component]
pub fn Preview(seq: i64, paragraph: Option<i64>, #[props(default)] lines: bool) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let state = use_context::<AppState>();
    let found = use_resource(move || {
        let backend = backend.clone();
        let session = state.current.peek().clone();
        async move {
            let session = session?;
            backend.node(&session, seq).await.ok().flatten()
        }
    });
    let text = match &*found.read() {
        Some(Some(detail)) => {
            let from_paragraph = paragraph.and_then(|i| {
                detail
                    .paragraphs
                    .iter()
                    .find(|p| p.index == Some(i))
                    .map(|p| {
                        let hidden = if p.hidden == Some(true) {
                            "[숨김] "
                        } else {
                            ""
                        };
                        format!("{hidden}{}", p.text.clone().unwrap_or_default())
                    })
            });
            from_paragraph.unwrap_or_else(|| {
                let payload = parse_json(detail.props.get("payload_json"));
                let shown = if payload["tool_response"].is_null() {
                    &payload["error"]
                } else {
                    &payload["tool_response"]
                };
                text_of(shown)
            })
        }
        Some(None) => String::new(),
        None => "불러오는 중…".into(),
    };
    if text.is_empty() {
        return rsx! {};
    }
    // Raw text keeps its lines, so tool output reads as it was printed.
    let shown = if lines {
        super::metro::excerpt_lines(&text)
    } else {
        clip(&super::metro::excerpt(&text), 240)
    };
    rsx! { div { class: "preview", "{shown}" } }
}

/// What the engine marked on a paragraph of a tool result: the rules that hid it from a
/// person reading it, and the sensitive values it carried. Nothing when it marked nothing.
#[component]
pub fn ParagraphTags(seq: i64, paragraph: Option<i64>) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let state = use_context::<AppState>();
    let found = use_resource(move || {
        let backend = backend.clone();
        let session = state.current.peek().clone();
        async move {
            let session = session?;
            backend.node(&session, seq).await.ok().flatten()
        }
    });
    let mut tags: Vec<(&'static str, String)> = Vec::new();
    if let Some(Some(detail)) = &*found.read() {
        if let Some(p) =
            paragraph.and_then(|i| detail.paragraphs.iter().find(|p| p.index == Some(i)))
        {
            if p.hidden == Some(true) {
                let rules: Vec<String> = p
                    .hidden_rules
                    .as_array()
                    .map(|r| {
                        r.iter()
                            .filter_map(|v| v.as_str())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                tags.push((
                    "hidden",
                    if rules.is_empty() {
                        "숨은 글".to_owned()
                    } else {
                        format!("숨은 글 · {}", rules.join(", "))
                    },
                ));
            }
            if let Some(i) = p.instructs.filter(|&i| i >= 0.5) {
                tags.push(("instructs", format!("지시하는 글 {}%", (i * 100.0).round())));
            }
        }
        let mut kinds: Vec<String> = detail
            .tokens
            .iter()
            .filter_map(|t| t.category.clone())
            .filter(|c| c != "address")
            .collect();
        kinds.sort();
        kinds.dedup();
        for k in kinds {
            let name = match k.as_str() {
                "credential" => "자격 증명",
                "pii" => "개인 정보",
                "financial" => "금융 정보",
                "device" => "기기 정보",
                _ => "민감한 값",
            };
            tags.push(("token", name.to_owned()));
        }
    }
    rsx! {
        if !tags.is_empty() {
            div { class: "ptags",
                for (tone, text) in tags {
                    span { class: "ptag {tone}", "{text}" }
                }
            }
        }
    }
}

/// What the agent was asked and has done, as the verdict saw it.
#[component]
pub fn Context(trace: Value) -> Element {
    let intents = trace["intents"].as_str().unwrap_or("").to_owned();
    let turn = trace["turn"].as_str().unwrap_or("").to_owned();
    rsx! {
        if !intents.is_empty() {
            details { class: "section",
                summary { "사용자 의도" }
                pre { "{intents}" }
            }
        }
        if !turn.is_empty() {
            details { class: "section",
                summary { "이번 턴 흐름" }
                pre { "{turn}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{attempt, change_lines, check_cut, option_seq, reasons, sorted};

    #[test]
    fn an_edit_shows_what_it_adds_against_what_it_keeps() {
        let payload = json!({"tool_name": "Edit", "tool_input": {"old_string": "import os", "new_string": "# MARK\nimport os"}});
        let (lines, command) = attempt(&payload);
        assert_eq!(
            lines,
            [('+', "# MARK".to_owned()), (' ', "import os".to_owned())]
        );
        assert!(command.is_none());
        let bash = json!({"tool_name": "Bash", "tool_input": {"command": "rm -rf data"}});
        assert_eq!(attempt(&bash).1.as_deref(), Some("rm -rf data"));
    }

    #[test]
    fn reasons_read_the_breach_the_check_and_the_undo() {
        let trace = json!({
            "deviation": {"worst": "hijacked_goal", "expected": {"hijacked_goal": 0.83}, "irreversible": 0.94},
            "check": {"branch": "file", "unwanted": 0.6},
        });
        let why = reasons(&trace);
        assert_eq!(why.len(), 3);
        assert!(why[0].0.contains("새 목표") && why[0].1 == "확률 0.83");
        assert!(why[1].0.contains("추가한 내용"));
        assert!(why[2].0.contains("되돌리기"));
        assert!(reasons(&json!({"deviation": {"worst": "none"}})).is_empty());
    }

    #[test]
    fn options_name_a_result_or_its_paragraph() {
        assert_eq!(option_seq("g12"), Some(12));
        assert_eq!(option_seq("g12p3"), Some(12));
        assert_eq!(option_seq("s0"), None);
        assert_eq!(option_seq("none"), None);
    }

    // The Decision Module's cuts, as the engine's page reads them.
    #[test]
    fn second_check_cuts_by_branch() {
        let file = json!({"branch": "file"});
        let asked = json!({"request": {"questions": {"requested": {}, "unasked": {}}}});
        assert_eq!(check_cut(&file, None), Some(0.5));
        assert_eq!(check_cut(&file, Some(&asked)), Some(0.2));
        assert_eq!(check_cut(&json!({"branch": "run"}), None), Some(0.4));
        assert_eq!(check_cut(&json!({"branch": "clear"}), None), None);
    }

    #[test]
    fn changes_and_answers_read_in_order() {
        assert_eq!(change_lines(&json!("a\nb"), '+'), ["+ a", "+ b"]);
        assert!(change_lines(&json!("(nothing)"), '-').is_empty());
        let rows = sorted(&json!({"a": 0.1, "b": 0.7, "c": 0.2}));
        assert_eq!(
            rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
    }
}

/// What a Call tried to do, from its tool input: the lines an edit adds and removes, what a
/// write puts in, the command a shell runs, or the target of anything else.
fn attempt(payload: &Value) -> (Vec<(char, String)>, Option<String>) {
    let input = &payload["tool_input"];
    let lines = |text: &Value, sign: char| -> Vec<(char, String)> {
        text.as_str()
            .map(|t| t.lines().map(|l| (sign, l.to_owned())).collect())
            .unwrap_or_default()
    };
    match payload["tool_name"].as_str().unwrap_or("") {
        "Edit" => {
            let (old, new) = (&input["old_string"], &input["new_string"]);
            let old_lines = lines(old, '-');
            let new_lines = lines(new, '+');
            // Lines on both sides read as context rather than a change.
            let kept: Vec<String> = old_lines
                .iter()
                .filter(|(_, l)| new_lines.iter().any(|(_, n)| n == l))
                .map(|(_, l)| l.clone())
                .collect();
            let mut out: Vec<(char, String)> = old_lines
                .into_iter()
                .filter(|(_, l)| !kept.contains(l))
                .collect();
            out.extend(
                new_lines
                    .into_iter()
                    .map(|(s, l)| if kept.contains(&l) { (' ', l) } else { (s, l) }),
            );
            (out, None)
        }
        "Write" => (
            lines(&input["content"], '+').into_iter().take(12).collect(),
            None,
        ),
        "Bash" => (Vec::new(), input["command"].as_str().map(str::to_owned)),
        _ => (
            Vec::new(),
            ["file_path", "path", "pattern", "url", "query"]
                .iter()
                .find_map(|k| input[*k].as_str().map(str::to_owned)),
        ),
    }
}

/// Why the engine stopped a Call, as sentences from its trace: the worst breach of the first
/// question, what the second check found, and how hard it is to undo.
pub(super) fn reasons(trace: &Value) -> Vec<(String, String)> {
    let d = &trace["deviation"];
    let mut out = Vec::new();
    if let Some(worst) = d["worst"].as_str().filter(|w| *w != "none") {
        let p = if worst == "explicit" {
            d["explicit"].as_f64()
        } else {
            d["expected"][worst].as_f64()
        };
        out.push((
            format!("{}.", breach_text(worst)),
            p.map(|p| format!("{:.0}%", p * 100.0)).unwrap_or_default(),
        ));
    }
    let c = &trace["check"];
    if let Some(u) = c["unwanted"].as_f64() {
        let what = match c["branch"].as_str() {
            Some("file") => "추가한 내용이 요청 밖의 것입니다.",
            Some("run") => "실행되는 코드가 요청 밖의 일을 합니다.",
            _ => "요청 밖의 목표를 위한 호출이거나 사용자에게 반합니다.",
        };
        out.push((what.to_owned(), format!("{:.0}%", u * 100.0)));
    } else if c["branch"] == "safeguard" {
        out.push((
            "에이전트 설정이나 훅을 바꾸려 했습니다.".into(),
            "규칙".into(),
        ));
    }
    if let Some(i) = d["irreversible"]
        .as_f64()
        .filter(|i| *i >= crate::metro::IRREVERSIBLE)
    {
        out.push((
            "되돌리기 어려운 작업입니다.".into(),
            format!("{:.0}%", i * 100.0),
        ));
    }
    out
}

/// The story of one Call at the top of the inspector, in the order a person asks: what it tried,
/// why it was stopped (or let through), and what told the agent to do it.
#[component]
pub fn Story(
    model: Arc<Model>,
    seq: i64,
    payload: Value,
    trace: Value,
    decision: String,
) -> Element {
    let mut state = use_context::<AppState>();
    let (lines, command) = attempt(&payload);
    let stopped = matches!(decision.as_str(), "DENY" | "REVIEW");
    let why = reasons(&trace);
    let link = trace["link"].as_str().unwrap_or("").to_owned();
    let cause = trace["segments"]
        .as_array()
        .and_then(|s| {
            s.iter().max_by(|a, b| {
                let p = |x: &Value| x["probability"].as_f64().unwrap_or(0.0);
                p(a).total_cmp(&p(b))
            })
        })
        .and_then(|x| {
            Some((
                x["seq"].as_i64()?,
                x["paragraph"].as_i64(),
                x["probability"].as_f64().unwrap_or(0.0),
            ))
        });
    let from_call = cause.and_then(|(r, ..)| model.call_of.get(&r).copied());
    let verdict_note = match decision.as_str() {
        "DENY" => "2. WithHuman이 차단한 이유",
        "REVIEW" => "2. WithHuman이 사용자 확인을 요청한 이유",
        _ => "2. WithHuman이 허용한 이유",
    };
    rsx! {
        div { class: if stopped { "story stopped" } else { "story" },
            h4 { "1. 에이전트가 수행하려는 일" }
            if !lines.is_empty() {
                div { class: "lines",
                    for (sign, line) in lines {
                        span { class: match sign { '+' => "add", '-' => "del", _ => "ctx" }, "{sign} {line}" }
                    }
                }
            } else if let Some(cmd) = command {
                div { class: "lines", span { "{cmd}" } }
            }
            h4 { "{verdict_note}" }
            if why.is_empty() {
                p { class: "muted", if stopped { "기록된 이유가 없습니다." } else { "요청한 일의 한 단계로 판단했습니다." } }
            } else {
                ul { class: "why",
                    for (text, p) in why {
                        li { "{text} " span { class: "muted", "{p}" } }
                    }
                }
            }
            h4 { "3. 에이전트의 행동을 발생시킨 부분" }
            match (link.as_str(), cause) {
                ("segment", Some((result, paragraph, p))) => rsx! {
                    div { class: "cause-quote",
                        button { class: "link", onclick: move |_| {
                            state.call_modal.set(false);
                            state.go_to(from_call.unwrap_or(result));
                        },
                            "{from_call.unwrap_or(result)}번 노드"
                        }
                        if let Some(i) = paragraph { " ¶{i}" }
                        span { class: "muted", " · {p * 100.0:.0}%" }
                        Preview { key: "{result}-{paragraph:?}", seq: result, paragraph }
                    }
                },
                ("request", _) => rsx! { p { "사용자 요청의 다음 단계입니다." } },
                ("none", _) => rsx! { p { "요청에도 도구 결과에도 이어지지 않습니다." } },
                _ => rsx! { p { class: "muted", "원인 추적 기록이 없습니다." } },
            }
        }
    }
}
