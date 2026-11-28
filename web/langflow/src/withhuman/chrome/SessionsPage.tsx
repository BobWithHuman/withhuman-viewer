// The flows list page (`/all`, `/all/folder/<id>`, `/flows`) as the recorded sessions' overview,
// drawn with the parts of Langflow's own list page (src/pages/MainPage: HomePage's header, its
// tab bar, its search field and list/grid switch, ListComponent's card rows, EmptyFolder's empty
// state) and framed like the flow page: the same left panel (SessionSidebar) in the same
// SidebarProvider, so both pages share one list of sessions on the left.
//
// No project, template, import or flow actions: the one fixed project is 기록 and its flows are the
// app's sessions (read-only). The rows come straight from `shell.sessions`.

import { useMemo, useState } from "react";
import { Outlet } from "react-router-dom";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import LoadingComponent from "@/components/common/loadingComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { ENABLE_NEW_SIDEBAR } from "@/customization/feature-flags";
import { useCustomNavigate } from "@/customization/hooks/use-custom-navigate";
import { useDocumentTitle } from "@/hooks/use-document-title";
import { useIsMobile } from "@/hooks/use-mobile";
import { FlowSearchProvider } from "@/pages/FlowPage/components/flowSidebarComponent";
import { cn } from "@/utils/utils";
import { send, type Shell, type ShellSession, useWithHuman } from "../bridge";
import { ago } from "../flow/sessions";
import { flowPath, PAGES } from "./app";
import { DAYS, dayOf, SessionSidebar } from "./SessionSidebar";

/** The overview's routes inside the same frame as the flow page: our session list on the left. */
export function SessionsLayout() {
  const isMobile = useIsMobile();
  return (
    <div className="flow-page-positioning">
      <div className="flex h-full overflow-hidden">
        <SidebarProvider width="20rem" defaultOpen={!isMobile} segmentedSidebar={ENABLE_NEW_SIDEBAR}>
          <FlowSearchProvider>
            <SessionSidebar />
            <main className="flex min-w-0 flex-1 overflow-hidden">
              <div className="relative mx-auto flex h-full w-full flex-col overflow-hidden">
                <Outlet />
              </div>
            </main>
          </FlowSearchProvider>
        </SidebarProvider>
      </div>
    </div>
  );
}

type Filter = "all" | "denied" | "review" | "outside";
const FILTERS: { id: Filter; label: string; test: (s: ShellSession) => boolean }[] = [
  { id: "all", label: "전체", test: () => true },
  { id: "denied", label: "차단", test: (s) => s.denied > 0 },
  { id: "review", label: "확인", test: (s) => s.review > 0 },
  { id: "outside", label: "외부 지시", test: (s) => s.outside > 0 },
];

/** Opens the engine's settings: a page here for the app's own engine, else the app's sheet. */
export function useOpenEngine(shell?: Shell) {
  const navigate = useCustomNavigate();
  return () => (shell?.own ? navigate(PAGES.engine) : send({ do: "settings" }));
}

/** ListComponent's swatch tile, toned by what the engine did in the session. */
function Swatch({ s }: { s: ShellSession }) {
  const [icon, tone] =
    s.denied > 0
      ? ["OctagonX", "bg-error-background text-error-foreground"]
      : s.review > 0
        ? ["CircleAlert", "bg-accent-amber text-accent-amber-foreground"]
        : s.outside > 0
          ? ["TriangleAlert", "bg-accent-amber text-accent-amber-foreground"]
          : ["Workflow", "bg-muted text-muted-foreground"];
  return (
    <div className={cn("flex items-center justify-center rounded-lg p-1.5", tone)}>
      <ForwardedIconComponent name={icon} aria-hidden="true" className="flex h-5 w-5 items-center justify-center" />
    </div>
  );
}

function Counts({ s }: { s: ShellSession }) {
  return (
    <>
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
      {s.outside > 0 && (
        <Badge variant="secondaryStatic" size="tag" className="shrink-0 tabular-nums">
          외부 지시 {s.outside}
        </Badge>
      )}
    </>
  );
}

const Live = () => <i aria-label="실행 중" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent-emerald-foreground" />;

