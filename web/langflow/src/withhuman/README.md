# withhuman: Langflow frontend without a backend

`web/langflow` is Langflow's frontend (`src/frontend`, commit 2b6faa1, MIT,
see `../../LICENSE`) copied as is, minus `tests/`, Dockerfiles, playwright
configs and `.husky`. Everything added for withhuman lives in
`src/withhuman/` plus `vite.single.config.mts`; the Langflow source files
changed for it (21, each marked `withhuman:`) are listed further down.

## Build

```sh
npm ci --ignore-scripts
npm run build:single      # -> dist/index.html (one file, everything inlined)
npm run dev:single        # same setup on the Vite dev server
```

`npm run build` / `npm start` still build and serve plain Langflow (browser
router, real backend through the Vite proxy).

## What the single-file config changes (vite.single.config.mts)

It loads Langflow's own `vite.config.mts` and adds a plugin that:

| Where | Change |
| --- | --- |
| `index.html` | entry `src/index.tsx` -> `src/withhuman/main.tsx`; removes the Google Fonts, favicon and manifest links; `<title>` WithHuman, `<html lang="ko">` |
| `src/routes.tsx` | `createBrowserRouter(` -> `createWithhumanRouter(` (at build time, file untouched) |
| voice-mode worklet | `?url&no-inline` -> `?url`, so it is inlined too |
| `src/assets/temp-pat-*.png` | the templates modal's six "Get started" pictures (~9.6 MB inlined) load as a 1x1 transparent GIF |
| build | `vite-plugin-singlefile`, `publicDir: false`, `outDir: dist` |

## Pieces

