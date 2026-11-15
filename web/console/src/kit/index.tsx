// The parts every dashboard is made of. A page is assembled from these and from the shadcn
// components in `@/components/ui`; it does not restyle them one by one, so the four pages read as
// one product.

import {
  ArrowDown, ArrowUp, ArrowUpDown, Eye, Landmark, Pencil, Send, Trash2, TriangleAlert, type LucideIcon,
} from "lucide-react";
import {
  type ColumnDef, type SortingState, flexRender, getCoreRowModel, getSortedRowModel, useReactTable,
} from "@tanstack/react-table";
import { useState, type ReactNode } from "react";
import {
  siAtlassian, siClaude, siFigma, siGithub, siGoogledrive, siNotion, siPostgresql, siSentry,
} from "simple-icons";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------- words and colours
export type Verdict = "allow" | "review" | "deny";
export const VERDICT: Record<Verdict, { label: string; color: string; cls: string }> = {
  allow: { label: "허용", color: "var(--allow)", cls: "bg-allow/15 text-allow" },
  review: { label: "확인", color: "var(--review)", cls: "bg-review/15 text-review" },
  deny: { label: "차단", color: "var(--deny)", cls: "bg-deny/15 text-deny" },
};
/** What a tool can do, as 인벤토리의 MCP 탭 and the registry name it. */
export type Nature = "read" | "write" | "delete" | "send" | "money";
export const NATURE: Record<Nature, { label: string; color: string; icon: LucideIcon }> = {
  read: { label: "읽기", color: "var(--read)", icon: Eye },
  write: { label: "쓰기", color: "var(--write)", icon: Pencil },
  delete: { label: "삭제", color: "var(--deny)", icon: Trash2 },
  send: { label: "외부 전송", color: "var(--send)", icon: Send },
  money: { label: "돈·계정", color: "var(--money)", icon: Landmark },
};
export type Severity = "crit" | "high" | "med" | "low" | "info";
export const SEVERITY: Record<Severity, { label: string; cls: string; color: string }> = {
  crit: { label: "치명", cls: "bg-[#e5484d] text-white", color: "#e5484d" },
  high: { label: "높음", cls: "bg-deny/20 text-deny", color: "var(--deny)" },
  med: { label: "중간", cls: "bg-review/20 text-review", color: "var(--review)" },
  low: { label: "낮음", cls: "bg-catch/15 text-catch", color: "var(--catch)" },
  info: { label: "정보", cls: "bg-white/8 text-muted-foreground", color: "var(--info)" },
};

// ---------------------------------------------------------------- small marks
export function VerdictBadge({ v, children }: { v: Verdict; children?: ReactNode }) {
  return <Badge variant="secondary" className={cn("gap-1 border-0 font-semibold", VERDICT[v].cls)}><i className="size-1.5 rounded-full bg-current" />{children ?? VERDICT[v].label}</Badge>;
}
export function SeverityBadge({ s }: { s: Severity }) {
  return <Badge variant="secondary" className={cn("border-0 font-semibold", SEVERITY[s].cls)}>{SEVERITY[s].label}</Badge>;
}
export function NatureIcon({ n, className }: { n: Nature; className?: string }) {
  const Icon = NATURE[n].icon;
  return <Icon className={cn("size-4 shrink-0", className)} style={{ color: NATURE[n].color }} strokeWidth={1.9} />;
}
export function Warn({ children }: { children: ReactNode }) {
  return <Badge variant="secondary" className="gap-1 border-0 bg-catch/15 font-semibold text-catch"><TriangleAlert className="size-3" />{children}</Badge>;
}

const SI: Record<string, { path: string; hex: string }> = {
  github: siGithub, atlassian: siAtlassian, notion: siNotion, postgres: siPostgresql, postgresql: siPostgresql,
  claude: siClaude, googledrive: siGoogledrive, figma: siFigma, sentry: siSentry,
};
/** A service's logo on a white disc; a server we have no logo for shows its first letter. */
export function ServiceLogo({ name, size = 20 }: { name: string; size?: number }) {
  const key = Object.keys(SI).find((k) => name.toLowerCase().includes(k));
  const inner = Math.round(size * 0.6);
  let mark: ReactNode;
  if (name.toLowerCase().includes("slack")) {
    mark = (
      <svg width={inner} height={inner} viewBox="0 0 24 24" aria-hidden>
        <rect x="9.5" y="2" width="5" height="9" rx="2.5" fill="#36C5F0" /><rect x="13" y="9.5" width="9" height="5" rx="2.5" fill="#2EB67D" />
        <rect x="9.5" y="13" width="5" height="9" rx="2.5" fill="#ECB22E" /><rect x="2" y="9.5" width="9" height="5" rx="2.5" fill="#E01E5A" />
      </svg>
    );
  } else if (key) {
    const dark = key === "github" || key === "notion";
    mark = <svg width={inner} height={inner} viewBox="0 0 24 24" aria-hidden><path d={SI[key].path} fill={dark ? "#111" : `#${SI[key].hex}`} /></svg>;
  } else {
    return <span className="grid shrink-0 place-items-center rounded-full bg-secondary font-semibold text-muted-foreground uppercase" style={{ width: size, height: size, fontSize: size * 0.48 }}>{name.slice(0, 1)}</span>;
  }
  return <span className="grid shrink-0 place-items-center rounded-full bg-white" style={{ width: size, height: size }}>{mark}</span>;
}