/** One session as ListComponent's row (list view) or card (grid view). */
function SessionCard({ s, grid, open }: { s: ShellSession; grid: boolean; open: (id: string) => void }) {
  const folder = s.folder?.split("/").filter(Boolean).pop();
  return (
    <Card
      className={cn(
        "group relative flex cursor-pointer rounded-lg bg-background shadow-none hover:bg-muted",
        grid ? "flex-col gap-3 border px-4 py-3" : "flex-row justify-between border-none px-4 py-3",
      )}
      data-testid="list-card"
    >
      <button
        type="button"
        className="absolute inset-0 z-0 rounded-lg focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-2"
        onClick={() => open(s.id)}
        aria-label={s.title}
        data-testid={`session-card-${s.id}`}
      />
      <div className="pointer-events-none relative z-10 flex min-w-0 items-center gap-4">
        <Swatch s={s} />
        <div className="flex min-w-0 flex-col justify-start">
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
            <span className="truncate text-sm font-semibold">{s.title}</span>
            {s.live && <Live />}
          </div>
          <div className="flex min-w-0 gap-1.5 text-xs text-muted-foreground">
            {folder && <span className="truncate">{folder}</span>}
            {folder && <span aria-hidden="true">·</span>}
            <span className="shrink-0">{ago(s.at)}</span>
          </div>
        </div>
      </div>
      <div className={cn("pointer-events-none relative z-10 flex items-center gap-1.5", grid ? "pl-[3.25rem]" : "ml-5 shrink-0")}>
        <Counts s={s} />
        <span className="ml-1 flex items-center gap-1 text-xs tabular-nums text-muted-foreground">
          <ForwardedIconComponent name="Wrench" className="h-3.5 w-3.5" aria-hidden="true" />
          {s.calls}
        </span>
      </div>
    </Card>
  );
}

/** EmptyFolder's empty state (Langflow's list page) with WithHuman's ways out. */
function Empty({ shell }: { shell?: Shell }) {
  const navigate = useCustomNavigate();
  const openEngine = useOpenEngine(shell);
  const up = !!shell?.engine.up;
  return (
    <div className="m-0 flex w-full justify-center">
      <div className="absolute top-1/2 flex w-full -translate-y-1/2 flex-col items-center justify-center gap-2">
        {up ? (
          <ForwardedIconComponent name="Workflow" className="h-8 w-8 text-muted-foreground" aria-hidden="true" />
        ) : (
          <i className="h-2.5 w-2.5 rounded-full bg-destructive" aria-hidden="true" />
        )}
        <h3 className="pb-5 pt-3 font-chivo text-2xl font-semibold" data-testid="mainpage_title">
          {up ? "세션 없음" : shell?.engine.label ?? "엔진"}
        </h3>
        <div className="flex items-center gap-2">
          {up && shell?.own && (
            <Button variant="default" onClick={() => send({ do: "try" })} data-testid="empty-try">
              <ForwardedIconComponent name="Play" aria-hidden="true" className="h-4 w-4" />
              <span className="whitespace-nowrap font-semibold">체험</span>
            </Button>
          )}
          {!up && (
            <Button variant="default" onClick={openEngine} data-testid="empty-engine">
              <ForwardedIconComponent name="Server" aria-hidden="true" className="h-4 w-4" />
              <span className="whitespace-nowrap font-semibold">설정</span>
            </Button>
          )}
          <Button variant="outline" onClick={() => navigate(PAGES.hooks)} data-testid="empty-hooks">
            <ForwardedIconComponent name="Webhook" aria-hidden="true" className="h-4 w-4" />
            <span className="whitespace-nowrap font-semibold">훅</span>
          </Button>
        </div>
      </div>
    </div>
  );
}

/** The engine down while sessions are listed: its label and the way to its settings. */
function EngineDown({ shell }: { shell: Shell }) {
  const openEngine = useOpenEngine(shell);
  return (
    <div className="mb-4 flex items-center gap-2 rounded-lg border border-destructive/40 bg-error-background px-3 py-2 text-sm text-error-foreground">
      <i className="h-2 w-2 shrink-0 rounded-full bg-destructive" aria-hidden="true" />
      <span className="min-w-0 flex-1 truncate font-medium">{shell.engine.label}</span>
      <Button variant="outline" size="sm" className="h-7" onClick={openEngine} data-testid="engine-down-settings">
        설정
      </Button>
    </div>
  );
}

