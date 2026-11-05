//! The update banner: a newer version found on launch, or from the menu, is offered here and
//! installed only when the user says so.

use std::time::Duration;

use cargo_packager_updater::Update;
use dioxus::prelude::*;

use super::AppState;
use crate::updater;

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, notes: String },
    Installing(String),
    Installed(String),
    Failed(String),
}

#[component]
pub fn UpdateBanner() -> Element {
    let state = use_context::<AppState>();
    let mut status = use_signal(|| Status::Idle);
    // The update found, kept apart: it is what gets installed, not something to show.
    let mut found = use_signal(|| None::<Update>);
    // Whether the user asked, so "up to date" and failures are worth saying.
    let mut asked = use_signal(|| false);

    let mut check = move |by_hand: bool| {
        if matches!(*status.peek(), Status::Checking | Status::Installing(_)) {
            return;
        }
        asked.set(by_hand);
        status.set(Status::Checking);
        spawn(async move {
            let next = match updater::check().await {
                Ok(Some(update)) => {
                    let next = Status::Available {
                        version: update.version.clone(),
                        notes: update.body.clone().unwrap_or_default(),
                    };
                    found.set(Some(update));
                    next
                }
                Ok(None) => Status::UpToDate,
                Err(e) => Status::Failed(e),
            };
            status.set(next);
        });
    };

    // On launch, a moment after the window is up; and whenever the menu asks.
    use_future(move || async move {
        if updater::enabled() {
            tokio::time::sleep(Duration::from_secs(3)).await;
            check(false);
        }
    });
    use_effect(move || {
        if (state.check_updates)() > 0 {
            check(true);
        }
    });

    let install = move |_| {
        let Some(update) = found.peek().clone() else {
            return;
        };
        let version = update.version.clone();
        status.set(Status::Installing(version.clone()));
        spawn(async move {
            status.set(match updater::install(update).await {
                Ok(()) => Status::Installed(version),
                Err(e) => Status::Failed(e),
            });
        });
    };
    let dismiss = move |_| status.set(Status::Idle);

    let by_hand = asked();
    let body = match status() {
        Status::Idle => return rsx! {},
        Status::Checking if !by_hand => return rsx! {},
        Status::UpToDate | Status::Failed(_) if !by_hand => return rsx! {},
        Status::Checking => rsx! { span { "업데이트를 확인하는 중…" } },
        Status::UpToDate => rsx! {
            span { "최신 버전입니다 ({updater::VERSION})." }
            button { onclick: dismiss, "닫기" }
        },
        Status::Available { version, notes } => rsx! {
            span { strong { "새 버전 {version}" } "이 나왔습니다. 지금 버전은 {updater::VERSION}입니다." }
            if !notes.is_empty() {
                span { class: "muted notes", title: "{notes}", "{notes.lines().next().unwrap_or_default()}" }
            }
            button { class: "primary", onclick: install, "지금 설치" }
            button { onclick: dismiss, "나중에" }
        },
        Status::Installing(version) => {
            rsx! { span { "{version} 내려받아 설치하는 중… 서명을 확인한 뒤 바꿉니다." } }
        }
        Status::Installed(version) => rsx! {
            span { "{version} 설치를 마쳤습니다. 다시 시작하면 적용됩니다." }
            button { class: "primary", onclick: move |_| updater::relaunch(), "다시 시작" }
        },
        Status::Failed(message) => rsx! {
            span { class: "error", "{message}" }
            button { onclick: dismiss, "닫기" }
        },
    };
    rsx! { div { class: "update-banner", {body} } }
}
