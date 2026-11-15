// Writes src/ui/icons.rs: Lucide icons as one path each, for the app's own chrome.
import { readFileSync, writeFileSync } from "node:fs";
const dir = "web/console/node_modules/lucide-react/dist/esm/icons/";
const names = ["house", "route", "clipboard-check", "scroll-text", "layout-grid", "star", "shield-check", "lock-keyhole", "monitor-smartphone", "refresh-cw", "maximize", "database", "settings", "panel-left"];
const n = (v) => +(+v).toFixed(3);
function toPath(tag, a) {
  if (tag === "path") return a.d;
  if (tag === "circle") { const r = +a.r, cx = +a.cx, cy = +a.cy; return `M${n(cx - r)} ${cy}a${r} ${r} 0 1 0 ${n(2 * r)} 0a${r} ${r} 0 1 0 ${n(-2 * r)} 0`; }
  if (tag === "ellipse") { const rx = +a.rx, ry = +a.ry, cx = +a.cx, cy = +a.cy; return `M${n(cx - rx)} ${cy}a${rx} ${ry} 0 1 0 ${n(2 * rx)} 0a${rx} ${ry} 0 1 0 ${n(-2 * rx)} 0`; }
  if (tag === "line") return `M${a.x1} ${a.y1}L${a.x2} ${a.y2}`;
  if (tag === "polyline" || tag === "polygon") { const p = a.points.trim().split(/\s+/).map((s) => s.replace(",", " ")); return "M" + p.join("L") + (tag === "polygon" ? "z" : ""); }
  if (tag === "rect") {
    const x = +(a.x ?? 0), y = +(a.y ?? 0), w = +a.width, h = +a.height, r = Math.min(+(a.rx ?? a.ry ?? 0), w / 2, h / 2);
    if (!r) return `M${x} ${y}h${w}v${h}h${-w}z`;
    return `M${n(x + r)} ${y}h${n(w - 2 * r)}a${r} ${r} 0 0 1 ${r} ${r}v${n(h - 2 * r)}a${r} ${r} 0 0 1 ${-r} ${r}h${n(2 * r - w)}a${r} ${r} 0 0 1 ${-r} ${-r}v${n(2 * r - h)}a${r} ${r} 0 0 1 ${r} ${-r}z`;
  }
  throw new Error("unknown " + tag);
}
let out = "//! Icons for the app's own chrome, from Lucide (https://lucide.dev, ISC licence), each as one\n//! path on a 24 by 24 box, drawn as an outline in the current colour. Made by a script from\n//! `lucide-react`'s icons; add a name there and run it again rather than drawing one by hand.\n\n";
for (const name of names) {
  const src = readFileSync(dir + name + ".mjs", "utf8");
  const m = src.match(/const __iconData = (\{[\s\S]*?\n\});/);
  const node = eval("(" + m[1] + ")").node;
  out += `pub const ${name.toUpperCase().replace(/-/g, "_")}: &str = "${node.map(([t, a]) => toPath(t, a)).join("")}";\n`;
}
writeFileSync("src/ui/icons.rs", out);
console.log(out.split("\n").length, "lines");
