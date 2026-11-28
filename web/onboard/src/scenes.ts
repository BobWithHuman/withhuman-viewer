// The setup's scenes, built from the landing's parts: its kick + split heading, its white cards with
// the soft shadow, its big buttons, its chips with the pinging dot (the hero badge's), its spinner
// (the demo's), the agents' marks (icons.ts) and blue for "you" / red for "not you".
// Each scene is built once; `update` puts the latest data in place, so typed fields keep their text.

import { ICON } from "./icons";
import { esc, h } from "./lib";
import { mark } from "./data";
import { ENGINE, STORE, scope, send, type Agent, type Data, type State } from "./state";

export type Where = "here" | "server";
export type Judge = "rules" | "jev";
export type Ui = {
  where: Where; judge: Judge; key: string; picked: Set<string>; started: boolean; store: typeof STORE; pyAsked: boolean;
};
export type Derived = {
  busy: boolean; chosen: Agent[]; going: Agent[]; jevReady: boolean; needPython: boolean; failed: boolean; locked: boolean;
};
export type Ctx = { ui: Ui; repaint: () => void };
export type Scene = { el: HTMLElement; update: (d: Data, x: Derived) => void };

export const usable = (a: Agent) => a.found && (!a.per_folder || !!a.folder);

const head = (n: string, kick: string, title: string, lead = "") => `
  <div class="head"><span class="kick"><b>${n}</b> ${kick}</span><h2 class="split">${title}</h2>${lead ? `<p class="lead">${lead}</p>` : ""}</div>`;

/** A state as the landing shows one: a dot (pinging when up), the demo's spinner, a red ban. */
export const stateMark = (s: State) =>
  s === "busy" ? `<i class="spin"></i>` : s === "ok" ? `<span class="okm">${ICON.check}</span>` : s === "err" ? `<span class="errm">${ICON.ban}</span>` : `<i class="dot"></i>`;
const chip = (cls: string, html: string) => `<span class="chip ${cls}">${html}</span>`;
const logo = (kind: string) => `<span class="logo">${ICON[mark(kind)]}</span>`;

/** The engine's parts as chips with the badge's dot: grey while down, green and pinging when up. */
const parts = (d: Data) =>
  (d.parts.length ? d.parts : ["PostgreSQL", "Decision", "Generator", "Collector"].map((name) => ({ name, up: false })))
    .map((p) => `<span class="part${p.up ? " up" : ""}"><i></i>${esc(p.name)}</span>`).join("");

// ---- 02 엔진: this Mac or the company's ----
export function whereScene(c: Ctx): Scene {
  const el = h(`<section class="scene" data-scene="where">
    ${head("02", "엔진", '엔진은 <span class="b">어디서</span> 돌까?')}
    <div class="choices rv">
      <button class="choice" data-v="here">
        <span class="tile" data-ic="laptop"></span>
        <span class="c-t">이 Mac ${chip("blue", "권장")}</span>
        <span class="c-s mono">${ENGINE}</span>
        <span class="parts" data-parts></span>
        <span class="tick">${ICON.check}</span>
      </button>
      <button class="choice" data-v="server">
        <span class="tile" data-ic="network"></span>
        <span class="c-t">회사 엔진</span>
        <span class="c-s mono">PostgreSQL · 읽기 전용</span>
        <span class="parts"><span class="part"><i></i>훅 엔진</span><span class="part"><i></i>WithHuman 키</span></span>
        <span class="tick">${ICON.check}</span>
      </button>
    </div>
  </section>`);
  el.querySelectorAll<HTMLElement>(".choice").forEach((b) => b.addEventListener("click", () => { c.ui.where = b.dataset.v as Where; c.repaint(); }));
  const ps = el.querySelector<HTMLElement>("[data-parts]")!;
  return {
    el,
    update: (d) => {
      el.querySelectorAll<HTMLElement>(".choice").forEach((b) => b.classList.toggle("sel", b.dataset.v === c.ui.where));
      const html = parts(d); if (ps.innerHTML !== html) ps.innerHTML = html;
    },
  };
}

