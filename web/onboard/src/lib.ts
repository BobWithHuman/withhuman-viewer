// The landing's helpers (site/src/lib.ts), with the page's flags read from the query or the hash.
import { ICON } from "./icons";

export const $ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => r.querySelector(s) as T;
export const $$ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => [...r.querySelectorAll(s)] as T[];
export const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/"/g, "&quot;");
export const clamp = (v: number, a = 0, b = 1) => Math.max(a, Math.min(b, v));
// Standalone, the state to look at comes in the query or the hash (srcdoc in the app has neither).
export const q = new URLSearchParams(location.search || location.hash.replace(/^#\??/, ""));
// ?still freezes the page for screenshots: no intro, no transitions, canvases drawn once.
export const still = q.has("still");
export const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
export const calm = reduced || still;
export const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;

// <x data-ic="name">label</x> gets the icon put in front of its label.
export const hydrateIcons = (root: ParentNode = document) => $$("[data-ic]", root).forEach((el) => {
  if (el.dataset.icDone) return;
  const svg = ICON[el.dataset.ic!]; if (svg) el.insertAdjacentHTML("afterbegin", svg);
  el.dataset.icDone = "1";
});

export const fitCanvas = (cv: HTMLCanvasElement) => {
  const dpr = Math.min(2, devicePixelRatio || 1), r = cv.getBoundingClientRect();
  cv.width = Math.round(r.width * dpr); cv.height = Math.round(r.height * dpr);
  const ctx = cv.getContext("2d")!; ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { ctx, w: r.width, h: r.height, dpr };
};

/** Builds an element from HTML, its icons put in. */
export const h = <T extends HTMLElement = HTMLElement>(html: string) => {
  const t = document.createElement("template"); t.innerHTML = html.trim();
  const el = t.content.firstElementChild as T; hydrateIcons(el); return el;
};