- `main.tsx` - imports the bridge, the mock, the local fonts, `i18n/`,
  Langflow's `src/index.tsx`, then `flow/sync.ts`, `live/follow.ts` and
  `chrome/alerts.ts` (they need Langflow's stores), then `launch.tsx`
  (onboarding, tour); `onboard/demo.ts` and `live/demo.ts` right after the
  bridge.
- `i18n/` - Korean from the first launch: Langflow's own `ko.json` plus
  `ko.ts` (overrides of Langflow's keys: short nouns, `""` for description
  lines, no "Langflow") registered on Langflow's i18next instance before its
  entry loads a language; `languagePreference` kept at `ko`;
  `returnEmptyString` on so an `""` override drops the line instead of
  falling back to English.
- `router.ts` - memory router. An iframe `srcdoc` page (`about:srcdoc`) has
  no path to route on, and a hash router breaks Langflow's
  `useHref("/")`-based URLs. Start path = URL hash if present
  (`#/flow/<id>`, `#/all`, `#/settings/general`, ...), else the open
  session's canvas, else `/all` (in the app nothing is rendered yet when the
  router is made; `flow/sync.ts` opens the session when the first `render`
  comes). `#/onboard…` (the onboarding's demo) starts on `/all`. The current
  path is mirrored to the hash. `router()` returns it.
- `fonts/` - Inter, Chivo and JetBrains Mono (OFL) as served by Google Fonts
  for Langflow's `<link>`, stored locally.
- `mock/index.ts` - installed before any Langflow module:
  - `axios.defaults.adapter` -> in-page handler (Langflow's `api` instance and
    the global axios both use it);
  - `window.fetch` -> same handler for `/api/v1`, `/api/v2`, `/health*`;
    other hosts (GitHub stars, Discord count, ...) get `{}` and no request
    leaves the page; `data:`/`blob:` pass through;
  - `EventSource` and `WebSocket` -> inert objects that never connect;
  - opaque origin (`"null"`, e.g. srcdoc in a `null`-origin host):
    `new URL(x, "null...")` gets a fixed base instead of throwing;
  - `localStorage` -> in-memory store if the frame is denied storage;
  - light theme on first load (`themePreference = "light"`; Langflow
    itself follows the OS until the user picks one in the account menu);
  - `<img>` of `/files/profile_pictures/*` -> inlined default avatar
    (`mock/assets/046-rocket.svg`, from Langflow's backend).
- `mock/data.ts` - user `langflow` (superuser, auto-login), the one project
  "기록", `/all` = the three node kinds of `flow/convert.ts`. Langflow's
  starter projects are gone (no starter JSON in the bundle).
- `mock/routes.ts` - the endpoints below; state is in memory (lost on reload).
  `handle` may answer with a promise (`GET flows/<id>` waits on the app).
- `flow/` - the app's sessions as Langflow flows (below).

## Sessions as flows (`flow/`)

- `sessions.ts` - the flows list = `shell.sessions` (name = title,
  description = folder · how long ago, `updated_at` from `at`, newest first,
  `data: null`, `locked: true`), read on every request. `GET flows/<id>`:
  the session last rendered, converted; another session -> `send({ do:
  "open", id })` once (repeated asks share it), resolved by the `render`
  that brings it, 504 after 15 s (Langflow then goes back to the list);
  opened on its own (no app) or unknown -> 404. `PATCH` of a session
  changes nothing.
- `convert.ts` - a `RecordData` as a flow. Boxes = web/record's `boxes()`
  (ported; without `subgraphs`: a box per prompt and one for what followed
  it, a prompt traced by `parents` hanging from that step's box). Each box =
  one `genericNode` (types `WithhumanPrompt` "프롬프트" / `WithhumanToolCalls`
  "도구 호출 N" / `WithhumanAnswer` "답변", Lucide icons MessageSquare /
  Wrench / MessageSquareText), placed as `layoutTree` places it (column =
  depth, one row per box in step order; row heights are Langflow's measured
  card parts). A prompt box's description = the prompt's first two lines.
  Each step (Call, result, answer, fold) = a read-only field (Langflow's
  copy field: `multiline` + `copy_field`): display name = the step's name,
  plus ` · 차단` / ` · 승인 요청` / ` · 승인 거절` / ` · 지시 72%`; value = the
  first line of `text.body` (else its type); the info tooltip = the catch,
  the details, the time. A box that hangs from another gets a first row of
  type `other` named for the tie (`요청에 따라`, `외부 지시 72%`, `결과 보고
  물음 82%`, ...) where the edge lands (a connected text field would show
  "Receiving input" instead of its value). Outputs: prompt "요청"
  (`Message`), tool box "결과" (`JSON`), answer "답변" only when something
  hangs from it; one output per step when ties leave from several steps of
  a box. Edges use Langflow's encoded handles (`œ`-JSON, keys sorted), so
  `cleanEdges` keeps them. The flow is `locked`.
- `sync.ts` - on each `render`: the sessions changed -> Langflow's flow and
  project queries refetched; the open session with a different graph -> set
  on the canvas in place (`processFlows` + `setCurrentFlow`, no fit); a
  session the page did not ask for -> navigate to its canvas (not from
  settings pages). When a session's canvas opens the record panel (below)
  opens on `focus`, else the last denied/rejected Call, on its 경로, and that
  way is framed after Langflow's own fit; with neither, Langflow's fit.

No Langflow source file is changed for any of this.

## Record panel: a step's detail and its way (`detail/`)

The flow page's right-hand panel (mounted beside the canvas in
`pages/FlowPage/index.tsx`), in Langflow's own parts (`Button`, `Badge`,
`Separator`, `Tabs`, `Dialog`, `Input`, `ShadTooltip`, Lucide icons through
`ForwardedIconComponent`) and tokens, so both themes follow Langflow's.

- `model.ts` - ported from web/record: `VERDICT`, `ONLY` (외부 지시 / 차단 /
  승인 요청), `RULE`, `WORST`, `judgeLabel`, a step's tone as Langflow classes
  (refused `error-*`, held `accent-amber`, caught `datatype-yellow`),
  `wayBack` (path.ts: a Call to the tool result that told it or its turn's
  prompt, a result to its Call, a prompt to its parent; each hop named by
  `TIE` / `PARENT_LABEL`, a cause with the SubGraph's `p`, else the result's
  catch score), `opening` (focus, else the latest refused Call).
- `store.ts` - zustand: view (`timeline` / `node` / `step`), the picked step,
  the picked node, the step opened in place, tab (`detail` 상세 / `path` 경로),
  filter chip, open, width (300-640, 380 by default; width and open kept in
  `localStorage`), and `want`: steps to bring into view (asked by picks made
  in the panel, not by clicks on the canvas).
- `RecordPanel.tsx` - `RecordPanel`: nothing picked -> 타임라인 (every step,
  each turn under its prompt; kind icon, name, verdict chip, time; chips
  외부 지시 / 차단 / 승인 요청 with counts filter it); a node clicked -> its
  steps, the latest refused (else first flagged) opened in place with 경로 /
  상세 buttons; a step -> header (kind, name, type, time, verdict) and tabs
  상세 (`DetailBody`) / 경로 (the way from the request to the step: each step
  a card, each hop's name and probability between them; a card opens in place
  and is marked on the canvas). Left edge drags the width (double press
  resets), the header button closes it, Escape goes back to the timeline.
  `RecordToolbar`: 경로 on/off and the panel open/closed, in the canvas
  toolbar's box.
- `DetailBody.tsx` - web/record's `DetailBody`: 차단 이유 / 확인 이유 (the
  deciding name, stage, rule, the score on a review/deny gauge, the link),
  the step's text, its facts (판정 as a chip, 규칙 as code with its name),
  지시한 곳 (the Call whose result it was, the paragraph, the score), a fold's
  steps.
