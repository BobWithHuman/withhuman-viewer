// 보안 현황's picture: every Call as a band from the kind of action it is, through what made the
// agent take it, to how the engine decided it (the kit's flow). A band or a node pressed narrows
// the page.

import { useMemo } from "react";
import { Flow as Bands, type FlowColumn, type FlowRow } from "@/kit/flow";
import { ACTION, ACTIONS, END, ENDS, type Filter, type FlowCount, HOUR, SOURCE, SOURCES, SOURCE_ICON } from "./data";

const COLUMNS: [FlowColumn, FlowColumn, FlowColumn] = [
  { title: "행동 종류", order: ACTIONS, label: (k) => ACTION[k]?.label ?? k, icon: (k) => ACTION[k]?.icon },
  { title: "정한 쪽", order: SOURCES, label: (k) => SOURCE[k] ?? k, icon: (k) => SOURCE_ICON[k] },
  { title: "판정", order: ENDS, label: (k) => END[k]?.label ?? k, icon: (k) => END[k]?.icon },
];

/** The Calls of a stretch, on one machine when one is picked, as the flow's rows; `only` keeps
 * the decisions named. */
export function flowRows(flow: FlowCount[], from: number, to: number, machine: number | null, only?: string[]): FlowRow[] {
  return flow
    .filter(([hour, m, , , verdict]) => hour + HOUR > from && hour < to && (machine === null || m === machine) && (!only || only.includes(verdict)))
    .map(([, , action, source, verdict, n]) => ({ keys: [action, source, verdict], n }));
}

export function ActionFlow({ rows, filter, onFilter }: { rows: FlowRow[]; filter: Filter; onFilter: (f: Filter) => void }) {
  const picked = useMemo<[string | undefined, string | undefined, string | undefined]>(() => [filter.action, filter.source, filter.verdict], [filter.action, filter.source, filter.verdict]);
  return (
    <Bands rows={rows} columns={COLUMNS} unit="건" left={156} right={128} scale="sqrt"
      color={(end) => END[end]?.color ?? "var(--info)"} weight={(end) => END[end]?.weight ?? 12}
      picked={picked} onPick={([action, source, verdict]) => onFilter({ ...filter, action, source, verdict })} />
  );
}
