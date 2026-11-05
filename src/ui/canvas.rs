//! The bar above the map and the legend over it.

use dioxus::prelude::*;

use super::{AppState, Load};

#[component]
pub fn Toolbar() -> Element {
    let mut state = use_context::<AppState>();
    let status = match &*state.graph.read() {
        Load::Ready(model) => {
            let calls = model.nodes.iter().filter(|n| n.kind == "Call").count();
            let stopped = model
                .nodes
                .iter()
                .filter(|n| matches!(n.decision(), Some("DENY" | "REVIEW")))
                .count();
            if stopped > 0 {
                format!("Call {calls} · 멈춤 {stopped}")
            } else {
                format!("Call {calls}")
            }
        }
        _ => String::new(),
    };
    rsx! {
        div { class: "toolbar",
            button { onclick: move |_| state.fit += 1, "전체 보기" }
            button {
                onclick: move |_| {
                    state.reload += 1;
                    state.refresh += 1;
                },
                "새로고침"
            }
            label { class: "check",
                input { r#type: "checkbox", checked: (state.live)(), oninput: move |e| state.live.set(e.checked()) }
                "자동 갱신"
            }
            div { class: "spacer" }
            span { class: "muted", "{status}" }
        }
    }
}

#[component]
pub fn Legend() -> Element {
    rsx! {
        div { class: "legend",
            span { i { class: "line", style: "border-color: var(--line-main)" } "사용자 요청이 시킨 일" }
            span { i { class: "line", style: "border-color: var(--line-branch)" } "도구 결과의 글이 시킨 일" }
            span { i { class: "line loose" } "출처 없음" }
            span { i { style: "background: var(--deny)" } "차단" }
            span { i { style: "background: var(--review)" } "확인 요청" }
        }
    }
}