// ---- 03 판정: rules, or Jev with its key checked and a model picked ----
export function judgeScene(c: Ctx): Scene {
  const el = h(`<section class="scene" data-scene="judge">
    ${head("03", "판정", '판정은 <span class="b">무엇으로</span>?')}
    <div class="choices compact rv">
      <button class="choice" data-v="rules">
        <span class="tile" data-ic="list"></span>
        <span class="c-t">규칙 ${chip("", "키 없음")}</span>
        <span class="c-s">이 Mac 안에서만</span>
        <span class="tick">${ICON.check}</span>
      </button>
      <button class="choice" data-v="jev">
        <span class="tile" data-ic="sparkles"></span>
        <span class="c-t">Jev ${chip("blue", "원인 추적")}</span>
        <span class="c-s mono">api.typesafe.ai</span>
        <span class="tick">${ICON.check}</span>
      </button>
    </div>
    <div class="panel" data-jev>
      <div class="row">
        <span class="lab">키</span>
        <label class="fieldbox"><span data-ic="key"></span><input class="inp" type="password" autocomplete="off" spellcheck="false" data-key /></label>
        <button class="btn sec md mag" data-check>확인</button>
        <span class="st" data-st></span>
      </div>
      <div class="row">
        <span class="lab">모델</span>
        <div class="models" data-models></div>
      </div>
    </div>
  </section>`);
  const panel = el.querySelector<HTMLElement>("[data-jev]")!, input = el.querySelector<HTMLInputElement>("[data-key]")!;
  const checkBtn = el.querySelector<HTMLButtonElement>("[data-check]")!, st = el.querySelector<HTMLElement>("[data-st]")!, models = el.querySelector<HTMLElement>("[data-models]")!;
  let busyNow = false;
  const check = () => { const k = c.ui.key.trim(); if (k && !busyNow) send({ do: "check_jev", key: k }); };
  el.querySelectorAll<HTMLElement>(".choice").forEach((b) => b.addEventListener("click", () => { c.ui.judge = b.dataset.v as Judge; c.repaint(); if (c.ui.judge === "jev") setTimeout(() => input.focus(), 50); }));
  input.addEventListener("input", () => { c.ui.key = input.value; c.repaint(); });
  input.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); check(); } });
  checkBtn.addEventListener("click", check);
  models.addEventListener("click", (e) => { const b = (e.target as HTMLElement).closest<HTMLElement>("[data-m]"); if (b && !b.hasAttribute("disabled")) send({ do: "model", model: b.dataset.m }); });
  return {
    el,
    update: (d, x) => {
      busyNow = x.busy;
      el.querySelectorAll<HTMLElement>(".choice").forEach((b) => b.classList.toggle("sel", b.dataset.v === c.ui.judge));
      panel.classList.toggle("shut", c.ui.judge !== "jev");
      input.placeholder = d.kept_key ? "저장된 키" : "jev_…";
      input.disabled = x.busy;
      checkBtn.disabled = x.busy || !c.ui.key.trim();
      checkBtn.innerHTML = (d.jev.state === "busy" ? `<i class="spin"></i>` : "") + "확인";
      st.className = "st " + d.jev.state;
      st.innerHTML = d.jev.state === "ok" ? chip("ok", `<i></i>${esc(d.jev.text ?? "확인됨")}`)
        : d.jev.state === "err" ? chip("err", `${ICON.ban}${esc(d.jev.text ?? "실패")}`)
        : d.jev.state === "busy" ? chip("", "확인 중")
        : d.kept_key ? chip("", `${ICON.key}키 저장됨`) : "";
      const list = d.jev.models.length ? d.jev.models : [d.model];
      const html = list.map((m) => `<button class="mchip${m === d.model ? " on" : ""}" data-m="${esc(m)}"${d.jev.models.length ? "" : " disabled"}>${esc(m)}</button>`).join("");
      if (models.innerHTML !== html) models.innerHTML = html;
    },
  };
}