- `text.tsx` - web/record's `code.tsx` and `longtext.tsx`: commands laid out
  as scripts, JSON laid out, `mark` and masked values marked; a long text
  folded under a map of its paragraphs, opened whole in Langflow's `Dialog`
  with numbered lines and a search.
- `canvas.tsx` - the canvas without touching Langflow's flow store: one
  `<style>` element outlines the node holding the picked step (as Langflow
  outlines a selected node), marks the step's row, and on 경로 lights the way's
  nodes and edges (edges in the hop's colour) and dims the rest and the rows
  off the way; it goes away with the panel or the tab. Another gives a locked
  flow's nodes their pointer events back (React Flow sets `none` inline). A
  click on a row (`data-withhuman-field`) picks that step; elsewhere on a node,
  that node; a press that moved (a pan) picks nothing. `frame` fits nodes once
  measured.

## First launch: onboarding and tour (`onboard/`, `tour/`, `launch.tsx`)

`launch.tsx` mounts a React root of its own in `<body id="body">` (Langflow's
theme class and tokens apply; its own `TooltipProvider`), over whatever route
Langflow is on. Langflow's stepper modal got two small hooks for it (table
below).

- `onboard/` - the personal app's first launch, ported from web/console's
  `pages/onboard.tsx`, in Langflow's own multi-step modal
  (`modals/stepperModal`) the way its `knowledgeBaseUploadModal` uses it:
  `StepperModal` (header icon tile + title + `ProgressIndicator`, the step
  in the bordered content box), `SidePanel` (the live status),
  `StepperModalFooter` (이전 / 다음 / the primary; its left "help" button
  for a second action), `useStepperState` for the steps. Shown while
  `shell.page` is `"onboard"` (or before any `render` once `views.onboard`
  came), always open: no close button, Escape and outside presses ignored,
  the backdrop plain `bg-background` instead of the dimming; a press on the
  header moves the window. Size: Langflow's dialog width 512 px
  (`max-w-lg`) x the knowledge-base modal's `MODAL_HEIGHT_DEFAULT` 580 px,
  plus the side panel (`SIDE_PANEL_WIDTH_PX` 300 + 1 px rule), so the app's
  first-launch window is 813 x 582 logical px. Data = `views.onboard`
  (src/ui/onboard.rs `OnboardPage`, through `__whRecordView('onboard', d)`);
  messages = `send({ page: "onboard", do })` with the old page's `do`s and
  fields (`check_jev {key}`, `model {model}`, `pick {agent}`, `python`,
  `start {jev,key,model,agents}`, `open {try}`, `server_test` /
  `server_save` `{url,user,database,password,engine,key}`).
  - `Onboard.tsx` - the modal and the flow: 엔진 -> 판정 -> 에이전트 -> 준비
    (this Mac), or 엔진 -> 회사 엔진 (the company's engine; `role:
    "developer"` starts there). Footer: 다음 (판정 gated on the key), 시작
    (`start`, spinning until done; again after a failed hook), then 체험
    (`open {try:true}`) with 기록 (`open`) on the left; 회사 엔진: 연결 확인
    on the left, 열기 (`server_save`) once the test is ok.
  - `steps.tsx` - the steps' content: 엔진 and 판정 as Langflow's radio
    option rows (`RadioGroup` in a bordered row, as the deploy dialog's
    provider phase) with `Badge`s; Jev's key (`Label` + `Input` + 확인,
    state as a `Badge`) and model (`Select` from `jev.models`), as
    StepConfiguration's fields; 에이전트 as checkbox rows with the logo
    (`chrome/agent-icon.tsx`), found or not, a folder button for per-folder
    agents, a Python row with 설치 when it is missing; 준비 as StepReview's
    summary (`SummaryItem` rows: engine, address, judging, model, agents'
    logos, Python); 회사 엔진 the store and hook fields.
  - `StatusPanel.tsx` - the side panel in FilesPanel's layout: 상태 with the
    parts up, rows for the engine's parts, the judging, the agents picked
    (each state as a Langflow icon: circle / `Loader2` / check / mark with
    the app's text on hover), or the company's engine with its session
    count.
  - `data.ts` the types; `demo.ts` - opened on its own, `#/onboard` shows it
    over the demo and answers its messages as the app would;
    `?step=where|judge|agents|ready|store&judge=jev&jev=busy|ok|err&kept=1&
    folder=1&python=0&agents=busy|done|err&run=busy|ok|err&up=N&done=1&
    role=developer&server=busy|ok|err` set a state; `?nokey` / `?nohook`
    (on any route) take the key / the hooks away for the engine menu's dots.
- `tour/` - the first session's tour. `store.ts`: `wantTour()` (the
  onboarding finished; localStorage `withhuman.tour` = `want`), `startTour()`
  (engine menu 둘러보기), done once finished or skipped. `Tour.tsx`: waits
  for a `/flow/` canvas with nodes and no onboarding, then five marks in
  Langflow's `PopoverContent` with Radix's arrow, anchored on the page with a
  lit frame: the refused Call's node (framed with React Flow's `fitView`),
  the edge into a "외부 지시" row (drawn red and dashed while shown; else the
  경로 button), the record panel on 상세 (verdict, gauge), the session list,
  the engine menu. 건너뛰기 / Escape end it, ← → move. `#…?tour` asks for it.
