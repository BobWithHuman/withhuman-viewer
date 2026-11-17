// The Calls worth a look over the period, as bars stacked by verdict; dragging across the bars
// (or pressing one day) makes that stretch the period.

import { useMemo, useState } from "react";
import { Bar, BarChart, CartesianGrid, ReferenceArea, XAxis, YAxis } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { Empty, VERDICT } from "@/kit";
import { type Call, HOUR, VERDICTS, axis, bucketOf, moment, tick } from "./data";

const CONFIG: ChartConfig = {
  deny: { label: "차단", color: VERDICT.deny.color },
  review: { label: "사용자 확인", color: VERDICT.review.color },
  allow: { label: "고위험 허용", color: VERDICT.allow.color },
};

export function Trend({ calls, from, to, onRange }: {
  calls: Call[]; from: number; to: number; onRange: (range: [number, number]) => void;
}) {
  const { step, starts } = useMemo(() => axis(from, to), [from, to]);
  const rows = useMemo(() => {
    const rows = starts.map((t) => ({ t, deny: 0, review: 0, allow: 0 }));
    for (const c of calls) {
      const i = c.at === null ? -1 : bucketOf(starts, c.at);
      if (i < 0) continue;
      rows[i][c.verdict]++;
    }
    return rows;
  }, [calls, starts]);
  // The bars a drag has covered so far, as where it began and where it is.
  const [drag, setDrag] = useState<[number, number] | null>(null);
  const at = (s: { activeTooltipIndex?: unknown }) => (s.activeTooltipIndex === undefined || s.activeTooltipIndex === null ? null : Number(s.activeTooltipIndex));

  if (!rows.length) return <Empty />;
  const lo = drag ? Math.min(...drag) : 0, hi = drag ? Math.max(...drag) : 0;
  return (
    <ChartContainer config={CONFIG} className="aspect-auto h-full w-full cursor-crosshair select-none [&_*:focus]:outline-none">
      <BarChart
        data={rows} margin={{ top: 8, right: 4, left: 0, bottom: 0 }} barCategoryGap="24%"
        onMouseDown={(s) => { const i = at(s); if (i !== null) setDrag([i, i]); }}
        onMouseMove={(s) => { const i = at(s); if (drag && i !== null && i !== drag[1]) setDrag([drag[0], i]); }}
        onMouseLeave={() => setDrag(null)}
        onMouseUp={() => {
          if (!drag) return;
          setDrag(null);
          // One hour pressed is too little to be a period; a day or a week pressed is one.
          if (lo === hi && step === "hour") return;
          const end = hi + 1 < starts.length ? starts[hi + 1] : step === "hour" ? starts[hi] + HOUR : to;
          onRange([Math.max(starts[lo], Number.isFinite(from) ? from : starts[lo]), Math.min(end, to)]);
        }}
      >
        <CartesianGrid vertical={false} />
        <XAxis dataKey="t" tickFormatter={(t: number) => tick(t, step)} tickLine={false} axisLine={false} tickMargin={6} minTickGap={28} />
        <YAxis allowDecimals={false} width={34} tickLine={false} axisLine={false} tickMargin={4} />
        <ChartTooltip cursor={{ fill: "var(--foreground)", fillOpacity: 0.05 }} content={<ChartTooltipContent labelFormatter={(_, p) => moment(Number(p?.[0]?.payload?.t), step)} />} />
        {VERDICTS.map((k) => <Bar key={k} dataKey={k} stackId="calls" fill={`var(--color-${k})`} maxBarSize={64} isAnimationActive={false} />)}
        {drag && <ReferenceArea x1={rows[lo].t} x2={rows[hi].t} fill="var(--primary)" fillOpacity={0.16} stroke="var(--primary)" strokeOpacity={0.5} />}
      </BarChart>
    </ChartContainer>
  );
}