const FACE = ["#d4a5bd", "#97c4b2", "#d6bb90", "#b9a8e0", "#a3afd2", "#e0b3a0", "#9cc0dd"];
/** A person as a coloured disc with their initial; the colour is the same wherever they appear. */
export function Person({ name, size = 22, label }: { name: string; size?: number; label?: boolean }) {
  const short = name.replace(/@.*/, "");
  const initial = (short.split(/[._-]/).pop() || short).slice(0, 1).toUpperCase();
  let h = 0;
  for (const c of short) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const disc = <span className="grid shrink-0 place-items-center rounded-full font-semibold text-[#1a1a1a]" style={{ width: size, height: size, fontSize: size * 0.48, background: FACE[h % FACE.length] }}>{initial}</span>;
  if (label) return <span className="inline-flex min-w-0 items-center gap-2">{disc}<span className="truncate">{short}</span></span>;
  return <Tooltip><TooltipTrigger asChild>{disc}</TooltipTrigger><TooltipContent>{short}</TooltipContent></Tooltip>;
}
export function People({ names, max = 4 }: { names: string[]; max?: number }) {
  return (
    <span className="flex items-center">
      {names.slice(0, max).map((n) => <span key={n} className="-ml-1.5 rounded-full ring-2 ring-card first:ml-0"><Person name={n} /></span>)}
      {names.length > max && <span className="ml-1 text-[11.5px] text-muted-foreground">+{names.length - max}</span>}
    </span>
  );
}

// ---------------------------------------------------------------- page furniture
export function PageHeader({ title, meta, children }: { title: string; meta?: ReactNode; children?: ReactNode }) {
  return (
    <header className="flex shrink-0 items-center gap-3 px-6 pt-5 pb-4">
      <h1 className="text-[20px] font-semibold tracking-[-0.01em]">{title}</h1>
      {meta && <div className="flex items-center gap-2 text-[12.5px] text-muted-foreground">{meta}</div>}
      <div className="ml-auto flex items-center gap-2">{children}</div>
    </header>
  );
}

/** A card with a title row; `aside` sits at the right of the title. */
export function Panel({ title, count, aside, children, className, pad = true }: { title?: ReactNode; count?: number; aside?: ReactNode; children: ReactNode; className?: string; pad?: boolean }) {
  return (
    <Card className={cn("gap-0 py-0 shadow-none", className)}>
      {title && (
        <CardHeader className="flex h-12 flex-row items-center gap-2 px-4 py-0">
          <CardTitle className="text-[13.5px] font-semibold">{title}</CardTitle>
          {count !== undefined && <span className="tnum rounded-full bg-secondary px-1.5 text-[11px] text-muted-foreground">{count}</span>}
          {aside && <div className="ml-auto flex items-center gap-2 text-[12px] text-muted-foreground">{aside}</div>}
        </CardHeader>
      )}
      <CardContent className={cn(pad ? "px-4 pb-4" : "p-0", !title && pad && "pt-4")}>{children}</CardContent>
    </Card>
  );
}

/** How every headline figure is set, on a `Stat` or inside a page's own summary. */
export const FIGURE = "tnum text-[28px] leading-none font-semibold tracking-[-0.02em]";

/** One figure with its name, and how it moved against the period before when that is known. */
export function Stat({ label, value, unit, delta, tone, icon: Icon, onClick, active }: {
  label: string; value: ReactNode; unit?: string; delta?: number | null; tone?: string; icon?: LucideIcon; onClick?: () => void; active?: boolean;
}) {
  return (
    <button onClick={onClick} disabled={!onClick} className={cn(
      "flex min-w-0 flex-col gap-2 rounded-xl border bg-card px-4 py-3.5 text-left transition-colors",
      onClick && "hover:border-white/20", active && "border-ring bg-accent/60",
    )}>
      <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">{Icon && <Icon className="size-3.5" />}{label}</span>
      <span className="flex items-baseline gap-1.5">
        <span className={FIGURE} style={tone ? { color: tone } : undefined}>{value}</span>
        {unit && <span className="text-[12.5px] text-muted-foreground">{unit}</span>}
        {delta !== undefined && delta !== null && delta !== 0 && (
          <span className={cn("tnum ml-auto inline-flex items-center gap-0.5 rounded-md px-1.5 py-0.5 text-[11.5px] font-medium", "bg-white/[.07] text-muted-foreground")}>
            {delta > 0 ? <ArrowUp className="size-3" /> : <ArrowDown className="size-3" />}{Math.abs(delta)}%
          </span>
        )}
      </span>
    </button>
  );
}

