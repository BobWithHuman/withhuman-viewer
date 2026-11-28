// The landing's page-wide effects (site/src/fx.ts): the cursor, magnetic buttons, the star burst.
import gsap from "gsap";
import { $, $$, fine, reduced } from "./lib";

export function initCursor() {
  if (!fine || reduced) return;
  document.body.classList.add("has-cursor");
  const cur = $("#cursor"), dot = $("#c-dot"), ring = $("#c-ring"), lab = $("#c-lab");
  const dx = gsap.quickTo(dot, "x", { duration: .08 }), dy = gsap.quickTo(dot, "y", { duration: .08 });
  const rx = gsap.quickTo(ring, "x", { duration: .45, ease: "power3" }), ry = gsap.quickTo(ring, "y", { duration: .45, ease: "power3" });
  addEventListener("pointermove", (e) => {
    dx(e.clientX); dy(e.clientY); rx(e.clientX); ry(e.clientY);
    cur.classList.remove("gone");
    const t = e.target as HTMLElement;
    const labelled = t.closest?.("[data-cursor]") as HTMLElement | null;
    cur.classList.toggle("lab", !!labelled);
    lab.textContent = labelled?.dataset.cursor || "";
    cur.classList.toggle("hov", !labelled && !!t.closest?.("a, button, [role=tab]"));
  }, { passive: true });
  document.addEventListener("pointerleave", () => cur.classList.add("gone"));
  // Over an iframe the page stops getting moves; hide ours so two cursors don't show.
  $$("iframe").forEach((f) => f.addEventListener("pointerenter", () => cur.classList.add("gone")));
  addEventListener("pointerdown", () => cur.classList.add("dn"));
  addEventListener("pointerup", () => cur.classList.remove("dn"));
}

export function initMagnets(root: ParentNode = document) {
  if (!fine || reduced) return;
  $$(".mag", root).forEach((el) => {
    if (el.dataset.mag) return; el.dataset.mag = "1";
    const x = gsap.quickTo(el, "x", { duration: .5, ease: "power3" }), y = gsap.quickTo(el, "y", { duration: .5, ease: "power3" });
    el.addEventListener("pointermove", (e) => {
      const r = el.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
      x((mx - r.width / 2) * .35); y((my - r.height / 2) * .45);
      el.style.setProperty("--bx", mx + "px"); el.style.setProperty("--by", my + "px");
    });
    el.addEventListener("pointerleave", () => { gsap.to(el, { x: 0, y: 0, duration: .9, ease: "elastic.out(1, .35)" }); });
  });
}

// Stars and sparks thrown from where you clicked "Star".
export function initBurst() {
  const cv = $<HTMLCanvasElement>("#burst"), ctx = cv.getContext("2d")!;
  type P = { x: number; y: number; vx: number; vy: number; r: number; a: number; c: string; star: boolean; rot: number; vr: number };
  let ps: P[] = [], running = false;
  const size = () => { const d = Math.min(2, devicePixelRatio || 1); cv.width = innerWidth * d; cv.height = innerHeight * d; ctx.setTransform(d, 0, 0, d, 0, 0); };
  size(); addEventListener("resize", size);
  const COLORS = ["#1f6bff", "#ef3b2c", "#e59a00", "#121212", "#5c95ff"];
  const starPath = (r: number) => { ctx.beginPath(); for (let i = 0; i < 10; i++) { const a = i * Math.PI / 5 - Math.PI / 2, rr = i % 2 ? r * .45 : r; ctx.lineTo(Math.cos(a) * rr, Math.sin(a) * rr); } ctx.closePath(); };
  const frame = () => {
    ctx.clearRect(0, 0, innerWidth, innerHeight);
    ps = ps.filter((p) => p.a > .02 && p.y < innerHeight + 40);
    for (const p of ps) {
      p.vy += .32; p.vx *= .985; p.x += p.vx; p.y += p.vy; p.a *= .975; p.rot += p.vr;
      ctx.save(); ctx.globalAlpha = p.a; ctx.fillStyle = p.c; ctx.translate(p.x, p.y); ctx.rotate(p.rot);
      if (p.star) { starPath(p.r); ctx.fill(); } else ctx.fillRect(-p.r / 2, -p.r, p.r, p.r * 2);
      ctx.restore();
    }
    if (ps.length) requestAnimationFrame(frame); else running = false;
  };
  const burst = (x: number, y: number) => {
    for (let i = 0; i < 70; i++) {
      const a = Math.random() * Math.PI * 2, s = 4 + Math.random() * 10;
      ps.push({ x, y, vx: Math.cos(a) * s, vy: Math.sin(a) * s - 6, r: 4 + Math.random() * 7, a: 1, c: COLORS[i % COLORS.length], star: i % 3 === 0, rot: Math.random() * 6, vr: (Math.random() - .5) * .4 });
    }
    if (!running) { running = true; requestAnimationFrame(frame); }
  };
  if (!reduced) $$(".star").forEach((b) => b.addEventListener("click", (e) => burst((e as MouseEvent).clientX, (e as MouseEvent).clientY)));
  return (x: number, y: number) => { if (!reduced) burst(x, y); };
}
