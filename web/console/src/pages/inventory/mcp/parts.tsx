// Small parts the MCP sheets share: the calls of a tool that were held or
// blocked, and the last 30 days as a chart.

import { Bar, BarChart, XAxis } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { num } from "@/kit";
import { cn } from "@/lib/utils";
import { DAY } from "../model";
import type { Tool } from "./types";

/** How many of a tool's calls the engine held, and how many it blocked, in their colours. */
export function Held({ t, className }: { t: Pick<Tool, "review" | "deny">; className?: string }) {
  if (!t.review && !t.deny) return null;
  const mark = "tnum rounded px-1 text-[10.5px] leading-[16px] font-semibold";
  return (
    <span className={cn("flex shrink-0 items-center gap-0.5", className)}>
      {t.review > 0 && <span className={cn(mark, "bg-review/15 text-review")}>{num(t.review)}</span>}
      {t.deny > 0 && <span className={cn(mark, "bg-deny/15 text-deny")}>{num(t.deny)}</span>}
    </span>
  );
}

type Series = { key: string; label: string; color: string };

/** The last 30 days as stacked bars, the oldest first. */
export function DaysChart({ rows, series, now, className }: { rows: Record<string, number>[]; series: Series[]; now: number; className?: string }) {
  const n = rows.length;
  const data = rows.map((r, i) => {
    const d = new Date(now - (n - 1 - i) * DAY);
    return { ...r, day: i === n - 1 ? "오늘" : `${d.getMonth() + 1}/${d.getDate()}` };
  });
  const config: ChartConfig = Object.fromEntries(series.map((s) => [s.key, { label: s.label, color: s.color }]));
  const ticks = [0, 7, 14, 21, n - 1].map((i) => data[i]?.day).filter(Boolean);
  return (
    <ChartContainer config={config} className={cn("aspect-auto w-full", className)}>
      <BarChart data={data} margin={{ top: 4, right: 10, bottom: 0, left: 10 }} barCategoryGap={1.5}>
        <XAxis dataKey="day" ticks={ticks} interval={0} tickLine={false} axisLine={false} tickMargin={4} height={20} tick={{ fontSize: 11 }} />
        <ChartTooltip cursor={{ fill: "rgb(255 255 255 / 0.06)" }} content={<ChartTooltipContent />} />
        {series.map((s, k) => (
          <Bar key={s.key} dataKey={s.key} stackId="calls" fill={`var(--color-${s.key})`} radius={k === series.length - 1 ? [2, 2, 0, 0] : 0} isAnimationActive={false} />
        ))}
      </BarChart>
    </ChartContainer>
  );
}