// ---- 04 에이전트: each with its mark, found or not, a folder for some; Python missing ----
export function agentsScene(c: Ctx): Scene {
  const el = h(`<section class="scene" data-scene="agents">
    ${head("04", "에이전트", '지킬 <span class="b">에이전트</span>')}
    <div class="py rv" data-py>
      <span class="tile" data-ic="terminal"></span>
      <span class="c-t">Python ${chip("err", "없음")}</span>
      <button class="btn sec md mag" data-get><span data-ic="download"></span>설치</button>
    </div>
    <div class="agents rv" data-list></div>
  </section>`);
  const list = el.querySelector<HTMLElement>("[data-list]")!, py = el.querySelector<HTMLElement>("[data-py]")!, get = el.querySelector<HTMLButtonElement>("[data-get]")!;
  get.addEventListener("click", () => { c.ui.pyAsked = true; send({ do: "python" }); c.repaint(); });
  let sig = "", locked = false;
  list.addEventListener("click", (e) => {
    const t = e.target as HTMLElement, card = t.closest<HTMLElement>("[data-k]"); if (!card || locked) return;
    const kind = card.dataset.k!;
    if (t.closest("[data-pick]")) { send({ do: "pick", agent: kind }); return; }
    if (card.classList.contains("can")) { const p = c.ui.picked; if (p.has(kind)) p.delete(kind); else p.add(kind); c.repaint(); }
  });
  return {
    el,
    update: (d, x) => {
      locked = x.busy || c.ui.started;
      py.classList.toggle("shut", !x.needPython);
      get.disabled = c.ui.pyAsked && !d.python;
      get.innerHTML = c.ui.pyAsked && !d.python ? `<i class="spin"></i>설치 중` : `${ICON.download}설치`;
      const rows = [...d.agents].sort((a, b) => Number(b.found) - Number(a.found));
      const s = rows.map((a) => a.kind).join(",");
      if (s !== sig) {
        sig = s;
        list.innerHTML = rows.map((a) => `<div class="agent" data-k="${esc(a.kind)}">
          ${logo(a.kind)}<span class="a-n">${esc(a.name)}</span><span class="a-s" data-sub></span>
          <span class="a-r"><span data-state></span><button class="btn sec xs" data-pick>${ICON.folder}<b data-pl>폴더</b></button><span class="tick">${ICON.check}</span></span>
        </div>`).join("");
      }
      for (const a of rows) {
        const card = list.querySelector<HTMLElement>(`[data-k="${CSS.escape(a.kind)}"]`)!;
        const on = usable(a) && c.ui.picked.has(a.kind);
        card.classList.toggle("sel", on);
        card.classList.toggle("off", !a.found);
        card.classList.toggle("can", usable(a) && !locked);
        const sub = card.querySelector<HTMLElement>("[data-sub]")!;
        sub.textContent = !a.found ? "없음" : a.per_folder ? (a.folder ?? "폴더별 · 폴더 선택") : scope(a);
        sub.classList.toggle("mono", !!a.folder);
        const pick = card.querySelector<HTMLButtonElement>("[data-pick]")!;
        pick.hidden = !(a.found && a.per_folder);
        pick.disabled = locked;
        card.querySelector<HTMLElement>("[data-pl]")!.textContent = a.folder ? "변경" : "폴더";
        const stEl = card.querySelector<HTMLElement>("[data-state]")!;
        const sh = a.state === "idle" ? "" : stateMark(a.state);
        if (stEl.innerHTML !== sh) stEl.innerHTML = sh;
        stEl.title = a.text ?? "";
      }
    },
  };
}

