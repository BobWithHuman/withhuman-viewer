// The desktop app's first launch, as a tutorial in the landing's style and from the landing's code
// (bobwithhuman.github.io/site): one scene at a time, moved by a wheel gesture, the arrow keys or
// the bar's buttons, each coming in with the landing's reveals.
//   1 intro   the hero: the field, the lens, the headline, 시작
//   2 gate    the "who asked?" diagram, its sources clickable
//   3 where   this Mac (its engine's parts) or the company's engine
//   4 judge   rules, or Jev (key, 확인, model)            } this Mac
//   5 agents  each agent's card; folders; Python          }
//   6 ready   시작, the parts and hooks coming up, 체험 / 기록 보기
//   4 store   (the company's engine) the form, 연결 확인, 열기
// The rules for next and back, and every message, are the existing onboarding's
// (web/langflow/src/withhuman/onboard/Onboard.tsx).
import "./tw.css";
import "./style.css";
import gsap from "gsap";
import { SplitText } from "gsap/SplitText";
import { ScrambleTextPlugin } from "gsap/ScrambleTextPlugin";
import { $, $$, calm, hydrateIcons, q, reduced } from "./lib";
import { ICON } from "./icons";
import { initBurst, initCursor, initMagnets } from "./fx";
import { initField, introAndHeadline } from "./hero";
import { initGate } from "./gate";
import { sceneIn, sceneOut } from "./sections";
import { agentsScene, judgeScene, readyScene, storeScene, usable, whereScene, type Ctx, type Derived, type Scene, type Ui } from "./scenes";
import { current, onData, send, startDemo, STORE, type Data } from "./state";
import logoUrl from "./favicon.svg";

gsap.registerPlugin(SplitText, ScrambleTextPlugin);
if (reduced) document.documentElement.classList.add("reduced");

type Id = "intro" | "gate" | "where" | "judge" | "agents" | "ready" | "store";
const FLOW: Record<Ui["where"], Id[]> = {
  here: ["intro", "gate", "where", "judge", "agents", "ready"],
  server: ["intro", "gate", "where", "store"],
};

const ui: Ui = {
  where: q.get("role") === "developer" ? "server" : "here",
  judge: q.get("judge") === "jev" ? "jev" : "rules",
  key: "", picked: new Set(), started: false, store: { ...STORE }, pyAsked: false,
};
const ctx: Ctx = { ui, repaint: () => paint() };

// ---- The scenes ----
hydrateIcons();
$("#brand").innerHTML = `<img src="${logoUrl}" alt="" />WithHuman`;
$("#works").innerHTML = [["claude", "Claude Code"], ["openai", "Codex"], ["cursor", "Cursor"], ["feather", "Hermes"], ["shrimp", "OpenClaw"]]
  .map(([k, n]) => `<span>${ICON[k]}${n}</span>`).join("");
const stage = $("#stage");
const built: Partial<Record<Id, Scene>> = {};
const add = (id: Id, s: Scene) => { built[id] = s; stage.appendChild(s.el); };
add("where", whereScene(ctx));
add("judge", judgeScene(ctx));
add("agents", agentsScene(ctx));
add("ready", readyScene(ctx));
add("store", storeScene(ctx));
const el = (id: Id) => $<HTMLElement>(`[data-scene="${id}"]`, stage);
$$<HTMLElement>(".scene", stage).forEach((s) => gsap.set(s, { autoAlpha: 0 }));

const hero = el("intro");
const field = initField(hero);
const gate = initGate($(".field-gate", el("gate")));
initCursor();
const burst = initBurst();

// ---- The rules (Onboard.tsx's) ----
function derive(d: Data): Derived {
  const busy = d.run.state === "busy" || d.agents.some((a) => a.state === "busy") || d.jev.state === "busy";
  const chosen = d.agents.filter((a) => usable(a) && ui.picked.has(a.kind));
  const going = d.agents.filter((a) => chosen.includes(a) || a.state !== "idle");
  const jevReady = ui.judge === "rules" || d.jev.state === "ok" || (d.kept_key && ui.key.trim() === "");
  const needPython = !d.python && chosen.length > 0;
  // A hook that failed once everything stopped: 시작 can be pressed again.
  const failed = !busy && d.agents.some((a) => a.state === "err");
  const locked = (ui.started && !failed) || d.done;
  return { busy, chosen, going, jevReady, needPython, failed, locked };
}
const flow = () => FLOW[ui.where];
let at = 0;
const sceneId = () => flow()[Math.min(at, flow().length - 1)];
const canNext = (id: Id, x: Derived) => (id === "judge" ? x.jevReady && !x.busy : id !== "ready" && id !== "store");
const canBack = (id: Id, x: Derived) => at > 0 && !(id === "ready" && x.locked);

