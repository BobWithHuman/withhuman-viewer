//! 인벤토리: the machines, people and sessions the engine protects (withhuman-core's agent
//! inventory), on the rail beside 이상 행위 탐지.
//! - Four counts first, protected or not by whether the hooks are heard, running or not by a
//!   session still going within the window:
//!   - machines with a session running;
//!   - registered machines with none (installed only, or idle);
//!   - agents running unprotected, which only the gateway can tell, so named and left empty;
//!   - sessions whose hooks sent no ID, a setup to look at.
//! - Then machines, people and sessions, one picked told beside the list.
//! - People hold the keys: one issued is shown once; one revoked, or a person stopped (every
//!   key), refuses that person's Calls from then on. Those actions go to the engine's admin API,
//!   and its record is 감사's 킬스위치 기록.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use super::main_page::{Card, Empty, clock, now_ms, stamp};
use crate::api::Backend;
use crate::model::{Device, Inventory, Session, SessionRun};
use crate::ui::{AppState, Load, Mode, Screen};

const MIN: i64 = 60_000;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Devices,
    People,
    Sessions,
}

/// Whether a session is still going: not ended, and heard from within the window.
fn running(r: &SessionRun, now: i64, window: i64) -> bool {
    r.ended_at.is_none() && r.last_seen_at.is_some_and(|t| t >= now - window)
}

/// How a machine stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Running,
    Idle,
    /// Registered at install, never heard from.
    InstalledOnly,
    /// Heard from, never registered: a benchmark's label or a setup without the installer.
    Unregistered,
}

impl State {
    fn tag(self) -> (&'static str, &'static str) {
        match self {
            State::Running => ("ok", "활동 중"),
            State::Idle => ("", "쉬는 중"),
            State::InstalledOnly => ("warn", "설치만 됨"),
            State::Unregistered => ("muted", "등록 안 됨"),
        }
    }
}

/// A machine as the list shows it.
#[derive(Clone, Debug, PartialEq)]
struct DeviceRow {
    device: Device,
    state: State,
    running: bool,
    sessions: usize,
    last: Option<i64>,
    permission_mode: Option<String>,
    user: Option<String>,
}

fn device_rows(inv: &Inventory, now: i64, window: i64) -> Vec<DeviceRow> {
    let mut by: BTreeMap<&str, Vec<&SessionRun>> = BTreeMap::new();
    for r in &inv.sessions {
        if let Some(c) = r.client_id.as_deref() {
            by.entry(c).or_default().push(r);
        }
    }
    let mut rows: Vec<DeviceRow> = inv
        .clients
        .iter()
        .map(|d| {
            let runs = by.get(d.client_id.as_str()).cloned().unwrap_or_default();
            let is_running = runs.iter().any(|r| running(r, now, window));
            let last = d.last_seen.max(runs.iter().filter_map(|r| r.last_seen_at).max());
            let latest = runs.iter().max_by_key(|r| r.last_seen_at.or(r.created_at));
            let state = if d.registered_at.is_none() {
                State::Unregistered
            } else if is_running {
                State::Running
            } else if last.is_none() {
                State::InstalledOnly
            } else {
                State::Idle
            };
            DeviceRow {
                state,
                running: is_running,
                sessions: runs.len(),
                last,
                permission_mode: latest.and_then(|r| r.permission_mode.clone()),
                user: d.user_email.clone().or_else(|| latest.and_then(|r| r.user_email.clone())),
                device: d.clone(),
            }
        })
        .collect();
    rows.sort_by_key(|r| (r.state, std::cmp::Reverse(r.last)));
    rows
}

/// A person as the list shows it.
#[derive(Clone, Debug, PartialEq)]
struct PersonRow {
    email: String,
    kind: Option<String>,
    devices: usize,
    sessions: usize,
    active_keys: usize,
    revoked_keys: usize,
    last_used: Option<i64>,
    denied: i64,
}

impl PersonRow {
    fn stopped(&self) -> bool {
        self.active_keys == 0 && self.revoked_keys > 0
    }
}