// ---- 05 시작: the engine's parts and each agent coming up, then 체험 / 기록 ----
export function readyScene(c: Ctx): Scene {
  const el = h(`<section class="scene" data-scene="ready">
    <div class="head" data-head><span class="kick"><b>05</b> 시작</span><h2 class="split"><span class="t-go">시키는 건 <span class="b">당신</span>뿐.</span><span class="t-done">준비 <span class="b">완료</span>.</span></h2></div>
    <div class="pipe rv">
      <div class="rail"><i data-fill></i></div>
      <div class="steps" data-steps></div>
    </div>
  </section>`);
  // Both headings are in the h2 from the start (its split restores what it found); the head picks one.
  const steps = el.querySelector<HTMLElement>("[data-steps]")!, fill = el.querySelector<HTMLElement>("[data-fill]")!, head = el.querySelector<HTMLElement>("[data-head]")!;
  steps.addEventListener("click", (e) => { if ((e.target as HTMLElement).closest("[data-py]")) { c.ui.pyAsked = true; send({ do: "python" }); c.repaint(); } });
  return {
    el,
    update: (d, x) => {
      head.classList.toggle("done", d.done);
      const up = d.parts.filter((p) => p.up).length, total = d.parts.length || 4;
      const engine: State = d.run.state === "err" ? "err" : up === total && d.run.state === "ok" ? "ok" : c.ui.started ? "busy" : "idle";
      const row = (st: State, icon: string, name: string, detail: string, extra = "", mono = true) => `<div class="step s-${st}">
        <span class="mk">${stateMark(st)}</span>${icon}<span class="s-n">${name}</span><span class="s-d${mono ? " mono" : ""}">${detail}</span>${extra}</div>`;
      const rows = [
        row(engine, `<span class="logo">${ICON.laptop}</span>`, "엔진", d.run.state === "err" ? esc(d.run.text ?? "") : `${ENGINE} · ${up}/${total}`, `<span class="parts">${parts(d)}</span>`),
        row(c.ui.judge === "jev" ? "ok" : "ok", `<span class="logo">${ICON[c.ui.judge === "jev" ? "sparkles" : "list"]}</span>`, "판정", c.ui.judge === "jev" ? `Jev · ${esc(d.model)}` : "규칙"),
        row(d.python ? "ok" : x.chosen.length ? "err" : "idle", `<span class="logo">${ICON.terminal}</span>`, "Python", esc(d.python ?? "없음"),
          !d.python && x.chosen.length ? `<button class="btn sec xs" data-py${c.ui.pyAsked ? " disabled" : ""}>${c.ui.pyAsked ? `<i class="spin"></i>` : ICON.download}설치</button>` : ""),
        ...x.going.map((a) => {
          const st: State = a.state === "idle" && !c.ui.started ? "idle" : a.state === "idle" ? "busy" : a.state;
          return row(st, logo(a.kind), esc(a.name), a.state === "err" ? esc(a.text ?? "") : esc(a.folder ?? scope(a)), "", !!a.folder || a.state === "err");
        }),
      ];
      if (!x.going.length) rows.push(row("idle", `<span class="logo">${ICON.bot}</span>`, "에이전트", "없음", "", false));
      const html = rows.join("");
      if (steps.innerHTML !== html) steps.innerHTML = html;
      // The rail fills as the engine's parts and the agents come up.
      const work = total + x.going.length, doneN = up + x.going.filter((a) => a.state === "ok" || a.state === "err").length;
      fill.style.transform = `scaleY(${work ? doneN / work : 0})`;
    },
  };
}

// ---- 02 회사 엔진: the store's form, 연결 확인, 열기 ----
export function storeScene(c: Ctx): Scene {
  const f = (k: keyof typeof STORE, label: string, o: { mono?: boolean; secure?: boolean; ph?: string; wide?: boolean } = {}) =>
    `<label class="fl${o.wide ? " wide" : ""}"><span class="lab">${label}</span><input class="inp${o.mono ? " mono" : ""}" data-f="${k}" type="${o.secure ? "password" : "text"}" autocomplete="off" spellcheck="false" placeholder="${o.ph ?? ""}" /></label>`;
  const el = h(`<section class="scene" data-scene="store">
    ${head("02", "회사 엔진", '회사 <span class="b">엔진</span>에 연결')}
    <div class="form rv">
      <div class="group"><span class="g-h"><span data-ic="database"></span>PostgreSQL</span>
        <div class="grid">${f("url", "주소", { mono: true, wide: true })}${f("user", "사용자")}${f("database", "데이터베이스")}${f("password", "비밀번호", { secure: true, wide: true })}</div></div>
      <div class="group"><span class="g-h"><span data-ic="plug"></span>훅</span>
        <div class="grid one">${f("engine", "엔진 주소", { mono: true, ph: "https://engine.example" })}${f("key", "WithHuman 키", { mono: true, secure: true, ph: "whk_…" })}</div>
        <div class="conn" data-conn></div></div>
    </div>
  </section>`);
  el.querySelectorAll<HTMLInputElement>("[data-f]").forEach((i) => {
    const k = i.dataset.f as keyof typeof STORE; i.value = c.ui.store[k];
    i.addEventListener("input", () => { c.ui.store = { ...c.ui.store, [k]: i.value }; c.repaint(); });
  });
  const conn = el.querySelector<HTMLElement>("[data-conn]")!;
  return {
    el,
    update: (d) => {
      const s = d.server;
      const html = `<span class="conn-mk">${stateMark(s.state)}</span><span class="conn-n">연결</span>` + (
        s.state === "ok" ? chip("ok", `<i></i>세션 ${s.sessions ?? "—"}`) : s.state === "err" ? chip("err", `${ICON.ban}${esc(s.text ?? "실패")}`) : s.state === "busy" ? chip("", "확인 중") : chip("", "확인 전"));
      if (conn.innerHTML !== html) conn.innerHTML = html;
      el.querySelectorAll<HTMLInputElement>("[data-f]").forEach((i) => (i.disabled = s.state === "busy"));
    },
  };
}
