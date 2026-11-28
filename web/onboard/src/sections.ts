// The landing's reveals (site/src/sections.ts, initReveals: headings rise line by line, the kick
// slides in, panels come up), played when a scene comes in instead of on scroll; and the hero's
// scroll-out (site/src/hero.ts, heroScrollOut) when it leaves.
import gsap from "gsap";
import { SplitText } from "gsap/SplitText";
import { $, $$, calm } from "./lib";

const splits = new WeakMap<Element, SplitText>();

export function sceneIn(el: HTMLElement, dir: number) {
  gsap.killTweensOf(el);
  gsap.set(el, { autoAlpha: 1, y: 0, scale: 1, yPercent: 0 });
  el.classList.add("on");
  if (calm) { $$(".rv", el).forEach((r) => gsap.set(r, { opacity: 1, y: 0 })); return; }
  const h = $(".split", el);
  if (h) {
    splits.get(h)?.revert();
    const s = SplitText.create(h, { type: "lines", linesClass: "line", mask: "lines" });
    splits.set(h, s);
    gsap.from(s.lines, { yPercent: 110, duration: 1.1, ease: "expo.out", stagger: .1, onComplete: () => { s.revert(); splits.delete(h); } });
  }
  const k = $(".kick", el); if (k) gsap.from(k, { opacity: 0, x: -20, duration: .8, ease: "expo.out" });
  const l = $(".lead", el); if (l) gsap.from(l, { opacity: 0, y: 16, duration: .9, ease: "expo.out", delay: .15 });
  const rv = $$(".rv", el);
  if (rv.length) gsap.fromTo(rv, { opacity: 0, y: 40 * Math.sign(dir || 1) }, { opacity: 1, y: 0, duration: 1, ease: "expo.out", stagger: .08, delay: .12, overwrite: true });
}

export function sceneOut(el: HTMLElement, dir: number) {
  el.classList.remove("on");
  if (calm) { gsap.set(el, { autoAlpha: 0 }); return; }
  gsap.to(el, { yPercent: -10 * Math.sign(dir || 1), scale: .95, autoAlpha: 0, duration: .55, ease: "power2.in" });
}