/** The sessions overview: HomePage's header, tabs as the verdict filter, search, list/grid. */
export function SessionsOverview() {
  const shell = useWithHuman().record?.shell;
  const navigate = useCustomNavigate();
  const open = (id: string) => navigate(flowPath(id));
  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  const [view, setView] = useState<"list" | "grid">(() => {
    try {
      return localStorage.getItem("view") === "grid" ? "grid" : "list";
    } catch {
      return "list";
    }
  });
  const pickView = (v: "list" | "grid") => {
    setView(v);
    try {
      localStorage.setItem("view", v);
    } catch {}
  };
  useDocumentTitle("기록");

  const sessions = useMemo(() => [...(shell?.sessions ?? [])].sort((a, b) => b.at - a.at), [shell?.sessions]);
  const k = search.trim().toLowerCase();
  const found = useMemo(
    () => sessions.filter((s) => !k || s.title.toLowerCase().includes(k) || !!s.folder?.toLowerCase().includes(k)),
    [sessions, k],
  );
  const counts = useMemo(() => Object.fromEntries(FILTERS.map((f) => [f.id, found.filter(f.test).length])) as Record<Filter, number>, [found]);
  const list = useMemo(() => found.filter(FILTERS.find((f) => f.id === filter)!.test), [found, filter]);
  const groups = useMemo(
    () => DAYS.map((d) => [d, list.filter((s) => dayOf(s.at) === d)] as const).filter(([, l]) => l.length > 0),
    [list],
  );
  const none = sessions.length === 0;

  return (
    <div className="flex h-full w-full flex-col overflow-y-auto" data-testid="cards-wrapper">
      <div className="flex h-full w-full flex-col 3xl:container">
        <div className="flex flex-1 flex-col justify-start p-4">
          <div className="flex h-full flex-col justify-start">
            <div className="flex items-center pb-4 text-sm font-medium" data-testid="mainpage_title">
              <div className="h-7 w-10 transition-all group-data-[open=true]/sidebar-wrapper:md:w-0 lg:hidden">
                <div className="relative left-0 opacity-100 transition-all group-data-[open=true]/sidebar-wrapper:md:opacity-0">
                  <SidebarTrigger>
                    <ForwardedIconComponent name="PanelLeftOpen" aria-hidden="true" />
                  </SidebarTrigger>
                </div>
              </div>
              기록
            </div>
            {!shell ? (
              <div className="flex flex-1 items-center justify-center">
                <LoadingComponent remSize={8} />
              </div>
            ) : none ? (
              <Empty shell={shell} />
            ) : (
              <>
                {shell && !shell.engine.up && <EngineDown shell={shell} />}
                <div className="flex pb-4" role="tablist" aria-label="판정">
                  {FILTERS.map((f) => (
                    <Button
                      key={f.id}
                      unstyled
                      role="tab"
                      aria-selected={filter === f.id}
                      data-testid={`filter-${f.id}`}
                      onClick={() => setFilter(f.id)}
                      className={`border-b ${
                        filter === f.id
                          ? "border-b-2 border-foreground text-foreground"
                          : "border-border text-muted-foreground hover:text-foreground"
                      } text-nowrap px-2 pb-2 pt-1 text-mmd`}
                    >
                      <div className={cn("flex items-center gap-1.5", filter === f.id && "-mb-px")}>
                        {f.label}
                        <span className="tabular-nums text-muted-foreground">{counts[f.id]}</span>
                      </div>
                    </Button>
                  ))}
                  <div className="w-full border-b dark:border-border" />
                </div>
                <div className="flex justify-between">
                  <div className="flex w-full xl:w-5/12">
                    <Input
                      icon="Search"
                      data-testid="search-store-input"
                      type="text"
                      placeholder="검색"
                      aria-label="세션 검색"
                      className="mr-2 !text-mmd"
                      inputClassName="!text-mmd"
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    <div className="relative mr-2 flex h-fit rounded-lg border border-muted bg-muted">
                      <div
                        className={`absolute top-[2px] h-[32px] w-8 transform rounded-md bg-background shadow-md transition-transform duration-300 ${
                          view === "list" ? "left-[2px] translate-x-0" : "left-[6px] translate-x-full"
                        }`}
                      />
                      {(["list", "grid"] as const).map((v) => (
                        <ShadTooltip key={v} content={v === "list" ? "목록" : "카드"} side="bottom">
                          <Button
                            unstyled
                            size="icon"
                            className={`group relative z-10 m-[2px] flex-1 rounded-lg p-2 ${
                              view === v ? "text-foreground" : "text-muted-foreground hover:bg-muted"
                            }`}
                            onClick={() => pickView(v)}
                            aria-label={v === "list" ? "목록" : "카드"}
                            aria-pressed={view === v}
                          >
                            <ForwardedIconComponent
                              name={v === "list" ? "Menu" : "LayoutGrid"}
                              aria-hidden="true"
                              className="h-4 w-4 group-hover:text-foreground"
                            />
                          </Button>
                        </ShadTooltip>
                      ))}
                    </div>
                  </div>
                  <div className="flex items-center gap-3 whitespace-nowrap text-xs tabular-nums text-muted-foreground">
                    <span>세션 {list.length}</span>
                    <span>호출 {list.reduce((n, s) => n + s.calls, 0)}</span>
                  </div>
                </div>
                {groups.length === 0 ? (
                  <div className="pt-24 text-center text-sm text-secondary-foreground">결과 없음</div>
                ) : (
                  groups.map(([day, l]) => (
                    <section key={day} className="mt-4" aria-label={day}>
                      <div className="flex items-center gap-2 px-1 pb-1 text-xs font-medium text-muted-foreground">
                        <span>{day}</span>
                        <span className="font-normal tabular-nums">{l.length}</span>
                      </div>
                      <div className={view === "grid" ? "grid grid-cols-1 gap-1 md:grid-cols-2 lg:grid-cols-3" : "flex flex-col gap-1"}>
                        {l.map((s) => (
                          <SessionCard key={s.id} s={s} grid={view === "grid"} open={open} />
                        ))}
                      </div>
                    </section>
                  ))
                )}
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default SessionsOverview;
