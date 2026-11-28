// Entry of the withhuman single-file build: the bridge to the app first, then the mock backend, then the
// self-hosted fonts, then Langflow's own entry (src/index.tsx) unchanged.
import "./bridge";
// Opened on its own with `#/onboard`: the first launch's demo (before the router reads the hash).
import "./onboard/demo";
// Opened on its own with `?live`: the demo session grows on a timer.
import "./live/demo";
import "./mock";
import "./fonts/fonts.css";
// Korean from the first launch (Langflow's i18n, before its entry loads a language).
import "./i18n";
import "../index";
// The canvas kept in step with what the app renders (after Langflow: it uses its stores).
import "./flow/sync";
// A live session's new steps come in and the canvas follows them.
import "./live/follow";
// Langflow's save / build / store toasts off for read-only sessions.
import "./chrome/alerts";
// The first launch (onboarding) and the first session's tour, over Langflow's page.
import "./launch";