- `chrome/EngineMenu.tsx` - after setup: a dot on the toggle and a first row
  훅 연결 (red; `shell.agents` empty) / 키 등록 (amber; `views.engine` jev
  without `has_key`, or `shell.engine.jev_on && has_key === false` when the
  app sends them); 둘러보기 starts the tour again.

## Mocked endpoints (`/api/v1/...` unless noted)

| Endpoint | Answer |
| --- | --- |
| `GET auto_login`, `GET session`, `GET users/whoami`, `POST refresh`, `POST logout`, `PATCH users/:id` | the mock user, logged in |
| `GET version`, `GET config`, `GET /health`, `GET /health_check` | 1.13.4, full config, ok |
| `GET all` | the three node kinds of a session (`flow/convert.ts`) |
| `GET flows/basic_examples/`, `GET starter-projects/` | none |
| `GET/POST projects/`, `GET/PATCH/DELETE projects/:id` | project list, paginated project page, create/rename/delete |
| `GET flows/` (list / paginated / `components_only`), `GET/PATCH/DELETE flows/:id`, `POST flows/`, `DELETE flows/` | the app's sessions (`flow/sessions.ts`), then flows made in the page (in memory) |
| `GET flows/:id/versions/`, `.../version-state`, `.../note_translations`, `.../events`, `GET flows/public_flow/:id` | empty history, current token, `{}`, no events, the flow |
| `POST custom_component/update` | the component back with the template sent (no field is recomputed) |
| `POST authz/me/permissions` | every requested action allowed (OSS pass-through) |
| `GET/POST variables/`, `PATCH/DELETE variables/:id` | in-memory global variables |
| `GET api_key/`, `GET store/check*`, `GET store/tags` | none |
| `GET monitor/builds`, `monitor/messages*`, `monitor/transactions`, `monitor/traces` | empty |
| `GET files/`, `files/profile_pictures/list`, `models*`, `mcp/servers`, `mcp/project/:id`, `.../installed`, `knowledge_bases/`, `knowledge-base-storage/status`, `memories`, `/api/v2/workflows/pending` | empty (MCP project lists the project's flows as tools) |
| `POST build/*`, `run/*`, `webhook/*`, `responses`, `agentic/*`, `workflows*`; `GET build/*` | 503 `No Langflow backend` (Langflow shows its own build error) |
| anything else | 200 with `[]` (GET) or `{}`; logged once as `[withhuman mock] unknown route` |

## Removing it

Build with `npm run build` instead of `build:single`, or delete
`src/withhuman/` and `vite.single.config.mts` and the two npm scripts and
the `vite-plugin-singlefile` devDependency.

## Chrome: sessions list, header, settings (`chrome/`)

The frame around the canvas, in Langflow's own components and classes; only
the content is WithHuman's. Everything sent goes through `bridge.ts`'s
`send`.

- `chrome/SessionsPage.tsx` - the flows list (`/all`, `/all/folder/<id>`,
  `/flows`, `/components`, `/mcp`) as the sessions' overview. `SessionsLayout`
  frames it like the flow page (same `SidebarProvider` at 20rem, same
  `SessionSidebar`), so both pages share one left panel; no projects
  sidebar, no new flow / import / templates / MCP tab. `SessionsOverview`
  is drawn with the list page's own parts: HomePage's title row, its tab bar
  as the filter 전체 / 차단 / 확인 / 외부 지시 (counts after the search), its
  search field and list / grid switch, ListComponent's card rows (swatch
  toned by verdict, title, live dot, folder · how long ago, badges 차단 N /
  확인 N / 외부 지시 N, calls), grouped 오늘 / 어제 / 이번 주 / 이전. A row
  opens `/flow/<id>`. Empty states in EmptyFolder's layout: no `shell` yet ->
  the loading mark; no session -> 세션 없음 with 체험 (`send({do:"try"})`,
  only when `own` and the engine is up) and 훅 (`/settings/hooks`); engine
  down -> its label with 설정 (`/settings/engine`, or `send({do:"settings"})`
  when not `own`) and 훅; engine down with sessions -> a strip with the
  label and 설정 above the list.
