//! The personal app keeps up with the engine by itself. While the window is up the session list
//! is read again every few seconds (the store's incremental read), and the open session's graph
//! when the session grew; while the window is hidden or minimised only the list is read, slowly,
//! for the notifications and the menu bar; after a failed read the next waits longer, and what is
//! on screen stays. A Call the engine refused or held since the app opened is a notification,
//! unless they are turned off, and a click on it opens the app on that Call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::{AppState, Load, Mode, Polling, Screen};
use crate::api::Backend;
use crate::layout::Model;
use crate::metro::Metro;
use crate::model::Session;
use crate::notify::{self, Notice};

/// How long to wait before the next read: every few seconds with the window up, slowly behind
/// it, and twice as long after each failed read in a row, up to a minute.
pub fn wait(polling: Polling) -> Duration {
    const UP: Duration = Duration::from_secs(3);
    const HIDDEN: Duration = Duration::from_secs(30);
    const MOST: Duration = Duration::from_secs(60);
    match polling {
        Polling::Up => UP,
        Polling::Hidden => HIDDEN,
        Polling::Retrying(n) => (UP * 2u32.pow(n.min(5))).min(MOST),
    }
}

/// What tells that a session grew: its Calls, its nodes, its last activity and its verdicts.
type Mark = (i64, i64, Option<i64>, i64, i64);

fn mark(s: &Session) -> Mark {
    (s.calls, s.nodes, s.last_at, s.denied, s.reviewed)
}

/// Calls refused and held in one session since the last read.
#[derive(Clone, Debug, PartialEq)]
pub struct News {
    pub session: String,
    pub denied: i64,
    pub reviewed: i64,
}

/// Each session's refused and held Calls as last read, for what came since. The first list read
/// from a source (the engine, the demo) is what was there before: nothing in it is news.
#[derive(Default)]
pub struct Watch {
    source: Option<String>,
    seen: HashMap<String, (i64, i64)>,
}

impl Watch {
    pub fn news(&mut self, source: &str, list: &[Session]) -> Vec<News> {
        let first = self.source.as_deref() != Some(source);
        if first {
            self.source = Some(source.to_owned());
            self.seen.clear();
        }
        let mut out = Vec::new();
        for s in list {
            let (denied, reviewed) = self.seen.get(&s.id).copied().unwrap_or((0, 0));
            let more = (s.denied - denied, s.reviewed - reviewed);
            if !first && (more.0 > 0 || more.1 > 0) {
                out.push(News {
                    session: s.id.clone(),
                    denied: more.0.max(0),
                    reviewed: more.1.max(0),
                });
            }
            self.seen.insert(
                s.id.clone(),
                (s.denied.max(denied), s.reviewed.max(reviewed)),
            );
        }
        out
    }
}

/// The verdict's name on a notification and in the menu bar.
pub const DENIED: &str = "차단";
pub const REVIEWED: &str = "확인";

/// The session's latest Calls of each verdict, as many as came: each with its verdict's name and
/// the name its step has on the page.
fn latest(metro: &Metro, news: &News) -> Vec<(i64, &'static str, String)> {
    let mut out = Vec::new();
    for (decision, label, count) in [
        ("DENY", DENIED, news.denied),
        ("REVIEW", REVIEWED, news.reviewed),
    ] {
        let mut found: Vec<_> = metro
            .stations
            .iter()
            .filter(|st| st.decision.as_deref() == Some(decision))
            .collect();
        found.sort_by_key(|st| st.seq);
        let skip = found
            .len()
            .saturating_sub(usize::try_from(count).unwrap_or(0));
        out.extend(found[skip..].iter().map(|st| (st.seq, label, st.name())));
    }
    out.sort_by_key(|(seq, _, _)| *seq);
    out
}

/// Notifications at most from one read: the latest, when an agent was stopped many times at once.
const AT_ONCE: usize = 3;

/// The notifications for what came in one session: `차단 · <the Call's name>` over the session's
/// name.
pub fn notices(model: &Model, news: &News, title: &str) -> Vec<Notice> {
    let metro = super::record::unfolded(model);
    let found = latest(&metro, news);
    let skip = found.len().saturating_sub(AT_ONCE);
    let subtitle = clip(title, 60);
    found[skip..]
        .iter()
        .map(|(seq, verdict, name)| Notice {
            title: format!("{verdict} · {name}"),
            subtitle: subtitle.clone(),
            session: news.session.clone(),
            seq: *seq,
        })
        .collect()
}