fn person_rows(inv: &Inventory) -> Vec<PersonRow> {
    let mut emails: Vec<String> = inv.users.iter().map(|u| u.email.clone()).collect();
    for k in &inv.keys {
        if !emails.contains(&k.user_email) {
            emails.push(k.user_email.clone());
        }
    }
    let mut rows: Vec<PersonRow> = emails
        .into_iter()
        .map(|email| {
            let keys: Vec<_> = inv.keys.iter().filter(|k| k.user_email == email).collect();
            let runs: Vec<_> = inv.sessions.iter().filter(|r| r.user_email.as_deref() == Some(&email)).collect();
            let mut devices: HashSet<&str> = runs.iter().filter_map(|r| r.client_id.as_deref()).collect();
            devices.extend(inv.clients.iter().filter(|c| c.user_email.as_deref() == Some(&email)).map(|c| c.client_id.as_str()));
            PersonRow {
                kind: inv.users.iter().find(|u| u.email == email).and_then(|u| u.kind.clone()),
                devices: devices.len(),
                sessions: runs.len(),
                active_keys: keys.iter().filter(|k| k.revoked_at.is_none()).count(),
                revoked_keys: keys.iter().filter(|k| k.revoked_at.is_some()).count(),
                last_used: keys.iter().filter_map(|k| k.last_used_at).max(),
                denied: keys.iter().filter_map(|k| k.denied_count).sum(),
                email,
            }
        })
        .collect();
    // People stopped first, then those using it most recently.
    rows.sort_by_key(|r| (!r.stopped(), std::cmp::Reverse(r.last_used)));
    rows
}

/// What a dialog over the page is for.
#[derive(Clone, Debug, PartialEq)]
enum Ask {
    Issue(String),
    RevokeKey { key_id: String, email: String },
    StopPerson(String),
    Issued { email: String, key_id: String, key: String },
    Admin,
}