// ---- Moving between scenes ----
let shown: Id | null = null;
function show(next: number, dir: number) {
  at = Math.max(0, Math.min(flow().length - 1, next));
  const id = sceneId();
  if (id === shown) { paint(); return; }
  const was = shown; shown = id;
  document.body.dataset.scene = id;
  paint();
  if (was) sceneOut(el(was), dir);
  sceneIn(el(id), dir);
  field.setActive(id === "intro");
  gate.setActive(id === "gate");
  // A field to type into gets the focus once its scene is up.
  if (id === "store") setTimeout(() => $<HTMLInputElement>('[data-f="url"]', el("store"))?.focus(), calm ? 0 : 500);
}
const go = (n: number) => {
  const x = derive(current()), id = sceneId();
  if (n > 0 && !canNext(id, x)) return;
  if (n < 0 && !canBack(id, x)) return;
  show(at + n, n);
};

// ---- The bar: back, where you are, the scene's actions ----
const bar = $("#bar"), back = $<HTMLButtonElement>("#back"), dots = $("#dots"), acts = $("#acts");
back.addEventListener("click", () => go(-1));
dots.addEventListener("click", (e) => {
  const b = (e.target as HTMLElement).closest<HTMLElement>("[data-i]"); if (!b) return;
  const i = Number(b.dataset.i), x = derive(current());
  // Back to any scene passed; forward only one by one (each may need something first).
  if (i < at && canBack(sceneId(), x)) show(i, -1); else if (i === at + 1) go(1);
});
const btn = (cls: string, label: string, act: string, on = true, spin = false, icon = "") =>
  `<button class="btn ${cls} mag" data-act="${act}"${on ? "" : " disabled"}>${spin ? `<i class="spin"></i>` : ""}${label}${icon ? `<span class="ar">${ICON[icon]}</span>` : ""}</button>`;
acts.addEventListener("click", (e) => {
  const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-act]"); if (!b || b.disabled) return;
  const d = current(), x = derive(d);
  switch (b.dataset.act) {
    case "next": go(1); break;
    case "start":
      ui.started = true;
      send({ do: "start", jev: ui.judge === "jev", key: ui.judge === "jev" ? ui.key.trim() : "", model: d.model, agents: x.chosen.map((a) => a.kind) });
      paint();
      break;
    case "try": send({ do: "open", try: true }); break;
    case "open": send({ do: "open", try: false }); break;
    case "test": if (d.server.state !== "busy" && ui.store.url) send({ do: "server_test", ...ui.store }); break;
    case "save": send({ do: "server_save", ...ui.store }); break;
  }
});
let actsHtml = "", dotsHtml = "";
function paintBar(d: Data, x: Derived) {
  const id = sceneId(), f = flow();
  bar.classList.toggle("away", id === "intro");
  back.disabled = !canBack(id, x);
  const dh = f.map((s, i) => `<button class="dt${i === at ? " on" : ""}${i < at ? " past" : ""}" data-i="${i}" aria-label="${s}"></button>`).join("")
    + `<span class="cnt">${String(at + 1).padStart(2, "0")} / ${String(f.length).padStart(2, "0")}</span>`;
  if (dh !== dotsHtml) { dotsHtml = dh; dots.innerHTML = dh; }
  let html: string;
  if (id === "ready" && d.done) html = btn("sec", "기록 보기", "open") + btn("pri star", "체험", "try", true, false, "arrowRight");
  else if (id === "ready") html = btn("pri", "시작", "start", !x.busy && !x.needPython && !(ui.started && !x.failed), ui.started && !x.failed);
  else if (id === "store") html = btn("sec", "연결 확인", "test", d.server.state !== "busy" && !!ui.store.url, d.server.state === "busy" && d.server.sessions === null)
    + btn("pri", "열기", "save", d.server.state === "ok", d.server.state === "busy" && d.server.sessions !== null, "arrowRight");
  else html = btn("pri", "다음", "next", canNext(id, x), false, "arrowRight");
  if (html !== actsHtml) { actsHtml = html; acts.innerHTML = html; initMagnets(acts); }
  // The landing's progress bar, red to blue, as far as the setup has come.
  $("#progress").style.transform = `scaleX(${f.length > 1 ? at / (f.length - 1) : 0})`;
}

