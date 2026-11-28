// The landing's "who asked?" diagram (site/src/gate.ts), as is but for its Korean labels and its start:
// the agent's calls fly as chips from where they came from toward WithHuman, a field of light.
// Calls you asked for pass through and land in the terminal on the right; the rest hit the field, bounce off
// and tumble onto the floor, where matter.js piles them like real things. Click a source to fire calls from it;
// point at any chip to see where it came from. It runs while its scene is on (`setActive`).
import gsap from "gsap";
import Matter from "matter-js";
import { $, esc, fine, fitCanvas, reduced, still } from "./lib";
import { SRC, type Key, type Src } from "./data";
import logoUrl from "./favicon.svg";
const BLUE = "#1f6bff", RED = "#ef3b2c", GREEN = "#12a150", INK = "#121212";
const MONO = '500 12px "Geist Mono", ui-monospace, monospace';
// Blue is yours (and WithHuman's); everything else is ink, paper and the rubble's own materials.
const OTHER = "#3d3a35";
const srcInk = (s: Src) => (s.id === "you" ? BLUE : OTHER);
// The shards are the chip's own materials: mostly white, some paper, now and then the terminal's ink.
const SHARD = () => { const r = Math.random(); return r < .66 ? "#ffffff" : r < .88 ? "#f1ede5" : "#1c1d21"; };
const DARK = "#1c1d21";

type Chip = {
  s: Src; text: string; w: number; state: "fly" | "pass" | "gone";
  t: number; x: number; y: number; a: number; born: number; lut?: number[][]; len?: number;
};
// A stopped call shatters on the field: its words on broken shards of the chip, its broken edges
// marked red, with small splinters. They fall and pile up as bodies.
type Piece = {
  s: Src; text: string; label: string; kind: "chunk"; fill: string; seed: number;
  verts: { x: number; y: number }[]; w: number; h: number; fs: number; body: Matter.Body; born: number; fade?: number; a: number;
};