/** Parts of a whole as one bar; each part can be pressed. */
export function SplitBar({ parts, height = 8, onPick }: { parts: { key: string; label: string; value: number; color: string }[]; height?: number; onPick?: (key: string) => void }) {
  const total = parts.reduce((a, p) => a + p.value, 0) || 1;
  return (
    <div className="flex gap-0.5 overflow-hidden rounded-full" style={{ height }}>
      {parts.filter((p) => p.value > 0).map((p) => (
        <Tooltip key={p.key}>
          <TooltipTrigger asChild>
            <span onClick={() => onPick?.(p.key)} className={cn("h-full transition-[filter] hover:brightness-110", onPick && "cursor-pointer")} style={{ flex: p.value / total, background: p.color }} />
          </TooltipTrigger>
          <TooltipContent>{p.label} {p.value.toLocaleString()}</TooltipContent>
        </Tooltip>
      ))}
    </div>
  );
}

export function Legend({ items }: { items: { label: string; color: string; value?: number }[] }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] text-muted-foreground">
      {items.map((i) => <span key={i.label} className="inline-flex items-center gap-1.5"><i className="size-2 rounded-full" style={{ background: i.color }} />{i.label}{i.value !== undefined && <b className="tnum font-medium text-foreground">{i.value.toLocaleString()}</b>}</span>)}
    </div>
  );
}

export function Empty({ children = "없음" }: { children?: ReactNode }) {
  return <div className="grid h-24 place-items-center text-[12.5px] text-muted-foreground">{children}</div>;
}

// ---------------------------------------------------------------- the table
/** A sortable table over shadcn's Table; a row can be pressed. */
export function DataTable<T>({ columns, rows, onRow, picked, initialSort, dense }: {
  columns: ColumnDef<T>[]; rows: T[]; onRow?: (row: T) => void; picked?: (row: T) => boolean; initialSort?: SortingState; dense?: boolean;
}) {
  const [sorting, setSorting] = useState<SortingState>(initialSort ?? []);
  const table = useReactTable({ data: rows, columns, state: { sorting }, onSortingChange: setSorting, getCoreRowModel: getCoreRowModel(), getSortedRowModel: getSortedRowModel() });
  return (
    <Table className="table-fixed">
      <TableHeader className="sticky top-0 z-10 bg-card">
        {table.getHeaderGroups().map((g) => (
          <TableRow key={g.id} className="hover:bg-transparent">
            {g.headers.map((h) => {
              const sort = h.column.getIsSorted();
              const w = h.column.columnDef.size;
              return (
                <TableHead key={h.id} className="h-9 text-[12px] font-normal text-muted-foreground" style={w && w !== 150 ? { width: w } : undefined}>
                  {h.column.getCanSort() ? (
                    <button onClick={h.column.getToggleSortingHandler()} className="inline-flex items-center gap-1 hover:text-foreground">
                      {flexRender(h.column.columnDef.header, h.getContext())}
                      {sort === "asc" ? <ArrowUp className="size-3" /> : sort === "desc" ? <ArrowDown className="size-3" /> : <ArrowUpDown className="size-3 opacity-40" />}
                    </button>
                  ) : flexRender(h.column.columnDef.header, h.getContext())}
                </TableHead>
              );
            })}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.map((r) => (
          <TableRow key={r.id} onClick={() => onRow?.(r.original)} data-state={picked?.(r.original) ? "selected" : undefined} className={cn(onRow && "cursor-pointer")}>
            {r.getVisibleCells().map((c) => <TableCell key={c.id} className={cn("truncate", dense ? "py-1.5" : "py-2.5")}>{flexRender(c.column.columnDef.cell, c.getContext())}</TableCell>)}
          </TableRow>
        ))}
        {!rows.length && <TableRow className="hover:bg-transparent"><TableCell colSpan={columns.length}><Empty /></TableCell></TableRow>}
      </TableBody>
    </Table>
  );
}

// ---------------------------------------------------------------- time and numbers
export const ago = (ms: number | null | undefined, now = Date.now()) => {
  if (!ms) return "";
  const d = now - ms, m = 60e3, h = 60 * m, day = 24 * h;
  if (d < h) return `${Math.max(1, Math.round(d / m))}분 전`;
  if (d < day) return `${Math.round(d / h)}시간 전`;
  return `${Math.round(d / day)}일 전`;
};
export const clock = (ms: number | null | undefined) => {
  if (!ms) return "";
  const d = new Date(ms), z = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}/${d.getDate()} ${z(d.getHours())}:${z(d.getMinutes())}`;
};
export const num = (n: number) => (n >= 10000 ? `${(n / 1000).toFixed(n >= 100000 ? 0 : 1)}K` : n.toLocaleString());
export const pct = (a: number, b: number) => (b ? Math.round((a / b) * 100) : 0);
export const shortUser = (email: string) => email.replace(/@.*/, "");