- `chrome/CanvasMark.tsx` - `CanvasMark`: the flow page while the session is
  on its way (no flow yet: the pulsing app icon) or arrived with no node
  (the same for 2.5 s, then a Workflow icon and 단계 없음).
  `WithhumanWait`: the pulsing icon on its own page, for `/login`,
  `/signup`, `/login/admin`.
- `chrome/alerts.ts` - Langflow's toasts cut at the store (`useAlertStore`'s
  setters wrapped once): success and notice only for a copy (복사); errors
  only when Korean and not about saving, building, the store, updates or the
  backend. So "Flow saved successfully!" on leaving a canvas, auto-save and
  build errors never show.
- `chrome/SessionSidebar.tsx` - the flow page's left panel (and the
  overview's): Langflow's
  segmented rail (one item, 기록), its search field (`/` focuses it, Enter
  opens the first match), `SidebarGroup` labels 차단 (`denied > 0`) then
  오늘 / 어제 / 이번 주 / 이전, `SidebarMenuButton` rows with `Badge`s 차단 N /
  확인 N and a live dot; hover shows folder, calls, outside, age. A row
  navigates to `/flow/<id>` (flow id = session id); the mock's
  `GET flows/<id>` asks the app (`flow/sessions.ts`). The open row is the
  route's session if listed, else `shell.current`; none on the overview.
