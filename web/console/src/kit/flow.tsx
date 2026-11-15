// A flow in three columns: every item as a band running from the first column's node, through
// the second's, to the third's, coloured by where it ends, the heavier ends the stronger. A band
// or a node pressed picks it, and the rest fall back.

import { ChevronRight, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { cn } from "@/lib/utils";
import { Empty, pct } from ".";

export type FlowKeys = [string, string, string];
/** Items that share their three nodes. */
export type FlowRow = { keys: FlowKeys; n: number };
/** A column: its heading, the order of its nodes, and each node's name and icon. */
export type FlowColumn = { title: string; order: readonly string[]; label: (key: string) => string; icon?: (key: string) => LucideIcon | undefined };
export type FlowPick = [string | undefined, string | undefined, string | undefined];

type Rib = { keys: FlowKeys; n: number; th: number; y: [number, number, number, number] };
type Node = { col: 0 | 1 | 2; key: string; n: number; y: number; h: number };
type Hover = { rib: Rib } | { node: Node } | null;

const TOP = 46, BOTTOM = 6, BAR = 8, MIN = 3;

/** The bands and nodes laid out in a box: one scale for all three columns, each centred. */
function layout(rows: FlowRow[], columns: FlowColumn[], height: number, size: (n: number) => number) {
  const count = new Map<string, Rib>();
  for (const row of rows) {
    if (row.n <= 0) continue;
    const id = row.keys.join("|");
    const r = count.get(id) ?? { keys: row.keys, n: 0, th: 0, y: [0, 0, 0, 0] };
    r.n += row.n;
    count.set(id, r);
  }
  const ribs = [...count.values()];
  // The nodes each column has, in its order; a key the column does not name comes last.
  const keys = columns.map((c, i) => {
    const seen = [...new Set(ribs.map((r) => r.keys[i]))];
    return [...c.order.filter((k) => seen.includes(k)), ...seen.filter((k) => !c.order.includes(k))];
  });
  const most = Math.max(...keys.map((k) => k.length), 1);
  const inner = height - TOP - BOTTOM;
  // Room between nodes for the middle column's names, less of it when there are many nodes.
  const gap = Math.max(12, Math.min(26, (inner * 0.45) / most));
  const room = inner - gap * (most - 1);
  // The largest scale at which the bands, none thinner than MIN, still fit the tallest column.
  let lo = 0, hi = room;
  for (let i = 0; i < 40; i++) {
    const k = (lo + hi) / 2;
    if (ribs.reduce((a, r) => a + Math.max(size(r.n) * k, MIN), 0) <= room) lo = k;
    else hi = k;
  }
  for (const r of ribs) r.th = Math.max(size(r.n) * lo, MIN);
  const total = ribs.reduce((a, r) => a + r.th, 0);
  const order = (r: Rib) => r.keys.map((k, i) => keys[i].indexOf(k));
  // A node stacks its bands by where they go next, so they cross as little as may be; a middle
  // node stacks them the same way on both its sides, so each band leaves where it came in.
  const sides: { col: 0 | 1 | 2; by: number[]; slot: number }[] = [
    { col: 0, by: [1, 2], slot: 0 },
    { col: 1, by: [2, 0], slot: 1 },
    { col: 1, by: [2, 0], slot: 2 },
    { col: 2, by: [1, 0], slot: 3 },
  ];
  const nodes: Node[] = [];
  for (const side of sides) {
    let y = TOP + (inner - total - gap * (keys[side.col].length - 1)) / 2;
    for (const key of keys[side.col]) {
      const mine = ribs.filter((r) => r.keys[side.col] === key).sort((a, b) => order(a)[side.by[0]] - order(b)[side.by[0]] || order(a)[side.by[1]] - order(b)[side.by[1]]);
      const top = y;
      for (const r of mine) {
        r.y[side.slot] = y;
        y += r.th;
      }
      if (side.slot !== 2) nodes.push({ col: side.col, key, n: mine.reduce((a, r) => a + r.n, 0), y: top, h: y - top });
      y += gap;
    }
  }
  return { ribs, nodes, gap };
}

/** A band between two columns: the same thickness at both ends, eased between them. */
const band = (xa: number, ya: number, xb: number, yb: number, th: number) => {
  const m = (xa + xb) / 2;
  return `M${xa},${ya}C${m},${ya} ${m},${yb} ${xb},${yb}L${xb},${yb + th}C${m},${yb + th} ${m},${ya + th} ${xa},${ya + th}Z`;
};

export function Flow({ rows, columns, color, weight, picked, onPick, height, unit = "개", left = 150, right = 136, scale = "linear" }: {
  rows: FlowRow[]; columns: [FlowColumn, FlowColumn, FlowColumn];
  /** An end node's colour, and how much of it a band there has when nothing is picked (percent). */
  color: (end: string) => string; weight?: (end: string) => number;
  picked: FlowPick; onPick: (picked: FlowPick) => void;
  /** The box's height; its container's when left out. */
  height?: number;
  unit?: string;
  /** Room for the first column's names and the last's. */
  left?: number; right?: number;
  /** How thick a band is for its count: in proportion, or by its square root so that a few
   * among very many still show. */
  scale?: "linear" | "sqrt";
}) {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<[number, number]>([0, 0]);
  const [hover, setHover] = useState<Hover>(null);
  const [tip, setTip] = useState<[number, number]>([0, 0]);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setSize([el.clientWidth, el.clientHeight]);
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    measure();
    return () => ro.disconnect();
  }, []);
  const width = size[0], tall = height ?? size[1];
  const { ribs, nodes, gap } = useMemo(() => layout(rows, columns, Math.max(tall, 120), scale === "sqrt" ? Math.sqrt : (n) => n), [rows, columns, tall, scale]);
  const total = ribs.reduce((a, r) => a + r.n, 0);
  const x = [left, (left + width - right - BAR) / 2, width - right - BAR];
  const narrowed = picked.some((p) => p !== undefined);
  const through = (r: Rib, n: Node) => r.keys[n.col] === n.key;
  const lit = (r: Rib) => {
    if (hover) return "rib" in hover ? hover.rib === r : through(r, hover.node);
    return picked.every((p, i) => p === undefined || p === r.keys[i]);
  };
  const isPicked = (n: Node) => picked[n.col] === n.key;
  const move = (e: MouseEvent) => {
    const b = box.current?.getBoundingClientRect();
    if (b) setTip([e.clientX - b.left, e.clientY - b.top]);
  };
  const pickRib = (r: Rib) => onPick(picked.every((p, i) => p === r.keys[i]) ? [undefined, undefined, undefined] : [...r.keys]);
  const pickNode = (n: Node) => onPick(picked.map((p, i) => (i === n.col ? (isPicked(n) ? undefined : n.key) : p)) as FlowPick);
  const name = (n: Node) => columns[n.col].label(n.key);
  // The middle column's names sit above their nodes when the gaps have room, else beside them.
  const above = gap >= 22;

  return (
    <div ref={box} className="relative w-full select-none" style={{ height: height ?? "100%" }} onMouseMove={move} onMouseLeave={() => setHover(null)}>
      {!total && <Empty />}
      {total > 0 && width > 0 && tall > 0 && (
        <>
          <svg width={width} height={tall} className="absolute inset-0">
            {/* Bands are opaque tints of the card, drawn quiet ends first, so where two cross one
                lies over the other instead of the colours mixing. */}
            {[...ribs].sort((a, b) => Number(lit(a)) - Number(lit(b)) || (weight?.(a.keys[2]) ?? 0) - (weight?.(b.keys[2]) ?? 0)).map((r) => {
              const on = lit(r), strong = on && (hover !== null || narrowed);
              const share = strong ? 80 : on ? weight?.(r.keys[2]) ?? 46 : 8;
              return (
                <g key={r.keys.join("|")} className="cursor-pointer [transition:fill_150ms]"
                  fill={`color-mix(in oklab, ${color(r.keys[2])} ${share}%, var(--card))`} stroke="var(--card)" strokeWidth={r.th > 4 ? 1 : 0}
                  onMouseEnter={() => setHover({ rib: r })} onClick={() => pickRib(r)}>
                  <path d={band(x[0] + BAR, r.y[0], x[1], r.y[1], r.th)} />
                  <path d={band(x[1] + BAR, r.y[2], x[2], r.y[3], r.th)} />
                </g>
              );
            })}
            {nodes.map((n) => (
              <rect key={`${n.col}${n.key}`} x={x[n.col]} y={n.y} width={BAR} height={n.h} rx={2} className="cursor-pointer"
                fill={n.col === 2 ? color(n.key) : "var(--foreground)"} opacity={n.col === 2 ? 1 : isPicked(n) ? 0.95 : 0.6}
                onMouseEnter={() => setHover({ node: n })} onClick={() => pickNode(n)} />
            ))}
          </svg>
          {columns.map((c, i) => (
            <span key={c.title} className="absolute top-0 text-[11.5px] text-muted-foreground" style={i === 0 ? { right: width - x[0] - BAR } : { left: x[i] }}>{c.title}</span>
          ))}
          {nodes.map((n) => {
            const Icon = columns[n.col].icon?.(n.key);
            const on = isPicked(n);
            const other = picked[n.col] !== undefined && !on;
            return (
              <button key={`${n.col}${n.key}`} onClick={() => pickNode(n)} onMouseEnter={() => setHover({ node: n })} aria-pressed={on}
                className={cn(
                  "absolute flex h-6 max-w-[168px] items-center gap-1.5 rounded-md px-1.5 text-[12.5px] whitespace-nowrap transition-colors hover:bg-accent",
                  n.col === 1 && (above ? "-translate-x-1.5 bg-card/80" : "bg-card/85"), on && "bg-accent font-semibold ring-1 ring-ring", other && "opacity-45",
                )}
                style={n.col === 1 && above ? { top: n.y - 25, left: x[1] } : { top: n.y + n.h / 2 - 12, ...(n.col === 0 ? { right: width - x[0] + 6, maxWidth: left - 8 } : { left: x[n.col] + BAR + 6 }) }}>
                {Icon && <Icon className="size-3.5 shrink-0" strokeWidth={1.9} style={{ color: n.col === 2 ? color(n.key) : "var(--muted-foreground)" }} />}
                <span className="truncate">{name(n)}</span>
                <b className="tnum font-medium text-muted-foreground">{n.n.toLocaleString()}</b>
              </button>
            );
          })}
          {hover && (
            <div className="pointer-events-none absolute z-20 w-max min-w-[168px] rounded-lg border bg-popover px-3 py-2 text-[12px] shadow-xl"
              style={{ left: Math.max(0, Math.min(tip[0] + 14, width - 260)), top: Math.max(0, Math.min(tip[1] + 14, tall - ("rib" in hover ? 62 : 132))) }}>
              {"rib" in hover ? (
                <>
                  <div className="flex items-center gap-1 text-muted-foreground">
                    {columns[0].label(hover.rib.keys[0])}<ChevronRight className="size-3" />{columns[1].label(hover.rib.keys[1])}<ChevronRight className="size-3" />
                    <span style={{ color: color(hover.rib.keys[2]) }}>{columns[2].label(hover.rib.keys[2])}</span>
                  </div>
                  <div className="tnum mt-1 flex items-baseline gap-1.5"><b className="text-[15px] font-semibold">{hover.rib.n.toLocaleString()}</b>{unit}<span className="ml-auto text-muted-foreground">{pct(hover.rib.n, total)}%</span></div>
                </>
              ) : (
                <>
                  <div className="tnum flex items-baseline gap-1.5">
                    <span className="font-medium">{name(hover.node)}</span>
                    <b className="ml-auto pl-4 text-[15px] font-semibold">{hover.node.n.toLocaleString()}</b>{unit}<span className="text-muted-foreground">{pct(hover.node.n, total)}%</span>
                  </div>
                  <div className="mt-1.5 grid gap-1 border-t pt-1.5">
                    {/* An end node is split by the middle column; any other by how it ended. */}
                    {(() => {
                      const node = hover.node, by = node.col === 2 ? 1 : 2;
                      const mine = ribs.filter((r) => through(r, node));
                      const seen = [...new Set(mine.map((r) => r.keys[by]))];
                      const keys = [...columns[by].order.filter((k) => seen.includes(k)), ...seen.filter((k) => !columns[by].order.includes(k))];
                      return keys.map((k) => (
                        <div key={k} className="tnum flex items-center gap-1.5 text-muted-foreground">
                          <i className="size-2 rounded-full" style={{ background: by === 2 ? color(k) : "var(--muted-foreground)" }} />
                          {columns[by].label(k)}<b className="ml-auto pl-4 font-medium text-foreground">{mine.filter((r) => r.keys[by] === k).reduce((a, r) => a + r.n, 0).toLocaleString()}</b>
                        </div>
                      ));
                    })()}
                  </div>
                </>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