#[component]
pub fn InventoryPage() -> Element {
    let state = use_context::<AppState>();
    let backend = use_context::<Arc<Backend>>();
    let bump = use_signal(|| 0u32);
    let found = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            let _ = bump();
            async move { (b.inventory().await, now_ms()) }
        }
    });
    let mut tab = use_signal(|| Tab::Devices);
    let mut window = use_signal(|| 10 * MIN);
    let mut all = use_signal(|| false);
    let mut picked_device = use_signal(|| None::<String>);
    let mut picked_person = use_signal(|| None::<String>);
    let mut ask = use_signal(|| None::<Ask>);
    let note = use_signal(|| None::<(bool, String)>);
    let now = now_ms();
    let p = (state.public)();
    let titles: HashMap<String, Session> = match &*state.sessions.read() {
        Load::Ready(list) => list.iter().map(|s| (s.id.clone(), s.clone())).collect(),
        _ => HashMap::new(),
    };
    let (inv, failed, loaded): (Option<Inventory>, Option<String>, Option<i64>) = match &*found.read() {
        Some((Ok(i), at)) => (i.clone(), None, Some(*at)),
        Some((Err(e), at)) => (None, Some(e.clone()), Some(*at)),
        None => (None, None, None),
    };
    let loading = found.read().is_none();
    let at = clock(loaded);
    let source = p.store();

    let header = rsx! {
        header { class: "cs-head",
            div { class: "cs-crumb",
                strong { "인벤토리" }
                span { class: "sep" }
                span { "기기 · 사용자 · 세션" }
            }
            span { class: "cs-src",
                span { class: if p.demo { "dot demo" } else { "dot" } }
                "{source}"
            }
            label { class: "admin-pick-field",
                span { "활동 기준" }
                select {
                    onchange: move |e| window.set(e.value().parse::<i64>().unwrap_or(10) * MIN),
                    for m in [10, 30, 60] {
                        option { key: "{m}", value: "{m}", selected: window() == m * MIN, "최근 {m}분 안 신호" }
                    }
                }
            }
            button { class: "admin-clear", onclick: move |_| ask.set(Some(Ask::Admin)),
                if p.demo { "관리 API (데모)" } else if p.has_admin_token { "관리 API 연결됨" } else { "관리 API 연결" }
            }
        }
    };

    let Some(inv) = inv else {
        return rsx! {
            div { class: "home admin inventory",
                {header}
                div { class: "cs-grid",
                    div { class: "cs-card inv-none",
                        if loading {
                            p { "불러오는 중…" }
                        } else if let Some(e) = failed {
                            p { class: "admin-failed", "인벤토리를 불러오지 못했습니다: {e}" }
                        } else {
                            h3 { "이 엔진은 아직 인벤토리를 모으지 않습니다" }
                            p { "엔진에 기기 등록과 사용자 키(withhuman-core의 인벤토리 기능)가 들어가면 여기에 기기, 사용자, 세션이 나옵니다. 예전 엔진(Neo4j)에는 없습니다." }
                        }
                    }
                }
                if let Some(a) = ask() { Dialog { ask: a, ask_signal: ask, note, bump } }
            }
        };
    };

    let devices = device_rows(&inv, now, window());
    let people = person_rows(&inv);
    let registered: Vec<&DeviceRow> = devices.iter().filter(|d| d.state != State::Unregistered).collect();
    let unregistered = devices.len() - registered.len();
    let protected_running = devices.iter().filter(|d| d.running).count();
    let running_sessions = inv.sessions.iter().filter(|r| running(r, now, window())).count();
    let installed_only = registered.iter().filter(|d| d.state == State::InstalledOnly).count();
    let idle = registered.iter().filter(|d| d.state == State::Idle).count();
    let no_id: Vec<&SessionRun> = inv.sessions.iter().filter(|r| r.client_id.is_none()).collect();
    let host: HashMap<&str, String> = inv
        .clients
        .iter()
        .map(|c| (c.client_id.as_str(), c.hostname.clone().unwrap_or_else(|| c.client_id.clone())))
        .collect();
    let shown_devices: Vec<DeviceRow> = devices.iter().filter(|d| all() || d.state != State::Unregistered).cloned().collect();
    let device = picked_device()
        .and_then(|id| devices.iter().find(|d| d.device.client_id == id).cloned())
        .or_else(|| shown_devices.first().cloned());
    let person = picked_person()
        .and_then(|e| people.iter().find(|p| p.email == e).cloned())
        .or_else(|| people.first().cloned());
    let mut runs: Vec<SessionRun> = inv.sessions.clone();
    runs.sort_by_key(|r| std::cmp::Reverse((running(r, now, window()), r.last_seen_at.or(r.created_at))));

    rsx! {
        div { class: "home admin inventory",
            {header}
            if let Some((bad, text)) = note() {
                p { class: if bad { "inv-note bad" } else { "inv-note" }, "{text}" }
            }
            div { class: "cs-grid",
                div { class: "cs-row four",
                    Card { title: "보호됨 · 동작 중", note: "우리 훅이 들리는 기기 중 세션이 도는 것", stamp: at.clone(),
                        div { class: "cs-figure static", span { class: "cs-big good", "{protected_running}" } }
                        div { class: "cs-sub", span { "도는 세션 " strong { "{running_sessions}" } "개" } }
                    }
                    Card { title: "보호됨 · 동작 안 함", note: "등록됐지만 지금 도는 세션이 없는 기기", stamp: at.clone(),
                        div { class: "cs-figure static", span { class: "cs-big", "{installed_only + idle}" } }
                        div { class: "cs-sub",
                            span { "설치만 됨 " strong { class: "warn", "{installed_only}" } "대" }
                            span { "쉬는 중 " strong { "{idle}" } "대" }
                        }
                    }
                    Card { title: "보호 안 됨 · 동작 중", note: "훅을 거치지 않고 도는 에이전트", stamp: at.clone(),
                        div { class: "cs-figure static", span { class: "cs-big muted", "–" } }
                        div { class: "cs-sub",
                            span { "모델 호출이 엔진 게이트웨이를 지나는 환경에서만 셀 수 있습니다." }
                        }
                    }
                    Card { title: "ID 없이 들어온 세션", note: "앱·IDE 실행이나 컨테이너에서 기기 ID가 빠짐", stamp: at.clone(),
                        div { class: "cs-figure static", span { class: if no_id.is_empty() { "cs-big" } else { "cs-big bad" }, "{no_id.len()}" } }
                        div { class: "cs-sub",
                            if let Some(last) = no_id.iter().filter_map(|r| r.created_at).max() {
                                span { "마지막 " strong { "{stamp(Some(last))}" } }
                            }
                            span { "설치나 환경 변수를 확인할 기기가 있다는 뜻입니다." }
                        }
                    }
                }
                nav { class: "inv-tabs",
                    for (t, label, n) in [(Tab::Devices, "기기", registered.len()), (Tab::People, "사용자", people.len()), (Tab::Sessions, "세션", runs.len())] {
                        button { key: "{label}", class: if tab() == t { "on" } else { "" }, onclick: move |_| tab.set(t),
                            "{label}" span { class: "n", "{n}" }
                        }
                    }
                    if tab() == Tab::Devices && unregistered > 0 {
                        label { class: "inv-toggle",
                            input { r#type: "checkbox", checked: all(), onchange: move |e| all.set(e.checked()) }
                            "등록 안 된 ID {unregistered}개도 보기 (벤치마크·예전 설치)"
                        }
                    }
                }
                match tab() {
                    Tab::Devices => rsx! {
                        div { class: "cs-row split inv-split",
                            Card { title: "기기", note: "활동 중, 쉬는 중, 설치만 됨 순", stamp: at.clone(),
                                div { class: "inv-table",
                                    div { class: "inv-tr devices head",
                                        span { "기기" } span { "사용자" } span { "에이전트" } span { "권한 모드" }
                                        span { "상태" } span { "세션" } span { "마지막 신호" }
                                    }
                                    if shown_devices.is_empty() { Empty { text: "기기 없음" } }
                                    div { class: "inv-tbody",
                                        for d in shown_devices.iter().cloned() {
                                            {
                                                let on = device.as_ref().is_some_and(|x| x.device.client_id == d.device.client_id);
                                                let id = d.device.client_id.clone();
                                                let (tone, label) = d.state.tag();
                                                let name = d.device.hostname.clone().unwrap_or_else(|| d.device.client_id.clone());
                                                rsx! {
                                                    button { key: "{d.device.client_id}", class: if on { "inv-tr devices on" } else { "inv-tr devices" },
                                                        onclick: move |_| picked_device.set(Some(id.clone())),
                                                        span { class: "name", title: "{d.device.client_id}", "{name}" }
                                                        span { class: "muted", "{d.user.clone().unwrap_or_else(|| \"–\".into())}" }
                                                        span { "{d.device.agent.clone().unwrap_or_else(|| \"–\".into())}" }
                                                        span { class: if d.permission_mode.as_deref() == Some("bypassPermissions") { "bad" } else { "" },
                                                            "{d.permission_mode.clone().unwrap_or_else(|| \"–\".into())}" }
                                                        span { class: "state {tone}", if d.running && d.state == State::Unregistered { "활동 중 · " } "{label}" }
                                                        span { class: "num", "{d.sessions}" }
                                                        span { class: "when", "{stamp(d.last)}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Card { title: "기기 자세히", note: device.as_ref().map_or(String::new(), |d| d.device.client_id.clone()), stamp: at.clone(),
                                if let Some(d) = device.clone() {
                                    DeviceDetail { row: d, inv: inv.clone(), titles: titles.clone(), ask }
                                } else {
                                    Empty { text: "기기를 고르세요" }
                                }
                            }
                        }
                    },
                    Tab::People => rsx! {
                        div { class: "cs-row split inv-split",
                            Card { title: "사용자", note: "멈춘 사람이 위에, 그다음 최근에 쓴 순", stamp: at.clone(),
                                div { class: "inv-table",
                                    div { class: "inv-tr people head",
                                        span { "사용자" } span { "기기" } span { "세션" } span { "키" } span { "마지막 사용" } span { "거부" }
                                    }
                                    if people.is_empty() { Empty { text: "키를 받은 사용자가 없습니다" } }
                                    div { class: "inv-tbody",
                                        for r in people.iter().cloned() {
                                            {
                                                let on = person.as_ref().is_some_and(|x| x.email == r.email);
                                                let email = r.email.clone();
                                                rsx! {
                                                    button { key: "{r.email}", class: if on { "inv-tr people on" } else { "inv-tr people" },
                                                        onclick: move |_| picked_person.set(Some(email.clone())),
                                                        span { class: "name",
                                                            "{r.email}"
                                                            if r.kind.as_deref() == Some("service") { span { class: "badge", "서비스" } }
                                                            if r.devices > 3 { span { class: "badge warn", title: "한 사람의 키가 여러 기기에서 쓰이면 키 공유를 의심해 볼 만합니다", "기기 많음" } }
                                                        }
                                                        span { class: "num", "{r.devices}" }
                                                        span { class: "num", "{r.sessions}" }
                                                        span { class: if r.stopped() { "state bad" } else { "state ok" },
                                                            if r.stopped() { "멈춤" } else { "활성 {r.active_keys}" }
                                                            if r.revoked_keys > 0 && !r.stopped() { " · 회수 {r.revoked_keys}" }
                                                        }
                                                        span { class: "when", "{stamp(r.last_used)}" }
                                                        span { class: if r.denied > 0 { "num bad" } else { "num" }, "{r.denied}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                button { class: "admin-save inv-issue", onclick: move |_| ask.set(Some(Ask::Issue(String::new()))), "새 사용자에게 키 발급" }
                            }
                            Card { title: "사용자 자세히", note: person.as_ref().map_or(String::new(), |p| p.email.clone()), stamp: at.clone(),
                                if let Some(r) = person.clone() {
                                    PersonDetail { row: r, inv: inv.clone(), ask }
                                } else {
                                    Empty { text: "사용자를 고르세요" }
                                }
                            }
                        }
                    },
                    Tab::Sessions => rsx! {
                        Card { title: "세션", note: "도는 세션이 위에, 그다음 최근 순", stamp: at.clone(),
                            div { class: "inv-table",
                                div { class: "inv-tr runs head",
                                    span { "상태" } span { "기기" } span { "사용자" } span { "작업" } span { "시작" }
                                    span { "권한 모드" } span { "모델" } span { "끝" }
                                }
                                div { class: "inv-tbody",
                                    for r in runs.iter().take(300).cloned() {
                                        RunRow { key: "{r.session_id}", run: r.clone(), host: r.client_id.as_deref().and_then(|c| host.get(c).cloned()),
                                            title: titles.get(&r.session_id).cloned(), is_running: running(&r, now, window()) }
                                    }
                                }
                            }
                        }
                    },
                }
            }
            if let Some(a) = ask() { Dialog { ask: a, ask_signal: ask, note, bump } }
        }
    }
}

/// One session as the inventory lists it; it opens on the route map when the graph has it.
#[component]
fn RunRow(run: SessionRun, host: Option<String>, title: Option<Session>, is_running: bool) -> Element {
    let mut state = use_context::<AppState>();
    let id = run.session_id.clone();
    let opens = title.is_some();
    let what = title.as_ref().map_or_else(|| run.session_id.chars().take(8).collect::<String>(), |s| {
        let folder = s.folder();
        if folder.is_empty() { s.title().to_owned() } else { format!("{folder} · {}", s.title()) }
    });
    let source = match run.start_source.as_deref() {
        Some("startup") => "새로 시작",
        Some("resume") => "이어 하기",
        Some("clear") => "지우고 시작",
        Some("compact") => "요약 뒤",
        Some("fork") => "갈라짐",
        _ => "–",
    };
    let ended = match (run.ended_at, run.end_reason.as_deref()) {
        (Some(t), r) => format!("{} · {}", clock(Some(t)), match r {
            Some("prompt_input_exit") => "종료",
            Some("logout") => "로그아웃",
            Some("clear") => "지움",
            Some("resume") => "이어 하기로",
            _ => "끝남",
        }),
        (None, _) if is_running => "–".into(),
        (None, _) => "신호 끊김".into(),
    };
    rsx! {
        button {
            class: if opens { "inv-tr runs" } else { "inv-tr runs dead" },
            title: if opens { "노선도에서 보기" } else { "그래프가 없는 세션" },
            onclick: move |_| {
                if opens {
                    state.open_session(id.clone());
                    state.mode.set(Mode::Graph);
                    state.screen.set(Screen::Work);
                }
            },
            span { class: if is_running { "state ok" } else { "state" }, if is_running { "진행 중" } else { "끝남" } }
            span { class: if run.client_id.is_none() { "bad" } else { "name" },
                "{host.clone().or_else(|| run.client_id.clone()).unwrap_or_else(|| \"ID 없음\".into())}" }
            span { class: "muted", "{run.user_email.clone().unwrap_or_else(|| \"–\".into())}" }
            span { title: "{what}", "{what}" }
            span { class: "when", "{stamp(run.created_at)} · {source}" }
            span { "{run.permission_mode.clone().unwrap_or_else(|| \"–\".into())}" }
            span { class: "muted", "{run.model.clone().unwrap_or_else(|| \"–\".into())}" }
            span { class: "when", "{ended}" }
        }
    }
}

/// A machine told: what it said at install, its projects, its latest sessions, its person.
#[component]
fn DeviceDetail(row: DeviceRow, inv: Inventory, titles: HashMap<String, Session>, ask: Signal<Option<Ask>>) -> Element {
    let mut state = use_context::<AppState>();
    let mut ask = ask;
    let d = row.device.clone();
    let projects: Vec<_> = inv.projects.iter().filter(|p| p.client_id == d.client_id).cloned().collect();
    let mut runs: Vec<SessionRun> = inv.sessions.iter().filter(|r| r.client_id.as_deref() == Some(&d.client_id)).cloned().collect();
    runs.sort_by_key(|r| std::cmp::Reverse(r.last_seen_at.or(r.created_at)));
    runs.truncate(8);
    let text = |v: &Option<String>| v.clone().unwrap_or_else(|| "–".into());
    let user = row.user.clone();
    rsx! {
        div { class: "inv-detail",
            div { class: "dt-facts",
                div { small { "기기 이름" } span { "{text(&d.hostname)}" } }
                div { small { "팀" } span { "{text(&d.team)}" } }
                div { small { "OS · 계정" } span { "{text(&d.os)} · {text(&d.os_user)}" } }
                div { small { "에이전트" } span { "{text(&d.agent)}" } }
                div { small { "훅 버전" } span { "{text(&d.hook_version)}" } }
                div { small { "등록" } span { if d.registered_at.is_some() { "{stamp(d.registered_at)}" } else { "등록 안 됨" } } }
                div { small { "처음 들림" } span { "{stamp(d.first_seen)}" } }
                div { small { "마지막 신호" } span { "{stamp(row.last)}" } }
                div { small { "권한 모드" } span { "{text(&row.permission_mode)}" } }
            }
            h4 { "설치한 프로젝트 " span { class: "n", "{projects.len()}" } }
            if projects.is_empty() { p { class: "muted", "등록 때 알려 온 프로젝트가 없습니다." } }
            for p in projects {
                div { key: "{p.project}", class: "inv-line", code { "{p.project}" } span { class: "when", "{stamp(p.installed_at)}" } }
            }
            h4 { "최근 세션" }
            if runs.is_empty() { p { class: "muted", "이 기기에서 들어온 세션이 없습니다." } }
            for r in runs {
                {
                    let id = r.session_id.clone();
                    let t = titles.get(&r.session_id).map(|s| s.title().to_owned());
                    let opens = t.is_some();
                    rsx! {
                        button { key: "{r.session_id}", class: if opens { "inv-line pick" } else { "inv-line" },
                            onclick: move |_| if opens {
                                state.open_session(id.clone());
                                state.mode.set(Mode::Graph);
                                state.screen.set(Screen::Work);
                            },
                            span { "{t.clone().unwrap_or_else(|| r.session_id.chars().take(8).collect())}" }
                            span { class: "when", "{stamp(r.last_seen_at.or(r.created_at))}" }
                        }
                    }
                }
            }
            if let Some(email) = user {
                div { class: "inv-acts",
                    span { "사용자 " strong { "{email}" } }
                    button { class: "inv-stop", onclick: move |_| ask.set(Some(Ask::StopPerson(email.clone()))), "이 사용자 멈추기" }
                }
            }
        }
    }
}

/// A person told: their keys, each revocable, a new key, and the person's kill switch.
#[component]
fn PersonDetail(row: PersonRow, inv: Inventory, ask: Signal<Option<Ask>>) -> Element {
    let mut ask = ask;
    let mut keys: Vec<_> = inv.keys.iter().filter(|k| k.user_email == row.email).cloned().collect();
    keys.sort_by_key(|k| (k.revoked_at.is_some(), std::cmp::Reverse(k.created_at)));
    let events: Vec<_> = inv.events.iter().filter(|e| e.user_email.as_deref() == Some(&row.email)).take(8).cloned().collect();
    let email = row.email.clone();
    let issue_for = row.email.clone();
    rsx! {
        div { class: "inv-detail",
            div { class: "dt-facts",
                div { small { "종류" } span { if row.kind.as_deref() == Some("service") { "서비스 계정" } else { "사람" } } }
                div { small { "기기" } span { "{row.devices}대" } }
                div { small { "세션" } span { "{row.sessions}개" } }
                div { small { "마지막 사용" } span { "{stamp(row.last_used)}" } }
                div { small { "회수 뒤 거부된 호출" } span { class: if row.denied > 0 { "bad" } else { "" }, "{row.denied}건" } }
            }
            h4 { "키 " span { class: "n", "{keys.len()}" } }
            for k in keys {
                {
                    let key_id = k.key_id.clone();
                    let email = k.user_email.clone();
                    rsx! {
                        div { key: "{k.key_id}", class: if k.revoked_at.is_some() { "inv-key revoked" } else { "inv-key" },
                            div { class: "body",
                                strong { "{k.key_id}" }
                                span { class: "muted", "발급 {stamp(k.created_at)} · {k.created_by.clone().unwrap_or_default()}" }
                                if let Some(at) = k.revoked_at {
                                    span { class: "bad", "회수 {stamp(Some(at))} · {k.revoked_by.clone().unwrap_or_default()} · {k.revoke_reason.clone().unwrap_or_default()}" }
                                    if let Some(first) = k.first_denied_at {
                                        span { class: "muted", "회수 뒤 첫 거부 {stamp(Some(first))}, 모두 {k.denied_count.unwrap_or(0)}건" }
                                    }
                                } else {
                                    span { class: "muted", "마지막 사용 {stamp(k.last_used_at)}" }
                                }
                            }
                            if k.revoked_at.is_none() {
                                button { class: "inv-revoke", onclick: move |_| ask.set(Some(Ask::RevokeKey { key_id: key_id.clone(), email: email.clone() })), "회수" }
                            }
                        }
                    }
                }
            }
            div { class: "inv-acts",
                button { class: "admin-save", onclick: move |_| ask.set(Some(Ask::Issue(issue_for.clone()))), "키 발급" }
                if row.active_keys > 0 {
                    button { class: "inv-stop", onclick: move |_| ask.set(Some(Ask::StopPerson(email.clone()))), "이 사용자 멈추기 (키 모두 회수)" }
                }
            }
            if !events.is_empty() {
                h4 { "기록" }
                for (i, e) in events.into_iter().enumerate() {
                    div { key: "{i}", class: "inv-line",
                        span { "{event_label(&e.kind)} · {e.actor.clone().unwrap_or_else(|| \"엔진\".into())}" }
                        span { class: "when", "{stamp(e.at)}" }
                    }
                }
            }
        }
    }
}

pub(super) fn event_label(kind: &str) -> &'static str {
    match kind {
        "issued" => "키 발급",
        "revoked" => "회수",
        "denied" => "회수된 키로 온 호출 거부",
        _ => "기록",
    }
}

/// Who is acting, for the record: the person signed in to this Mac, until the engine knows the
/// security team's own accounts.
fn me() -> String {
    std::env::var("USER").unwrap_or_else(|_| "security-admin".into())
}

/// The dialog over the page: a key to issue, a key or a person to stop, a key shown once, the
/// admin API's connection.
#[component]
fn Dialog(ask: Ask, ask_signal: Signal<Option<Ask>>, note: Signal<Option<(bool, String)>>, bump: Signal<u32>) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let mut ask_signal = ask_signal;
    let mut note = note;
    let mut bump = bump;
    let p = (state.public)();
    let mut email = use_signal(|| match &ask {
        Ask::Issue(e) | Ask::StopPerson(e) => e.clone(),
        Ask::RevokeKey { email, .. } | Ask::Issued { email, .. } => email.clone(),
        Ask::Admin => String::new(),
    });
    let mut actor = use_signal(me);
    let mut reason = use_signal(String::new);
    let mut api = use_signal(|| p.engine_api.clone());
    let mut token = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let close = move |_| ask_signal.set(None);

    let body = match ask.clone() {
        Ask::Issued { email, key_id, key } => rsx! {
            h2 { "키를 발급했습니다" }
            p { "{email} · {key_id}" }
            div { class: "dt-code",
                code { "{key}" }
                button { title: "복사", onclick: {
                    let key = key.clone();
                    move |_| {
                        let text = serde_json::to_string(&key).unwrap_or_default();
                        document::eval(&format!("navigator.clipboard.writeText({text})"));
                    }
                }, "복사" }
            }
            p { class: "warn", "이 키는 지금 한 번만 보입니다. 사용자에게 안전한 경로로 전달하세요. 설치할 때 configure.py --key로 넣습니다." }
            div { class: "inv-dialog-acts", button { class: "admin-save", onclick: close, "닫기" } }
        },
        Ask::Admin => rsx! {
            h2 { "엔진 관리 API 연결" }
            p { class: "muted", "키 발급과 회수는 엔진(Collector)의 관리 API로 합니다. 주소는 엔진 게이트웨이, 토큰은 엔진의 WITHHUMAN_ADMIN_TOKEN입니다. 토큰은 이 맥의 키체인에 저장합니다." }
            if p.demo { p { class: "warn", "지금은 데모라 키 발급·회수가 이 앱 안에서만 일어납니다." } }
            label { "주소" input { r#type: "text", value: "{api}", oninput: move |e| api.set(e.value()) } }
            label { "관리 토큰"
                input { r#type: "password", value: "{token}", placeholder: if p.has_admin_token { "비워 두면 저장된 토큰 유지" } else { "엔진의 WITHHUMAN_ADMIN_TOKEN" }, oninput: move |e| token.set(e.value()) }
            }
            if let Some(e) = error() { p { class: "bad", "{e}" } }
            div { class: "inv-dialog-acts",
                button { onclick: close, "취소" }
                button { class: "admin-save", onclick: {
                    let backend = backend.clone();
                    move |_| {
                        let t = token();
                        match backend.save_admin(&api(), Some(t.as_str()).filter(|t| !t.is_empty())) {
                            Ok(next) => {
                                state.public.set(next);
                                ask_signal.set(None);
                                note.set(Some((false, "엔진 관리 API 연결을 저장했습니다.".into())));
                            }
                            Err(e) => error.set(Some(e)),
                        }
                    }
                }, "저장" }
            }
        },
        other => {
            let (title, about, needs_reason) = match &other {
                Ask::Issue(_) => ("키 발급", "사용자에게 엔진 키를 새로 발급합니다. 키는 발급 직후 한 번만 보입니다.", false),
                Ask::RevokeKey { .. } => ("키 회수", "이 키로 오는 도구 호출을 엔진이 막습니다. 되돌리려면 새 키를 발급해야 합니다.", true),
                _ => ("사용자 멈추기", "이 사용자의 키를 모두 회수합니다. 그 사람의 에이전트가 하는 도구 호출을 엔진이 막고, 거부된 호출은 감사의 킬스위치 기록에 남습니다.", true),
            };
            let target = match &other {
                Ask::RevokeKey { key_id, email } => format!("{email} · {key_id}"),
                _ => String::new(),
            };
            rsx! {
                h2 { class: if needs_reason { "bad" } else { "" }, "{title}" }
                p { class: "muted", "{about}" }
                if !target.is_empty() { p { strong { "{target}" } } }
                if !matches!(other, Ask::RevokeKey { .. }) {
                    label { "사용자 이메일" input { r#type: "email", value: "{email}", oninput: move |e| email.set(e.value()) } }
                }
                label { "누가 (기록에 남음)" input { r#type: "text", value: "{actor}", oninput: move |e| actor.set(e.value()) } }
                label { if needs_reason { "사유 (필수)" } else { "메모" }
                    input { r#type: "text", value: "{reason}", placeholder: if needs_reason { "예: 승인됐지만 main 강제 푸시였음" } else { "예: 신규 입사자" }, oninput: move |e| reason.set(e.value()) }
                }
                if let Some(e) = error() { p { class: "bad", "{e}" } }
                div { class: "inv-dialog-acts",
                    button { onclick: close, "취소" }
                    button {
                        class: if needs_reason { "inv-stop" } else { "admin-save" },
                        disabled: busy() || email().trim().is_empty() || actor().trim().is_empty() || (needs_reason && reason().trim().is_empty()),
                        onclick: {
                            let backend = backend.clone();
                            let other = other.clone();
                            move |_| {
                                let backend = backend.clone();
                                let other = other.clone();
                                busy.set(true);
                                spawn(async move {
                                    let (who, by, why) = (email().trim().to_owned(), actor().trim().to_owned(), reason().trim().to_owned());
                                    let done = match &other {
                                        Ask::Issue(_) => backend.issue_key(&who, &by, &why).await.map(|(key_id, key)| {
                                            ask_signal.set(Some(Ask::Issued { email: who.clone(), key_id, key }));
                                            None
                                        }),
                                        Ask::RevokeKey { key_id, .. } => backend.revoke(Some(key_id), None, &by, &why).await
                                            .map(|_| Some(format!("{who}의 키 {key_id}를 회수했습니다."))),
                                        _ => backend.revoke(None, Some(&who), &by, &why).await
                                            .map(|n| Some(format!("{who}을(를) 멈췄습니다 (키 {n}개 회수)."))),
                                    };
                                    busy.set(false);
                                    match done {
                                        Ok(message) => {
                                            if let Some(m) = message {
                                                ask_signal.set(None);
                                                note.set(Some((false, m)));
                                            }
                                            bump += 1;
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            }
                        },
                        if busy() { "보내는 중…" } else { "{title}" }
                    }
                }
            }
        }
    };
    rsx! {
        div { class: "inv-shade", onclick: close,
            div { class: "inv-dialog", onclick: move |e| e.stop_propagation(), {body} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ApiKey;

    fn run(client: Option<&str>, user: Option<&str>, last: i64, ended: Option<i64>) -> SessionRun {
        SessionRun {
            session_id: format!("{client:?}-{last}"),
            client_id: client.map(str::to_owned),
            user_email: user.map(str::to_owned),
            created_at: Some(last - 10),
            last_seen_at: Some(last),
            ended_at: ended,
            ..SessionRun::default()
        }
    }

    #[test]
    fn machines_stand_running_idle_installed_or_unregistered() {
        let now = 1_000 * MIN;
        let device = |id: &str, registered: bool| Device {
            client_id: id.into(),
            registered_at: registered.then_some(1),
            ..Device::default()
        };
        let inv = Inventory {
            clients: vec![device("a", true), device("b", true), device("c", true), device("bench/x", false)],
            sessions: vec![
                run(Some("a"), Some("kim"), now - 2 * MIN, None),
                run(Some("b"), Some("lee"), now - 60 * MIN, None),
                run(Some("bench/x"), None, now - MIN, None),
            ],
            ..Inventory::default()
        };
        let rows = device_rows(&inv, now, 10 * MIN);
        let state = |id: &str| rows.iter().find(|r| r.device.client_id == id).unwrap().state;
        assert_eq!(state("a"), State::Running);
        assert_eq!(state("b"), State::Idle, "heard from, but not within the window");
        assert_eq!(state("c"), State::InstalledOnly);
        assert_eq!(state("bench/x"), State::Unregistered);
        assert!(rows.iter().find(|r| r.device.client_id == "bench/x").unwrap().running);
        assert_eq!(rows[0].device.client_id, "a", "running first");
    }

    #[test]
    fn a_person_with_every_key_revoked_is_stopped_and_listed_first() {
        let key = |id: &str, user: &str, revoked: bool| ApiKey {
            key_id: id.into(),
            user_email: user.into(),
            revoked_at: revoked.then_some(5),
            denied_count: revoked.then_some(2),
            ..ApiKey::default()
        };
        let inv = Inventory {
            keys: vec![key("k1", "kim", false), key("k2", "park", true), key("k3", "lee", true), key("k4", "lee", false)],
            sessions: vec![run(Some("a"), Some("kim"), 10, None), run(Some("b"), Some("kim"), 20, None)],
            ..Inventory::default()
        };
        let rows = person_rows(&inv);
        assert_eq!(rows[0].email, "park");
        assert!(rows[0].stopped());
        let lee = rows.iter().find(|r| r.email == "lee").unwrap();
        assert!(!lee.stopped(), "one key still active");
        assert_eq!(rows.iter().find(|r| r.email == "kim").unwrap().devices, 2);
    }
}
