//! The security team's dashboards, and the personal app's first launch, are drawn by a page built
//! from `web/console` (`npm run build` there writes `src/admin/console.html`): React, Tailwind and shadcn's components, in one file. A screen
//! shows it in a frame with [`Frame`], handing it the screen's data as JSON, and hears back what
//! the person did there. The page keeps its own styles, so the app's do not reach it.

use dioxus::prelude::*;
use serde_json::Value;

/// The page, one file with its script and styles inside.
const PAGE: &str = include_str!("../admin/console.html");

/// What goes in the app's head: the page as a string, how a screen's frame is made and given its
/// data, and where the page's messages go. `</` is escaped so the page's own closing tags do not
/// end this script.
pub fn head_script() -> String {
    let page = serde_json::to_string(PAGE)
        .unwrap_or_default()
        .replace("</", "<\\/");
    format!(
        "<script>window.__WH_CONSOLE_HTML = {page};\n\
         window.__whConsoleData = {{}};\n\
         window.__whConsoleOn = {{}};\n\
         window.__whConsoleSend = function (page, m) {{ const f = window.__whConsoleOn[page]; if (f) f(m); }};\n\
         window.__whConsoleSet = function (page, d) {{\n\
           window.__whConsoleData[page] = d;\n\
           const host = document.getElementById('wh-console-' + page); if (!host) return;\n\
           let f = host.querySelector('iframe');\n\
           if (!f) {{\n\
             f = document.createElement('iframe'); f.className = 'wh-console-frame';\n\
             f.addEventListener('load', () => window.__whFontInto && window.__whFontInto(f.contentDocument));\n\
             f.addEventListener('load', () => f.contentWindow.WHConsole && f.contentWindow.WHConsole.render(page, window.__whConsoleData[page]));\n\
             f.srcdoc = window.__WH_CONSOLE_HTML; host.appendChild(f); return;\n\
           }}\n\
           if (f.contentWindow && f.contentWindow.WHConsole) f.contentWindow.WHConsole.render(page, d);\n\
         }};</script>"
    )
}

/// One of the page's screens (`detect`, `secrets`, `inventory`, `onboard`) in a frame filling its
/// place: `data` is the screen's data as JSON text, sent again whenever it changes; `onmsg` gets
/// each message the page sends (`send(page, {...})` in `web/console/src/bridge.ts`).
#[component]
pub fn Frame(page: &'static str, data: String, onmsg: Option<EventHandler<Value>>) -> Element {
    use_effect(use_reactive!(|data| {
        document::eval(&format!(
            "requestAnimationFrame(() => window.__whConsoleSet && window.__whConsoleSet('{page}', {data}))"
        ));
    }));
    use_future(move || async move {
        let mut eval = document::eval(&format!(
            "window.__whConsoleOn['{page}'] = (m) => dioxus.send(m); await new Promise(() => {{}});"
        ));
        while let Ok(message) = eval.recv::<Value>().await {
            if let Some(handler) = &onmsg {
                handler.call(message);
            }
        }
    });
    rsx! {
        div { id: "wh-console-{page}", class: "wh-console" }
    }
}
