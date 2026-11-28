// The flow page's left panel with the recorded sessions in place of Langflow's component palette,
// drawn with the palette's own parts: the segmented rail (one item, 기록), the search field, group
// labels, menu rows and badges, at the palette's spacing. The sessions the engine refused something
// in stand above as 차단; the rest go by day. A row opens that session's canvas (flow id = session
// id); loading that flow asks the app for the session (src/withhuman/flow/sessions.ts).

import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar";
import { useCustomNavigate } from "@/customization/hooks/use-custom-navigate";
import NoResultsMessage from "@/pages/FlowPage/components/flowSidebarComponent/components/emptySearchComponent";
import SidebarSegmentedNav from "@/pages/FlowPage/components/flowSidebarComponent/components/sidebarSegmentedNav";
import { useSidebarHotkeys } from "@/pages/FlowPage/components/flowSidebarComponent/hooks/useSidebarHotkeys";
import ShortcutDisplay from "@/pages/FlowPage/components/nodeToolbarComponent/shortcutDisplay";
import { cn } from "@/utils/utils";
import { type ShellSession, useWithHuman } from "../bridge";
import { comparePath, useComparePicks } from "../compare/store";
import { flowPath } from "./app";

export const DAYS = ["오늘", "어제", "이번 주", "이전"] as const;

/** The day a session falls in, for the list's groups. */
export function dayOf(at: number) {
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((start(new Date()) - start(new Date(at))) / 86_400_000);
  return days <= 0 ? "오늘" : days === 1 ? "어제" : days < 7 ? "이번 주" : "이전";
}

/** How long ago, as a list says it. */
function ago(at: number) {
  const min = Math.max(0, Math.round((Date.now() - at) / 60_000));
  if (min < 1) return "방금";
  if (min < 60) return `${min}분 전`;
  if (min < 60 * 24) return `${Math.round(min / 60)}시간 전`;
  return `${Math.round(min / 60 / 24)}일 전`;
}

/** The session opened from the list: the canvas moves to its flow the way Langflow's own
 * navigation does; the mock's `GET flows/<id>` asks the app for it. */
export function useOpenSession() {
  const navigate = useCustomNavigate();
  return (id: string) => navigate(flowPath(id));
}

/** A row's menu: 열기, and 비교 - with the session on the canvas (flow page), else picked for the
 * overview's compare (two at a time; the second pick opens the compare view). */
