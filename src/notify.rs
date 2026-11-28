//! The personal app's notifications: a Call the engine refused or held, as the machine shows any
//! app's. A click on one brings the app up on that Call. On a Mac they go through
//! NSUserNotificationCenter (`mac-notification-sys`), under the app's bundle identifier, which a
//! bare `cargo run` binary takes on as the packaged app has it; elsewhere none is shown.

use tokio::sync::mpsc::UnboundedSender;

/// One notification, and the Call it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub title: String,
    pub subtitle: String,
    pub session: String,
    pub seq: i64,
}

/// The app's bundle identifier: the packaged app's (Cargo.toml) and the one `scripts/dev-run.sh`
/// signs a debug build with.
#[cfg(target_os = "macos")]
const BUNDLE: &str = "dev.withhuman.viewer";

/// Notifications waiting for a click at most: each holds a thread until it is clicked or cleared
/// from the Notification Centre. Those past it are shown all the same, without the click.
#[cfg(target_os = "macos")]
const WAITING: usize = 16;

/// Shows the notice; a click on it sends its session and Call to `clicked`.
#[cfg(target_os = "macos")]
pub fn show(notice: Notice, clicked: UnboundedSender<(String, i64)>) {
    use std::sync::Once;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use mac_notification_sys::{Notification, NotificationResponse, set_application};

    static APP: Once = Once::new();
    static WAITING_NOW: AtomicUsize = AtomicUsize::new(0);
    std::thread::spawn(move || {
        // Set before the first notification: unset, the crate looks an app up by AppleScript.
        APP.call_once(|| {
            if let Err(e) = set_application(BUNDLE) {
                eprintln!("notifications: {e}");
            }
        });
        let wait = WAITING_NOW.fetch_add(1, Ordering::SeqCst) < WAITING;
        let mut n = Notification::new();
        n.title(&notice.title).subtitle(&notice.subtitle);
        if wait {
            n.wait_for_click(true);
        } else {
            n.asynchronous(true);
        }
        let answer = n.send();
        WAITING_NOW.fetch_sub(1, Ordering::SeqCst);
        match answer {
            Ok(NotificationResponse::Click | NotificationResponse::ActionButton(_)) => {
                let _ = clicked.send((notice.session, notice.seq));
            }
            Ok(_) => {}
            Err(e) => eprintln!("notifications: {e}"),
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn show(notice: Notice, clicked: UnboundedSender<(String, i64)>) {
    let _ = (notice, clicked);
}