// ---- Painting the data ----
function paint() {
  const d = current(), x = derive(d);
  for (const s of Object.values(built)) s!.update(d, x);
  paintBar(d, x);
}

let seeded = false, wasDone = false;
const folders = new Map<string, string | null>();
onData((d, first) => {
  // The app's first data: a developer's machine starts on the company's engine, a kept key on Jev,
  // the agents found (not per folder) are picked, and a run already going is shown as started.
  if (first && !seeded) {
    seeded = true;
    if (d.role === "developer") ui.where = "server";
    if (d.kept_key) ui.judge = "jev";
    ui.picked = new Set(d.agents.filter((a) => a.found && !a.per_folder).map((a) => a.kind));
    ui.started = (d.run.state !== "idle" && d.run.state !== "err") || d.done;
  }
  // A folder just picked for an agent picks that agent.
  for (const a of d.agents) {
    const was = folders.get(a.kind);
    if (a.folder && a.folder !== was && was !== undefined) ui.picked.add(a.kind);
    folders.set(a.kind, a.folder);
  }
  if (d.run.state === "err") ui.started = false;
  if (d.python) ui.pyAsked = false;
  if (at >= flow().length) at = flow().length - 1;
  if (shown) paint();
  // Everything up: the landing's star burst from 체험.
  if (d.done && !wasDone && shown === "ready") requestAnimationFrame(() => {
    const b = $('[data-act="try"]', acts); if (!b) return;
    const r = b.getBoundingClientRect(); burst(r.left + r.width / 2, r.top + r.height / 2);
  });
  wasDone = d.done;
});

// ---- Input: the intro's 시작, the window's bare top, the wheel, the keys ----
$$<HTMLElement>("[data-go=next]").forEach((b) => b.addEventListener("click", () => go(1)));
initMagnets();
// The page fills the window, which has no title bar: its bare top moves the window, a double press zooms it.
const drag = $("#drag");
drag.addEventListener("mousedown", (e) => { if (e.button === 0 && e.detail < 2) send({ do: "drag" }); });
drag.addEventListener("dblclick", () => send({ do: "zoom" }));
// One wheel gesture moves one whole scene (the landing's section snap): a trackpad keeps sending
// wheel events after the fingers lift; those belong to the same gesture.
{
  const QUIET = 220;
  let armed = true, last = 0, moving = 0;
  addEventListener("wheel", (e) => {
    if (e.ctrlKey) return;
    e.preventDefault();
    const now = performance.now(), gap = now - last; last = now;
    if (!armed && now > moving && gap > QUIET) armed = true;
    const dy = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    if (!armed || Math.abs(dy) < 3) return;
    armed = false; moving = now + (calm ? 0 : 700);
    go(dy > 0 ? 1 : -1);
  }, { passive: false });
}
addEventListener("keydown", (e) => {
  const t = e.target as HTMLElement;
  if (t.closest("input, textarea, select")) return;
  if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === "PageDown") { e.preventDefault(); go(1); }
  if (e.key === "ArrowLeft" || e.key === "ArrowUp" || e.key === "PageUp") { e.preventDefault(); go(-1); }
  if (e.key === "Enter" && !t.closest("button")) {
    const b = $<HTMLButtonElement>("[data-act]:not([disabled]):last-child", acts);
    if (sceneId() === "intro") go(1); else if (b && b.dataset.act === "next") go(1);
  }
});

// ---- Start ----
startDemo();
const asked = q.get("scene") as Id | null;
const startAt = asked && flow().includes(asked) ? flow().indexOf(asked) : 0;
document.fonts.ready.then(() => {
  show(startAt, 1);
  introAndHeadline(hero, startAt === 0, () => { if (!calm) setTimeout(field.ping, 200); });
  if (startAt !== 0) { $("#intro").style.display = "none"; }
});
