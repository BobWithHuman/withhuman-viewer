// The page's picture: every value as a band running from where it came in, through what kind
// it is, to how it ended (the kit's flow). A band or a node pressed narrows the page.


import { useMemo } from "react";
import { Flow as Bands, type FlowColumn, type FlowRow } from "@/kit/flow";
import { CATEGORIES, CATEGORY, FROM, FROMS, STATUS, STATUSES, categoryLabel, fromOf, type Category, type Filter, type From, type Status, type Trail } from "./data";

const COLUMNS: [FlowColumn, FlowColumn, FlowColumn] = [
  { title: "유입 경로", order: FROMS, label: (k) => FROM[k as From].label, icon: (k) => FROM[k as From].icon },
  { title: "유형", order: CATEGORIES, label: categoryLabel, icon: (k) => CATEGORY[k as Category]?.icon },
  { title: "처리 결과", order: STATUSES, label: (k) => STATUS[k as Status].label, icon: (k) => STATUS[k as Status].icon },
];
/** How much of a band is its end's colour when nothing is picked: the worse the end, the more. */
const WEIGHT: Record<Status, number> = { out: 78, blocked: 54, used: 22, seen: 13 };

export function Flow({ trails, filter, onFilter, height }: { trails: Trail[]; filter: Filter; onFilter: (f: Filter) => void; height: number }) {
  const rows = useMemo<FlowRow[]>(() => trails.map((t) => ({ keys: [fromOf(t), t.category, t.status], n: 1 })), [trails]);
  return (
    <Bands rows={rows} columns={COLUMNS} height={height}
      color={(end) => STATUS[end as Status].color} weight={(end) => WEIGHT[end as Status]}
      picked={[filter.from, filter.category, filter.status]}
      onPick={([from, category, status]) => onFilter({ ...filter, from: from as From | undefined, category, status: status as Status | undefined })} />
  );
}