- `chrome/EngineMenu.tsx` - the account menu as the engine's status: toggle =
  up/down dot, label, Jev, agents' logos (Langflow's icon set); menu = engine,
  판정, agents, 훅, 엔진 (own -> `/settings/engine`, else `send({do:"settings"})`),
  테마 (Langflow's `ThemeButtons`).
- `chrome/app.ts` - `send({do:"theme", theme})` on load and whenever
  Langflow's dark store changes (light / dark / system resolved); header
  `mousedown` on bare space -> `send({do:"drag"})`, double press ->
  `send({do:"zoom"})`.
- `chrome/AppSync.tsx` - `shell.page` "hooks" / "settings" opens
  `/settings/hooks` / `/settings/engine`; "record" after one of them leaves
  the page it had open for the open session's canvas.
- `chrome/HooksPage.tsx`, `chrome/EnginePage.tsx` - Settings › 훅 and › 엔진
  in Langflow's `Card`, `Button`, `Input`, `Select`, `Switch`, `Badge` and
  `SettingsPageHeader`; same messages as web/record's HooksView / EngineView
  (`{page:"hooks", do:…}`, `{page:"engine", do:…}`). Each tells the app
  `{do:"page", page:"hooks"|"settings"}` while open and `"record"` on leaving.
- `chrome/settings-items.tsx`, `chrome/logo.tsx` (+ `withhuman.png`, the app
  icon), `chrome/agent-icon.tsx`, `chrome/settings-parts.tsx`.

Langflow files changed for it (each marked `withhuman:` in place):

| File | Why |
| --- | --- |
| `customization/components/custom-header-logo.tsx` | WithHuman's icon instead of Langflow's logo (Langflow's own seam) |
| `customization/components/custom-AccountMenu.tsx` | the account menu is `EngineMenu` |
| `customization/components/custom-langflow-counts.tsx` | no GitHub stars / Discord count |
| `customization/components/custom-settings-page-frame.tsx` | title 설정, no description line, back -> open session's canvas |
| `customization/utils/custom-routes-store.tsx` | adds `/settings/hooks` and `/settings/engine` |
| `routes.tsx` | `/settings` index redirects to `hooks` instead of `general`; the flows list routes (`CollectionPage` / `HomePage`) are `SessionsLayout` / `SessionsOverview`; `/login`, `/signup`, `/login/admin` show `WithhumanWait`; `/playground/:id` redirects to `/` |
| `hooks/use-document-title.ts` | tab title = the page's name alone (WithHuman when none), no "\| Langflow" |
| `pages/SettingsPage/use-settings-sidebar-items.tsx` | settings menu = 훅, 엔진 only (Langflow's list kept as `useLangflowSettingsSidebarItems`, unused) |
| `components/core/appHeaderComponent/index.tsx` | 80 px left for the traffic lights, drag / zoom on bare header, mounts `AppSync`, no logo button, no notifications / separator |
| `components/core/appHeaderComponent/components/FlowMenu/index.tsx` | flow name opens no details editor (`WITHHUMAN_VIEW_ONLY`); the project crumb keeps Langflow's navigation to the flows list; the 실시간 chip (`live/LiveChip.tsx`) beside the name |
| `pages/FlowPage/index.tsx` | `SessionSidebar` instead of `FlowSidebarComponent`; panel 20rem instead of 17.5rem; `RecordPanel` beside the canvas; `CanvasMark` (loading / empty mark) over the page |
| `pages/FlowPage/components/flowSidebarComponent/components/sidebar-nav-items.ts` | rail (and the collapsed rail on the canvas) = 기록 only; Langflow's list kept as `LANGFLOW_NAV_ITEMS` |
| `components/core/flowToolbarComponent/index.tsx` | no Playground / Share / run toolbar, its shortcuts off; the record panel's 경로 / 상세 buttons (`RecordToolbar`) in its box |
| `CustomNodes/GenericNode/components/NodeInputField/index.tsx` | `data-withhuman-field={name}` on a field's row, so a click on a step's row picks it and the panel can mark it |
| `components/core/canvasControlsComponent/CanvasControls.tsx` | no Langflow Assistant button, no "Try the new Langflow Assistant" banner, no help menu (Langflow links) |
| `components/common/loadingComponent/index.tsx` | WithHuman's icon instead of Langflow's logo, no "Loading..." line |
| `CustomNodes/GenericNode/components/NodeStatus/index.tsx` | no run (▶) button in node headers: the canvas is read-only (`WITHHUMAN_READ_ONLY`) |
| `modals/stepperModal/StepperModal.tsx`, `modals/stepperModal/types.ts` | `hideCloseButton` and `overlayClassName` passed through to `DialogContent` (the first launch: not closable, no dimming) |
| `modals/stepperModal/components/ProgressIndicator.tsx` | the "N/M completed" label through i18n (`stepper.completed`, Langflow's English as default; `ko.ts` gives `N/M`) |
| `components/core/parameterRenderComponent/components/copyFieldAreaComponent/index.tsx` | copy toast "복사됨" instead of "Endpoint URL copied"; button label 복사 / 복사됨 instead of Copy / Copied |

## Overview charts (`viz/`)

Above the overview's list (`chrome/SessionsPage.tsx`), from `shell.sessions` only (each session's
`calls` / `denied` / `review` / `outside`, counted on the day of its `at`):

- `viz/OverviewCharts.tsx` - three panels in Langflow's border and type:
  - 일별 호출: a bar per day from the oldest listed session to today (7 to 31 days), stacked
    허용 / 확인 / 차단 (allowed = calls - denied - review), 2 px gaps, a legend;
  - 폴더: a stacked bar per folder (the folder's last part), the five biggest, `+N` for the rest;
  - 외부 지시: a ring of outside / calls, with the count, the calls and the sessions with any.
  Hover on a bar or a row: Langflow's tooltip (`ShadTooltip`) with 세션, 차단, 확인, 허용, 호출.
  A day or a folder pressed filters the list (pressed again lets it go; the others dim); the ring
  toggles the 외부 지시 tab. Each chart counts what the other picks leave (the day range stays the
  whole list's).
- `viz/colors.ts` - the charts' colours from Langflow's tokens (it ships no chart palette and no
  chart library; nothing was installed): emerald (quiet) / `--status-yellow` / destructive, indigo
  for 외부 지시.
- In `SessionsPage.tsx`: the search feeds the charts; the list = search, then day, folder and tab;
  chips under the search field for the tab, the day and the folder (each `×` lets it go, `전체`
  resets all when there are two or more).

## A live session (`live/`)

- `live/follow.ts` - on Langflow's flow store: when the open flow's nodes change (flow/sync.ts set
  a newer graph of the same session), the nodes and the step rows (`step_*` template keys) that
  were not there before play an entrance (Web Animations: fade and slide; a row flashes emerald),
  and while the session is `live` nodes glide to their new places (a transform transition). The
  view then centres the newest step's row at the current zoom, unless the person dragged or
  wheeled on the canvas since the session opened (a click does not count). `useLive` = { live,
  following }.
- `live/LiveChip.tsx` - in the header beside the session's name while it is `live`: the live dot
  and 실시간; once the view was moved, a crosshair too, and pressing it follows again.
- `live/demo.ts` - opened on its own with `?live` in the hash (`#/flow/<demo id>?live`): the demo
  session starts with two steps and gets one more every 1.8 s, then a third turn (Grep, Read,
  Edit, Bash, a GitHub PR held for 확인, an answer), then `live` goes off. Its row's counts follow.

## Compare two sessions (`compare/`)

`/all?compare=<a>,<b>` (the overview's route; no Langflow route added). Opened from the overview
(each row's checkbox, two at a time -> 선택 2 / 비교) or the session sidebar's row menu (`…` ->
비교: with the session on the canvas; on the overview it picks, and the second pick opens it).

- `compare/load.ts` - the app hands one open session at a time, so each is fetched the mock's way
  (`flowFor`: `send({do:"open", id})`, awaiting the `render` that brings it), the open one first,
  one after the other; both kept in memory with their `RecordData`, converted and `processFlows`ed.
  Opened on its own, the other sessions get a record made up from their counts
  (`compare/demo.ts`).
- `compare/CompareView.tsx` - a strip on top as a butterfly chart (A's bar grows left, B's right)
  for 호출, 차단, 확인 (from `shell.sessions`), 외부 지시, 단계, 턴 (from the records), with Δ; a row
  pressed lights those steps on both canvases and dims the rest (a chip lets it go). Below, both
  canvases: React Flow instances of their own with Langflow's `nodeTypes` (GenericNode), read-only,
  flagged rows barred in their colour, each fitted once Langflow's nodes have laid out. Langflow's
  `DefaultEdge` reads its ends from the main canvas' store, so the compare canvases draw their
  edges with React Flow's bezier from the ends it gives. 동기 moves both views together.
- `compare/store.ts` - the picks (zustand), `comparePath`.