export function initGate(host: HTMLElement) {
  const cv = $<HTMLCanvasElement>("canvas", host), tip = $(".gtip", host);
  const logo = new Image(); logo.src = logoUrl;
  let ctx: CanvasRenderingContext2D, W = 0, H = 0;
  let GX = 0, GY = 0, FT = 0, FB = 0, FLOOR = 0, TX = 0, TY = 0, TW = 0, TH = 0, SX = 0;
  const CH = 26;
  const layout = () => {
    ({ ctx, w: W, h: H } = fitCanvas(cv));
    SX = 0; GX = Math.round(W * .56); GY = Math.round(H * .46);
    // (the landing: .1 and .74; here the name above the field needs room in a short canvas)
    FT = H * .2; FB = H * .78; FLOOR = H - 10;
    TW = Math.min(340, W * .3); TH = Math.min(230, H * .46); TX = W - TW; TY = GY - TH / 2;
    SRC.forEach((s, i) => (s.y = H * (.16 + i * .135)));
    setWalls();
  };
  const bez = (p: number[][], t: number) => { const u = 1 - t; return [u * u * u * p[0][0] + 3 * u * u * t * p[1][0] + 3 * u * t * t * p[2][0] + t * t * t * p[3][0], u * u * u * p[0][1] + 3 * u * u * t * p[1][1] + 3 * u * t * t * p[2][1] + t * t * t * p[3][1]]; };
  const inPath = (s: Src, w = 0) => { const x0 = SX + 200, x1 = GX - w / 2 - 6; return [[x0, s.y], [x0 + (x1 - x0) * .5, s.y], [x1 - (x1 - x0) * .4, GY], [x1, GY]]; };
  const outPath = (w: number, slot: number) => { const x0 = GX + w / 2 + 6, x1 = TX + 18 + w / 2, y1 = TY + 52 + slot * 24; return [[x0, GY], [x0 + 80, GY], [x1 - 80, y1], [x1, y1]]; };

  let chips: Chip[] = [], pieces: Piece[] = [], ripples: { y: number; t: number; c: string; amp: number }[] = [], sparks: { x: number; y: number; vx: number; vy: number; a: number }[] = [];
  const log: { text: string; t: number }[] = [];
  let ran = 0, seq = 0, grow = 0, mx = -1, my = -1, held: Chip | null = null, heldPiece: Piece | null = null, hoverSrc: Src | null = null;

  const measure = (s: string) => { ctx.font = MONO; return Math.ceil(ctx.measureText(s).width) + 30; };
  const fire = (s: Src) => {
    const text = s.calls[seq++ % s.calls.length];
    chips.push({ s, text, w: measure(text), state: "fly", t: 0, x: 0, y: 0, a: 1, born: performance.now() });
  };
  // The floor the stopped calls land on: a matter.js world with a floor, the sources' edge and the field as walls.
  const engine = Matter.Engine.create({ gravity: { x: 0, y: 1.1 }, enableSleeping: true });
  let walls: Matter.Body[] = [];
  function setWalls() {
    Matter.Composite.remove(engine.world, walls);
    const o = { isStatic: true, friction: .8 };
    walls = [
      Matter.Bodies.rectangle(W / 2, FLOOR + 40, W * 2, 80, o),
      Matter.Bodies.rectangle(SX + 200 - 40, H / 2, 80, H * 3, o),
      Matter.Bodies.rectangle(GX - 4 + 40, H / 2, 80, H * 3, o),
    ];
    Matter.Composite.add(engine.world, walls);
  }
  const MAX_PIECES = 30, PIECE_LIFE = 9000;
  const stencil = (fs: number) => `500 ${fs}px "Geist Mono", ui-monospace, monospace`;
  // A rectangle with its corners knocked off at random: a convex broken chunk.
  const broken = (w: number, h: number) => {
    const hw = w / 2, hh = h / 2, m = Math.min(w, h), c = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]], pts: { x: number; y: number }[] = [];
    c.forEach(([x, y], i) => {
      if (Math.random() < .2) { pts.push({ x, y }); return; }
      const [px, py] = c[(i + 3) % 4], [nx, ny] = c[(i + 1) % 4], a = m * (.1 + Math.random() * .38), b = m * (.1 + Math.random() * .38);
      const lp = Math.hypot(px - x, py - y), ln = Math.hypot(nx - x, ny - y);
      pts.push({ x: x + (px - x) / lp * a, y: y + (py - y) / lp * a }, { x: x + (nx - x) / ln * b, y: y + (ny - y) / ln * b });
    });
    return pts;
  };
  const shatter = (c: Chip, x: number, y: number) => {
    // The whole command stays on one shard: split into words, a pile of "git" and "npm" would read
    // as if those were blocked, when it is this call, from that source, that was.
    const now = performance.now(), parts = [c.text];
    const made: Piece[] = [];
    const add = (p: Pick<Piece, "label" | "kind" | "fill" | "w" | "h" | "fs">, body: Matter.Body) =>
      made.push({ ...p, s: c.s, text: c.text, seed: Math.random() * 1e6, body, born: now, a: 1,
        verts: body.vertices.map((v) => ({ x: v.x - body.position.x, y: v.y - body.position.y })) });
    const stone = { restitution: .15, friction: .9, frictionAir: .012, density: .0022 };
    parts.forEach((label, i) => {
      const sc = .9 + Math.random() * .3, fs = Math.round(10.5 * sc + 1), h = Math.round(16 * sc + 10);
      ctx.font = stencil(fs); const w = Math.ceil(ctx.measureText(label).width) + 20 + Math.random() * 14;
      const body = Matter.Bodies.fromVertices(Math.min(x - i * 8, GX - 50 - w / 2), y + (i - 1) * 12, [broken(w, h)], stone);
      add({ label, kind: "chunk", fill: SHARD(), w, h, fs }, body);
    });
    for (let i = 0, n = 3 + Math.floor(Math.random() * 3); i < n; i++) {
      const h = 6 + Math.random() * 12, w = h * (1 + Math.random() * 1.4);
      add({ label: "", kind: "chunk", fill: SHARD(), w, h, fs: 0 }, Matter.Bodies.fromVertices(Math.min(x, GX - 50 - w), y - 10, [broken(w, h)], stone));
    }
    for (const p of made) {
      Matter.Body.setVelocity(p.body, { x: -(2.5 + Math.random() * 5), y: -(1.5 + Math.random() * 5.5) });
      Matter.Body.setAngularVelocity(p.body, (Math.random() - .5) * .3);
      Matter.Composite.add(engine.world, p.body);
    }
    pieces.push(...made);
    // the oldest on the floor fade away so the pile stays small
    const live = pieces.filter((p) => p.fade === undefined);
    for (let i = 0; i < live.length - MAX_PIECES; i++) live[i].fade = now;
  };
  const fieldX = (y: number, now: number) => {
    let d = 0;
    for (const r of ripples) { const age = (now - r.t) / 1000, dy = Math.abs(y - r.y); d += r.amp * Math.exp(-(dy * dy) / 2600) * Math.sin(age * 22 - dy * .09) * Math.exp(-age * 3.2); }
    return GX + d;
  };

  // Points along a curve by distance, so a chip moves at one speed from end to end.
  const table = (p: number[][]) => { const pts: number[][] = []; let d = 0, prev = bez(p, 0); for (let i = 0; i <= 48; i++) { const q = bez(p, i / 48); d += Math.hypot(q[0] - prev[0], q[1] - prev[1]); pts.push([q[0], q[1], d]); prev = q; } return pts; };
  const at = (lut: number[][], f: number) => {
    const d = Math.min(1, Math.max(0, f)) * lut[lut.length - 1][2];
    let i = 1; while (i < lut.length - 1 && lut[i][2] < d) i++;
    const a = lut[i - 1], b = lut[i], k = b[2] > a[2] ? (d - a[2]) / (b[2] - a[2]) : 0;
    return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
  };
  const roundRect = (x: number, y: number, w: number, h: number, r: number) => { ctx.beginPath(); ctx.roundRect(x, y, w, h, r); };
  // The sources as the things they are, small and a little askew on the page: your message, a
  // README, a web page, a tool's JSON, the agent's own thought.
  const TILT: Record<Key, number> = { you: -.05, readme: .04, web: -.03, mcp: .05, own: -.03 };
  const line = (x: CanvasRenderingContext2D, x0: number, y: number, x1: number, c: string, w = 2) => { x.beginPath(); x.moveTo(x0, y); x.lineTo(x1, y); x.strokeStyle = c; x.lineWidth = w; x.lineCap = "round"; x.stroke(); };
  const PAPER = "#fffdf8", EDGE = "#d9d3c7", RULE = "#d3cdc1";
  const ARTIFACT: Record<Key, (x: CanvasRenderingContext2D, on: boolean) => void> = {
    you: (x) => {
      x.beginPath(); x.roundRect(-28, -19, 56, 34, 11); x.moveTo(-17, 14); x.lineTo(-23, 23); x.lineTo(-7, 14); x.fillStyle = BLUE; x.fill(); x.shadowColor = "transparent";
      line(x, -17, -7, 15, "rgba(255,255,255,.95)", 3); line(x, -17, 3, 5, "rgba(255,255,255,.7)", 3);
    },
    readme: (x, on) => {
      x.beginPath(); x.moveTo(-20, -24); x.lineTo(11, -24); x.lineTo(20, -15); x.lineTo(20, 24); x.lineTo(-20, 24); x.closePath(); x.fillStyle = PAPER; x.fill(); x.shadowColor = "transparent";
      x.strokeStyle = on ? OTHER : EDGE; x.lineWidth = 1; x.stroke();
      x.beginPath(); x.moveTo(11, -24); x.lineTo(11, -15); x.lineTo(20, -15); x.fillStyle = "#ece7dc"; x.fill(); x.stroke();
      line(x, -13, -14, 2, OTHER, 2.6); [[-4, 12], [3, 9], [10, 13], [17, 6]].forEach(([y, l]) => line(x, -13, y, -13 + l * 1.9, RULE, 2));
    },
    web: (x, on) => {
      x.beginPath(); x.roundRect(-28, -21, 56, 42, 6); x.fillStyle = PAPER; x.fill(); x.shadowColor = "transparent"; x.strokeStyle = on ? OTHER : EDGE; x.lineWidth = 1; x.stroke();
      x.save(); x.clip(); x.fillStyle = "#eeeae2"; x.fillRect(-28, -21, 56, 11); x.restore();
      [-22, -17, -12].forEach((cx) => { x.beginPath(); x.arc(cx, -15.5, 1.7, 0, Math.PI * 2); x.fillStyle = "#bdb6a8"; x.fill(); });
      x.beginPath(); x.roundRect(-6, -18.5, 29, 6, 3); x.fillStyle = PAPER; x.fill();
      x.fillStyle = "#e3ded3"; x.fillRect(-22, -4, 18, 19); [[-1, 22], [5, 17], [11, 20]].forEach(([y, e]) => line(x, 1, y, e, RULE, 2));
    },
    mcp: (x) => {
      x.beginPath(); x.roundRect(-28, -20, 56, 40, 6); x.fillStyle = "#1c1d21"; x.fill(); x.shadowColor = "transparent";
      x.font = '600 11px "Geist Mono", monospace'; x.fillStyle = "#8a909b"; x.textBaseline = "middle"; x.fillText("{", -23, -9); x.fillText("}", -23, 11);
      line(x, -14, -4, 4, "#5c616c", 2); line(x, 7, -4, 18, "#8fb89a", 2); line(x, -14, 3, 10, "#5c616c", 2); line(x, -14, 10, -2, "#5c616c", 2);
    },
    own: (x, on) => {
      x.beginPath(); x.ellipse(2, -4, 26, 16, 0, 0, Math.PI * 2); x.fillStyle = PAPER; x.fill(); x.shadowColor = "transparent";
      x.setLineDash([3, 3]); x.strokeStyle = on ? OTHER : "#a29b8d"; x.lineWidth = 1.4; x.stroke();
      for (const [cx, cy, r] of [[-17, 16, 4], [-24, 23, 2.4]]) { x.beginPath(); x.arc(cx, cy, r, 0, Math.PI * 2); x.fillStyle = PAPER; x.fill(); x.stroke(); }
      x.setLineDash([]); [-7, 2, 11].forEach((cx) => { x.beginPath(); x.arc(cx, -4, 2, 0, Math.PI * 2); x.fillStyle = OTHER; x.fill(); });
    },
  };
  const sprites = new Map<string, HTMLCanvasElement>();
  const sprite = (c: Chip, kind: "fly" | "pass" | "held") => {
    const key = c.text + "|" + kind + "|" + c.s.id;
    let sp = sprites.get(key); if (sp) return sp;
    const dpr = Math.min(2, devicePixelRatio || 1), pad = 14, col = srcInk(c.s);
    sp = document.createElement("canvas"); sp.width = (c.w + pad * 2) * dpr; sp.height = (CH + pad * 2) * dpr;
    const x = sp.getContext("2d")!; x.scale(dpr, dpr); x.translate(pad + c.w / 2, pad + CH / 2);
    const good = kind === "pass", hl = kind === "held";
    x.shadowColor = "rgba(20,18,15,.18)"; x.shadowBlur = hl ? 16 : 9; x.shadowOffsetY = 3;
    x.beginPath(); x.roundRect(-c.w / 2, -CH / 2, c.w, CH, 8); x.fillStyle = "#fff"; x.fill(); x.shadowColor = "transparent";
    x.strokeStyle = good ? "rgba(18,161,80,.7)" : hl ? col : "rgba(20,18,15,.12)"; x.lineWidth = hl || good ? 1.6 : 1; x.stroke();
    x.beginPath(); x.arc(-c.w / 2 + 12, 0, 3.5, 0, Math.PI * 2); x.fillStyle = good ? GREEN : col; x.fill();
    x.font = MONO; x.textBaseline = "middle"; x.fillStyle = INK; x.fillText(c.text, -c.w / 2 + 21, .5);
    sprites.set(key, sp); return sp;
  };
  const drawChip = (c: Chip) => {
    const kind = held === c ? "held" : c.state === "pass" ? "pass" : "fly";
    const sp = sprite(c, kind), w = c.w + 28, h = CH + 28;
    ctx.save(); ctx.globalAlpha = c.a * (hoverSrc && hoverSrc !== c.s ? .25 : 1); ctx.translate(c.x, c.y);
    ctx.drawImage(sp, -w / 2, -h / 2, w, h); ctx.restore();
  };
  const pieceSprite = (p: Piece) => {
    const key = `p|${p.kind}|${p.label}|${p.fs}|${p.fill}|${Math.round(p.seed)}`;
    let sp = sprites.get(key); if (sp) return sp;
    const dpr = Math.min(2, devicePixelRatio || 1), pad = 10, ext = Math.max(...p.verts.map((v) => Math.max(Math.abs(v.x), Math.abs(v.y)))) + pad;
    const half = p.kind === "chunk" ? ext : Math.max(p.w, p.h) / 2 + pad;
    sp = document.createElement("canvas"); sp.width = sp.height = Math.ceil(half * 2 * dpr);
    const x = sp.getContext("2d")!; x.scale(dpr, dpr); x.translate(half, half);
        x.shadowColor = "rgba(20,18,15,.16)"; x.shadowBlur = 9; x.shadowOffsetY = 3;
    const v = p.verts, dark = p.fill === DARK;
    x.beginPath(); x.moveTo(v[0].x, v[0].y); v.slice(1).forEach((q) => x.lineTo(q.x, q.y)); x.closePath();
    x.fillStyle = p.fill; x.fill(); x.shadowColor = "transparent";
    x.strokeStyle = dark ? "rgba(255,255,255,.08)" : "rgba(20,18,15,.13)"; x.lineWidth = 1; x.stroke();
    // the faces it broke along (the slanted edges) in a thin red line
    x.strokeStyle = "rgba(239,59,44,.75)"; x.lineWidth = 1.3; x.lineCap = "round";
    v.forEach((a, i) => { const b = v[(i + 1) % v.length]; if (Math.abs(a.x - b.x) > .5 && Math.abs(a.y - b.y) > .5) { x.beginPath(); x.moveTo(a.x, a.y); x.lineTo(b.x, b.y); x.stroke(); } });
    if (p.label) {
      x.font = stencil(p.fs); x.textAlign = "center"; x.textBaseline = "middle"; x.fillStyle = dark ? "#e6e7ea" : INK; x.fillText(p.label, 0, .5);
    }
    sprites.set(key, sp); return sp;
  };
  const drawPiece = (p: Piece) => {
    const b = p.body, sp = pieceSprite(p), d = sp.width / Math.min(2, devicePixelRatio || 1);
    ctx.save(); ctx.globalAlpha = p.a * (hoverSrc && hoverSrc !== p.s ? .3 : 1);
    if (heldPiece && heldPiece.text === p.text && heldPiece.s === p.s) { ctx.shadowColor = "rgba(239,59,44,.9)"; ctx.shadowBlur = 12; }
    ctx.translate(b.position.x, b.position.y); ctx.rotate(b.angle); ctx.drawImage(sp, -d / 2, -d / 2, d, d);
    ctx.restore();
  };

  let last = performance.now(), acc = 0, visible = false;
  const frame = (now: number, dt: number) => {
    const k = dt / 16.7, g = grow;
    ctx.clearRect(0, 0, W, H);

    // Paths from each source to the field
    SRC.forEach((s) => {
      const p = inPath(s), on = hoverSrc === s;
      ctx.beginPath(); ctx.moveTo(p[0][0], p[0][1]); ctx.bezierCurveTo(p[1][0], p[1][1], p[2][0], p[2][1], p[3][0], p[3][1]);
      ctx.strokeStyle = on ? (s.id === "you" ? BLUE : "rgba(61,58,53,.55)") : "rgba(20,18,15,.08)"; ctx.lineWidth = on ? 2 : 1.2; ctx.setLineDash([3, 6]); ctx.lineDashOffset = -now / 40; ctx.stroke(); ctx.setLineDash([]);
    });
    // Held chip: the path back to where it came from
    if (held && held.state === "fly") {
      const lut = held.lut ?? table(inPath(held.s, held.w)); ctx.beginPath(); ctx.moveTo(lut[0][0], lut[0][1]);
      for (let i = 1; i <= 30; i++) { const q = at(lut, held.t * i / 30); ctx.lineTo(q[0], q[1]); }
      ctx.strokeStyle = srcInk(held.s); ctx.lineWidth = 2.5; ctx.stroke();
    }

    // The terminal on the right, where calls that pass get run
    ctx.save(); ctx.globalAlpha = g; ctx.translate((1 - g) * 40, 0);
    ctx.shadowColor = "rgba(20,30,60,.35)"; ctx.shadowBlur = 40; ctx.shadowOffsetY = 18; roundRect(TX, TY, TW, TH, 14); ctx.fillStyle = "#16171b"; ctx.fill(); ctx.shadowColor = "transparent";
    roundRect(TX, TY, TW, 32, 14); ctx.fillStyle = "#1e1f24"; ctx.fill(); ctx.fillRect(TX, TY + 18, TW, 14);
    ["#3a3c42", "#3a3c42", "#3a3c42"].forEach((c, i) => { ctx.beginPath(); ctx.arc(TX + 16 + i * 14, TY + 16, 4.5, 0, Math.PI * 2); ctx.fillStyle = c; ctx.fill(); });
    ctx.font = MONO; ctx.fillStyle = "#82868f"; ctx.textAlign = "right"; ctx.fillText(`실행 ${ran}`, TX + TW - 14, TY + 20); ctx.textAlign = "left";
    log.slice(-Math.floor((TH - 50) / 24)).forEach((l, i) => {
      const a = Math.min(1, (now - l.t) / 300); ctx.globalAlpha = g * a;
      ctx.fillStyle = GREEN; ctx.fillText("✓", TX + 16, TY + 56 + i * 24); ctx.fillStyle = "#d8d8d8"; ctx.fillText(l.text, TX + 34, TY + 56 + i * 24);
    });
    ctx.restore();

    // The field: a line of light that ripples where calls meet it
    ripples = ripples.filter((r) => now - r.t < 1600);
    if (g > .01) {
      const top = GY - (GY - FT) * g, bot = GY + (FB - GY) * g;
      const grad = ctx.createLinearGradient(0, top, 0, bot); grad.addColorStop(0, "rgba(31,107,255,0)"); grad.addColorStop(.15, "rgba(31,107,255,.9)"); grad.addColorStop(.85, "rgba(31,107,255,.9)"); grad.addColorStop(1, "rgba(31,107,255,0)");
      for (const [lw, al] of [[14, .14], [2.5, 1]] as const) {
        ctx.beginPath(); for (let y = top; y <= bot; y += 6) { const x = fieldX(y, now); y === top ? ctx.moveTo(x, y) : ctx.lineTo(x, y); }
        ctx.strokeStyle = grad; ctx.globalAlpha = al; ctx.lineWidth = lw; ctx.lineCap = "round"; ctx.stroke();
      }
      ctx.globalAlpha = 1;
      for (const r of ripples) { const a = Math.max(0, 1 - (now - r.t) / 700); if (r.c === RED && a > 0) { const rg = ctx.createRadialGradient(GX, r.y, 0, GX, r.y, 60); rg.addColorStop(0, `rgba(239,59,44,${.45 * a})`); rg.addColorStop(1, "rgba(239,59,44,0)"); ctx.fillStyle = rg; ctx.fillRect(GX - 60, r.y - 60, 120, 120); } }
      // its name, above
      ctx.globalAlpha = g; if (logo.complete) ctx.drawImage(logo, GX - 64, top - 52, 26, 26);
      ctx.fillStyle = BLUE; ctx.font = '700 15px "Geist", sans-serif'; ctx.fillText("WithHuman", GX - 32, top - 33);
      ctx.font = 'italic 21px "Instrument Serif", serif'; ctx.textAlign = "center"; ctx.fillText("who asked?", GX, bot + 30); ctx.textAlign = "left"; ctx.globalAlpha = 1;
    }

    // Sources, down the left
    SRC.forEach((s, i) => {
      const kk = Math.max(0, Math.min(1, g * 1.6 - i * .12)), on = hoverSrc === s;
      ctx.save(); ctx.globalAlpha = kk * (hoverSrc && !on ? .35 : 1); ctx.translate((1 - kk) * -30, 0);
      const you = s.id === "you";
      ctx.save(); ctx.translate(SX + 30, s.y - (on ? 2 : 0)); ctx.rotate(TILT[s.id]);
      ctx.shadowColor = you ? "rgba(31,107,255,.35)" : "rgba(40,34,24,.18)"; ctx.shadowBlur = on ? 18 : 10; ctx.shadowOffsetY = on ? 6 : 4;
      ARTIFACT[s.id](ctx, on);
      ctx.restore();
      ctx.font = `${you ? 700 : 600} 14.5px "Geist", sans-serif`; ctx.fillStyle = you ? "#1557d6" : INK; ctx.fillText(s.name, SX + 70, s.y - 2);
      ctx.font = '500 11.5px "Geist Mono", monospace'; ctx.fillStyle = on ? srcInk(s) : "#8d9098"; ctx.fillText(on ? "눌러서 보내기 →" : you ? "내 요청" : "내가 아님", SX + 70, s.y + 14);
      ctx.restore();
    });

    // Chips
    Matter.Engine.update(engine, Math.min(dt, 33));
    for (const c of chips) {
      if (c === held) continue;
      if (c.state === "fly") {
        if (!c.lut) { c.lut = table(inPath(c.s, c.w)); c.len = c.lut[c.lut.length - 1][2]; }
        c.t += dt * .34 / c.len!; [c.x, c.y] = at(c.lut, c.t);
        if (c.t >= 1) {
          if (c.s.id === "you") { c.state = "pass"; c.t = 0; c.lut = undefined; ripples.push({ y: GY, t: now, c: BLUE, amp: 5 }); }
          else {
            c.state = "gone"; shatter(c, c.x - 4, c.y);
            ripples.push({ y: GY, t: now, c: RED, amp: 16 });
            for (let i = 0; i < 22; i++) { const a = Math.PI * (.55 + Math.random() * .9), sp = 1 + Math.random() * 4; sparks.push({ x: GX - 4, y: GY, vx: Math.cos(a) * sp, vy: Math.sin(a) * sp - 1, a: 1 }); }
          }
        }
      } else if (c.state === "pass") {
        if (!c.lut || c.t === 0) { c.lut = table(outPath(c.w, Math.min(log.length, Math.floor((TH - 50) / 24) - 1))); c.len = c.lut[c.lut.length - 1][2]; }
        c.t += dt * .4 / c.len!; [c.x, c.y] = at(c.lut, c.t);
        c.a = c.t > .85 ? Math.max(0, 1 - (c.t - .85) / .15) : 1;
        if (c.t >= 1) { c.state = "gone"; ran++; log.push({ text: c.text, t: now }); if (log.length > 30) log.shift(); }
      }
    }
    chips = chips.filter((c) => c.state !== "gone");
    for (const p of pieces) {
      // a splinter squeezed past the field goes back to its side
      if (p.body.position.x > GX - 44) Matter.Body.setPosition(p.body, { x: GX - 60, y: Math.min(p.body.position.y, FLOOR - 20) });
      if (p.fade === undefined && now - p.born > PIECE_LIFE) p.fade = now;
      if (p.fade !== undefined) { p.a = Math.max(0, 1 - (now - p.fade) / 500); if (p.a <= 0) Matter.Composite.remove(engine.world, p.body); }
    }
    pieces = pieces.filter((p) => p.a > 0);
    pieces.forEach(drawPiece);
    chips.forEach(drawChip);
    if (held) drawChip(held);
    sparks = sparks.filter((s) => s.a > .04);
    for (const s of sparks) { s.vy += .15; s.x += s.vx; s.y += s.vy; s.a *= .92; ctx.globalAlpha = s.a; ctx.fillStyle = s.a > .5 ? "rgba(239,59,44,.8)" : "#c9c4ba"; ctx.fillRect(s.x - 1.1, s.y - 1.1, 2.2, 2.2); }
    ctx.globalAlpha = 1;

  };

  const tipAt = (c: { s: Src; text: string; x: number; y: number; stopped: boolean } | null) => {
    if (!c) { tip.classList.remove("show"); return; }
    const bad = c.s.id !== "you", r = cv.getBoundingClientRect();
    tip.className = "gtip show " + (bad ? "no" : "ok");
    tip.innerHTML = `<b>${esc(c.text)}</b><span><i>출처</i>${esc(c.s.id === "you" ? "내 요청" : c.s.name)}</span><em>${bad ? (c.stopped ? "멈춤 · " + esc(c.s.why) : "→ 막힘") : "→ 실행"}</em>`;
    tip.style.transform = `translate(${Math.min(c.x + 16, r.width - 290)}px, ${c.y - 86}px)`;
  };
  const loop = (now: number) => {
    if (!visible) return;
    requestAnimationFrame(loop);
    const dt = Math.min(50, now - last); last = now;
    acc += dt; if (acc > 1300) { acc = 0; fire(Math.random() < .72 ? SRC[0] : SRC[1 + Math.floor(Math.random() * 4)]); }
    // what the pointer is on
    held = null; heldPiece = null; hoverSrc = null;
    if (mx >= 0) {
      hoverSrc = SRC.find((s) => mx >= SX && mx <= SX + 190 && Math.abs(my - s.y) < 24) || null;
      for (let i = chips.length - 1; i >= 0; i--) { const c = chips[i]; if (c.state === "fly" && Math.abs(mx - c.x) < c.w / 2 + 4 && Math.abs(my - c.y) < CH / 2 + 4) { held = c; break; } }
      if (!held) { const hit = Matter.Query.point(pieces.map((p) => p.body), { x: mx, y: my })[0]; heldPiece = hit ? pieces.find((p) => p.body === hit) ?? null : null; }
    }
    frame(now, dt);
    tipAt(held ? { ...held, stopped: false } : heldPiece ? { s: heldPiece.s, text: heldPiece.text, x: heldPiece.body.position.x, y: heldPiece.body.position.y, stopped: true } : null);
  };
  cv.addEventListener("pointermove", (e) => { const r = cv.getBoundingClientRect(); mx = e.clientX - r.left; my = e.clientY - r.top; });
  cv.addEventListener("pointerleave", () => { mx = my = -1; });
  cv.addEventListener("click", () => { if (hoverSrc) { const s = hoverSrc; fire(s); setTimeout(() => fire(s), 220); setTimeout(() => fire(s), 440); } });
  if (!fine) cv.addEventListener("touchstart", (e) => { const r = cv.getBoundingClientRect(); mx = e.touches[0].clientX - r.left; my = e.touches[0].clientY - r.top; setTimeout(() => (mx = my = -1), 1600); }, { passive: true });

  let started = false, active = false;
  const start = () => {
    started = true;
    layout();
    if (still || reduced) {
      grow = 1; ["npm test", "Edit src/parser.rs", "git commit -m 'fix'"].forEach((t) => log.push({ text: t, t: 0 })); ran = 3;
      [1, 2, 3, 4, 1].forEach((i, n) => { const s = SRC[i], text = s.calls[n % s.calls.length]; shatter({ s, text, w: measure(text), state: "gone", t: 0, x: 0, y: 0, a: 1, born: 0 }, GX - 60 - n * 30, H * .3 - n * 30); });
      pieces.forEach((p) => (p.born = Infinity));
      for (let i = 0; i < 300; i++) Matter.Engine.update(engine, 16);
      frame(performance.now(), 16); return;
    }
    const gw = { v: 0 };
    gsap.to(gw, { v: 1, duration: 1.4, ease: "expo.out", delay: .35, onUpdate: () => (grow = gw.v) });
  };
  const run = () => { if (!active || visible) return; visible = true; last = performance.now(); requestAnimationFrame(loop); };
  addEventListener("resize", () => { if (started) layout(); });
  return {
    setActive: (v: boolean) => {
      active = v;
      if (!v) { visible = false; return; }
      document.fonts.ready.then(() => { if (!started) start(); if (!still && !reduced) run(); });
    },
    /** Fires three calls from a source, as a click on it does. */
    fire: (id: Key) => { const s = SRC.find((x) => x.id === id); if (s) [0, 220, 440].forEach((t) => setTimeout(() => fire(s), t)); },
  };
}