function RowMenu({ s, here }: { s: ShellSession; here: string }) {
  const navigate = useCustomNavigate();
  const picked = useComparePicks((p) => p.ids.includes(s.id));
  const compare = () => {
    if (here && here !== s.id) return navigate(comparePath(here, s.id));
    const { ids, toggle } = useComparePicks.getState();
    const other = ids.find((x) => x !== s.id);
    if (!picked && other) {
      toggle(s.id);
      navigate(comparePath(other, s.id));
    } else toggle(s.id);
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <SidebarMenuAction showOnHover aria-label="메뉴" data-testid={`session-menu-${s.id}`}>
          <ForwardedIconComponent name="Ellipsis" className="h-4 w-4" />
        </SidebarMenuAction>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="right" align="start" className="min-w-[8rem]">
        <DropdownMenuItem onSelect={() => navigate(flowPath(s.id))} className="gap-2">
          <ForwardedIconComponent name="SquareArrowOutUpRight" className="h-4 w-4" />
          열기
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={compare} disabled={here === s.id} className="gap-2" data-testid={`session-compare-${s.id}`}>
          <ForwardedIconComponent name="Columns2" className="h-4 w-4" />
          {here ? "비교" : picked ? "비교 해제" : "비교 선택"}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SessionRow({ s, active, open, here }: { s: ShellSession; active: boolean; open: (id: string) => void; here: string }) {
  const icon = s.denied > 0 ? "OctagonX" : s.review > 0 ? "CircleAlert" : s.outside > 0 ? "TriangleAlert" : "Workflow";
  const tone = s.denied > 0 ? "text-destructive" : s.review > 0 ? "text-accent-amber-foreground" : "text-muted-foreground";
  const tip = (
    <span className="flex flex-col gap-0.5">
      {s.folder && <span className="font-medium">{s.folder}</span>}
      <span className="tabular-nums">호출 {s.calls} · 외부 {s.outside} · {ago(s.at)}</span>
    </span>
  );
  return (
    <SidebarMenuItem>
      <ShadTooltip content={tip} side="right" styleClasses="z-50">
        <SidebarMenuButton
          isActive={active}
          onClick={() => open(s.id)}
          data-testid={`session-${s.id}`}
          className="gap-2 px-2 group-focus-within/menu-item:pr-7 group-hover/menu-item:pr-7"
        >
          <ForwardedIconComponent name={icon} className={cn("h-4 w-4 shrink-0", tone)} />
          <span className="min-w-0 flex-1 truncate">{s.title}</span>
          {s.denied > 0 && (
            <Badge variant="errorStatic" size="tag" className="shrink-0 tabular-nums">
              차단 {s.denied}
            </Badge>
          )}
          {s.review > 0 && (
            <Badge variant="secondaryStatic" size="tag" className="shrink-0 bg-accent-amber tabular-nums text-accent-amber-foreground">
              확인 {s.review}
            </Badge>
          )}
          {s.live && <i aria-label="실행 중" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent-emerald-foreground" />}
        </SidebarMenuButton>
      </ShadTooltip>
      <RowMenu s={s} here={here} />
    </SidebarMenuItem>
  );
}

function Group({ label, list, active, open, here }: { label: string; list: ShellSession[]; active: string; open: (id: string) => void; here: string }) {
  return (
    <SidebarGroup className="p-3 pr-2 pb-0 last:pb-3">
      <SidebarGroupLabel className="flex w-full cursor-default items-center justify-between">
        <span>{label}</span>
        <span className="font-normal tabular-nums text-muted-foreground">{list.length}</span>
      </SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          {list.map((s) => (
            <SessionRow key={s.id} s={s} active={s.id === active} open={open} here={here} />
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

export function SessionSidebar() {
  const { record } = useWithHuman();
  const shell = record?.shell;
  const { id: routeId } = useParams();
  const open = useOpenSession();
  const { open: sidebarOpen, setOpen, activeSection, setActiveSection } = useSidebar();
  const [search, setSearch] = useState("");
  const [focused, setFocused] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null!);
  useSidebarHotkeys({ searchInputRef: searchRef, setOpen, isSearchFocused: focused });

  // Langflow remembers the rail's section; only 기록 is left, so come back to it.
  useEffect(() => {
    if (activeSection !== "components") setActiveSection("components");
  }, [activeSection, setActiveSection]);

  const sessions = shell?.sessions ?? [];
  // The open session: the route's while it names one of the list (a press shows at once), else
  // the one the app says is open. On the overview (no flow route) none is marked.
  const active = !routeId ? "" : sessions.some((s) => s.id === routeId) ? routeId : (shell?.current ?? "");
  const k = search.trim().toLowerCase();
  const list = useMemo(
    () => sessions.filter((s) => !k || s.title.toLowerCase().includes(k) || !!s.folder?.toLowerCase().includes(k)),
    [sessions, k],
  );
  const groups = useMemo(() => {
    const stopped = list.filter((s) => s.denied > 0);
    const days = DAYS.map((d) => [d, list.filter((s) => s.denied === 0 && dayOf(s.at) === d)] as const);
    return [["차단", stopped] as const, ...days].filter(([, l]) => l.length > 0);
  }, [list]);
  const hidden = !sidebarOpen;

  return (
    <Sidebar collapsible="offcanvas" data-testid="shad-sidebar" className="noflow select-none" role="navigation" aria-label="세션">
      <div className="flex h-full">
        <SidebarSegmentedNav hiddenFromTabOrder={hidden} />
        <div inert={hidden} aria-hidden={hidden} className="sidebar-segmented flex h-full w-full flex-col group-data-[collapsible=icon]:hidden">
          <SidebarHeader className="flex w-full flex-col gap-2 border-b group-data-[collapsible=icon]:hidden">
            <div className="relative w-full flex-1">
              <Input
                ref={searchRef}
                type="search"
                icon="Search"
                data-testid="sidebar-search-input"
                inputClassName="w-full rounded-lg bg-background text-sm"
                placeholder="검색"
                aria-label="세션 검색"
                onFocus={() => setFocused(true)}
                onBlur={() => setFocused(false)}
                onChange={(e) => setSearch(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && list[0]) open(list[0].id);
                }}
                value={search}
              />
              {!focused && search === "" && (
                <div className="pointer-events-none absolute inset-y-0 right-2 top-1/2 flex -translate-y-1/2 items-center justify-between gap-2 text-sm text-muted-foreground">
                  <span>
                    <ShortcutDisplay sidebar shortcut="/" />
                  </span>
                </div>
              )}
            </div>
          </SidebarHeader>
          <SidebarContent segmentedSidebar className="gutter-stable flex-1 group-data-[collapsible=icon]:hidden">
            {groups.length > 0 ? (
              groups.map(([label, l]) => <Group key={label} label={label} list={l} active={active} open={open} here={active} />)
            ) : (
              <NoResultsMessage
                message={k ? "결과 없음" : "세션 없음"}
                showClearSearch={!!k}
                clearSearchText="지우기"
                additionalText=""
                onClearSearch={() => setSearch("")}
              />
            )}
          </SidebarContent>
        </div>
      </div>
    </Sidebar>
  );
}

export default SessionSidebar;