pub fn clip(text: &str, n: usize) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

/// Where the record comes from: a change of it starts the notifications over.
fn source(state: AppState) -> String {
    let p = state.public.peek();
    format!("{}|{:?}|{}", p.demo, p.engine, p.neo4j_url)
}

/// Brings the window up from the menu bar, the Dock or a notification.
pub fn bring_up() {
    let window = dioxus::desktop::window();
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
}

/// Opens the app on a Call of a session: the record's page, the session, the Call in focus.
pub fn open_call(mut state: AppState, session: String, seq: i64) {
    bring_up();
    state.web.set("record");
    state.screen.set(Screen::Work);
    state.open_call(session, seq);
    state.mode.set(Mode::Graph);
}

pub fn use_live(backend: Arc<Backend>, mut state: AppState) {
    // Clicks on the notifications, from the threads that show them.
    let clicks = use_hook(|| {
        let (tx, rx) = unbounded_channel::<(String, i64)>();
        (tx, Rc::new(RefCell::new(Some(rx))))
    });
    let inbox = clicks.1.clone();
    use_future(move || {
        let inbox = inbox.clone();
        async move {
            let Some(mut rx) = inbox.borrow_mut().take() else {
                return;
            };
            while let Some((session, seq)) = rx.recv().await {
                open_call(state, session, seq);
            }
        }
    });

    let sender = clicks.0.clone();
    use_future(move || {
        let b = backend.clone();
        let sender = sender.clone();
        async move {
            let mut watch = Watch::default();
            // The session whose graph is on screen, as it was when the graph was read.
            let mut drawn: Option<(String, Mark)> = None;
            let mut failures = 0u32;
            loop {
                let window = dioxus::desktop::window();
                let shown = window.is_visible() && !window.is_minimized();
                let polling = match (failures, shown) {
                    (0, true) => Polling::Up,
                    (0, false) => Polling::Hidden,
                    (n, _) => Polling::Retrying(n),
                };
                if *state.polling.peek() != polling {
                    state.polling.set(polling);
                }
                let pause = if shown {
                    wait(polling)
                } else {
                    wait(polling).max(wait(Polling::Hidden))
                };
                tokio::time::sleep(pause).await;
                if *state.screen.peek() == Screen::Setup {
                    continue;
                }
                let list = match b.sessions().await {
                    Ok(list) => super::mine(state, list),
                    Err(_) => {
                        failures = failures.saturating_add(1);
                        if *state.engine_up.peek() != Some(false) {
                            state.engine_up.set(Some(false));
                        }
                        continue;
                    }
                };
                failures = 0;
                let up = Some(engine_answers(state).await);
                if *state.engine_up.peek() != up {
                    state.engine_up.set(up);
                }
                let news = watch.news(&source(state), &list);
                super::take_sessions(state, list.clone());

                // Notifications: not while the app is in front, where the record shows it.
                let window = dioxus::desktop::window();
                let front = window.is_visible() && window.is_focused();
                if !news.is_empty() && !state.prefs.peek().notify_off && !front {
                    tell(&b, &list, &news, &sender).await;
                }

                // The open session's graph, when it grew; read again only while it is seen.
                let shown = window.is_visible() && !window.is_minimized();
                let Some(id) = state.current.peek().clone() else {
                    continue;
                };
                let Some(now) = list.iter().find(|s| s.id == id).map(mark) else {
                    continue;
                };
                let loaded = state.graph_of.peek().as_deref() == Some(id.as_str())
                    && matches!(*state.graph.peek(), Load::Ready(_));
                match &drawn {
                    // Another session's graph was just read by the loaders: taken as it is.
                    Some((was, _)) if *was != id && loaded => drawn = Some((id, now)),
                    None if loaded => drawn = Some((id, now)),
                    Some((was, at)) if *was == id && *at == now => {}
                    _ if !shown || !loaded => {}
                    _ => {
                        if let Ok(data) = b.graph(&id).await {
                            let next = Load::Ready(Arc::new(Model::build(&data)));
                            if state.current.peek().as_deref() == Some(id.as_str()) {
                                if *state.graph.peek() != next {
                                    state.graph.set(next);
                                }
                                drawn = Some((id, now));
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Whether the engine answers: the store did, and for the app's own engine its Collector too,
/// where the hooks send.
async fn engine_answers(state: AppState) -> bool {
    if state.public.peek().engine == crate::settings::Engine::Own && !state.public.peek().demo {
        crate::own::collector_answers().await
    } else {
        true
    }
}

/// Reads each session with news and shows a notification for each Call that came.
async fn tell(
    b: &Backend,
    list: &[Session],
    news: &[News],
    sender: &UnboundedSender<(String, i64)>,
) {
    for n in news {
        let Ok(data) = b.graph(&n.session).await else {
            continue;
        };
        let model = Model::build(&data);
        let title = list
            .iter()
            .find(|s| s.id == n.session)
            .map(Session::title)
            .filter(|t| !t.is_empty())
            .unwrap_or("세션");
        for notice in notices(&model, n, title) {
            notify::show(notice, sender.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn session(id: &str, denied: i64, reviewed: i64) -> Session {
        serde_json::from_value(json!({"id": id, "denied": denied, "reviewed": reviewed})).unwrap()
    }

    #[test]
    fn only_what_came_after_the_first_read_is_news() {
        let mut w = Watch::default();
        // What was there when the app opened.
        assert!(w.news("pg", &[session("a", 2, 1)]).is_empty());
        // The same again: nothing.
        assert!(w.news("pg", &[session("a", 2, 1)]).is_empty());
        // One more refused in it, and a new session with one held.
        let news = w.news("pg", &[session("b", 0, 1), session("a", 3, 1)]);
        assert_eq!(
            news,
            [
                News {
                    session: "b".into(),
                    denied: 0,
                    reviewed: 1
                },
                News {
                    session: "a".into(),
                    denied: 1,
                    reviewed: 0
                },
            ]
        );
        // Another source (the demo) starts over: its sessions are not news.
        assert!(w.news("demo", &[session("d", 5, 5)]).is_empty());
        // A count that went down (a session read again in part) is not news, nor is it forgotten.
        assert!(w.news("demo", &[session("d", 4, 5)]).is_empty());
        assert!(w.news("demo", &[session("d", 5, 5)]).is_empty());
    }

    #[test]
    fn reads_wait_longer_behind_the_window_and_after_failures() {
        assert_eq!(wait(Polling::Up), Duration::from_secs(3));
        assert_eq!(wait(Polling::Hidden), Duration::from_secs(30));
        assert_eq!(wait(Polling::Retrying(1)), Duration::from_secs(6));
        assert_eq!(wait(Polling::Retrying(3)), Duration::from_secs(24));
        assert_eq!(wait(Polling::Retrying(9)), Duration::from_secs(60));
    }

    #[test]
    fn a_notice_names_the_verdict_and_the_latest_calls() {
        let call = |seq: i64, decision: &str, path: &str| {
            json!({"kind": "Call", "props": {"seq": seq, "decision": decision},
                   "summary": {"tool_name": "Write", "brief": path}})
        };
        let data = json!({
            "nodes": [
                {"kind": "Request", "props": {"seq": 1}, "summary": {"prompt": "정리해 줘"}},
                call(2, "DENY", "/w/old.txt"),
                call(3, "ALLOW", "/w/ok.txt"),
                call(4, "REVIEW", "/w/held.txt"),
                call(5, "DENY", "/w/settings.json"),
            ],
            "edges": [], "subgraphs": [],
        });
        let model = Model::build(&serde_json::from_value(data).unwrap());
        let news = News {
            session: "s1".into(),
            denied: 1,
            reviewed: 1,
        };
        let got = notices(&model, &news, "설정  정리");
        let short: Vec<(i64, &str)> = got.iter().map(|n| (n.seq, n.title.as_str())).collect();
        assert_eq!(short.len(), 2, "{got:?}");
        assert_eq!(short[0].0, 4);
        assert!(short[0].1.starts_with("확인 · "), "{}", short[0].1);
        assert_eq!(short[1].0, 5);
        assert!(short[1].1.starts_with("차단 · "), "{}", short[1].1);
        assert!(short[1].1.contains("settings.json"), "{}", short[1].1);
        assert!(
            got.iter()
                .all(|n| n.subtitle == "설정 정리" && n.session == "s1")
        );
    }
}
