// Langflow's account menu as the engine's status: the toggle shows the engine up or down and the
// agents it hears from; the menu, Langflow's own (HeaderMenu, its rows and its theme switch), lists
// the engine, how Calls are judged, each agent, and the ways to the 훅 and 엔진 settings.
// After setup, what is still missing shows here: a dot on the toggle and a row on top of the
// menu — 키 등록 (Jev picked, no key kept; amber) and 훅 연결 (no agent's hook set up; red).
// 둘러보기 plays the first session's tour again (tour/).

import { ChevronsUpDown } from "lucide-react";
import ThemeButtons from "@/components/core/appHeaderComponent/components/ThemeButtons";
import {
  HeaderMenu,
  HeaderMenuItemButton,
  HeaderMenuItems,
} from "@/components/core/appHeaderComponent/components/HeaderMenu";
import { DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useCustomNavigate } from "@/customization/hooks/use-custom-navigate";
import { cn } from "@/utils/utils";
import { send, useWithHuman } from "../bridge";
import { startTour } from "../tour/store";
import { AgentIcon } from "./agent-icon";
import { PAGES } from "./app";

const Dot = ({ up, className }: { up: boolean; className?: string }) => (
  <i className={cn("inline-block h-2 w-2 shrink-0 rounded-full", up ? "bg-accent-emerald-foreground" : "bg-destructive", className)} />
);

export function EngineMenu() {
  const { record, views } = useWithHuman();
  const navigate = useCustomNavigate();
  const shell = record?.shell;
  const engine = shell?.engine ?? { label: "엔진", up: false, jev: false };
  const agents = shell?.agents ?? [];
  const openEngine = () => (shell?.own ? navigate(PAGES.engine) : send({ do: "settings" }));
  // Jev picked with no key: from the 엔진 page's data while it was open, else from the shell when
  // the app sends `jev_on` / `has_key` there.
  const missingKey =
    (!!views.engine && views.engine.jev && !views.engine.has_key) || (engine.jev_on === true && engine.has_key === false);
  // No agent's hook set up: nothing reaches the engine.
  const noHook = !!shell && shell.page !== "onboard" && agents.length === 0 && !(views.hooks?.installs.length);
  const nudge = noHook ? "bg-destructive" : missingKey ? "bg-accent-amber-foreground" : null;

  return (
    <HeaderMenu>
      <DropdownMenuTrigger
        className="inline-flex items-center justify-center rounded-md pl-1 pr-1"
        data-testid="user_menu_button"
        id="user_menu_button"
        aria-label={engine.label}
      >
        <div className="group relative flex h-7 items-center gap-2 rounded-md px-2 transition-colors hover:bg-muted">
          {nudge && (
            <i className={cn("absolute -right-0.5 -top-0.5 h-2 w-2 rounded-full ring-2 ring-background", nudge)} data-testid="engine-menu-nudge" />
          )}
          <Dot up={engine.up} className={engine.up ? "animate-pulse" : undefined} />
          <span className="max-w-[10rem] truncate text-sm text-foreground">{engine.label}</span>
          {engine.jev && <span className="rounded-sm bg-muted px-1.5 text-xs font-medium text-muted-foreground">Jev</span>}
          {agents.length > 0 && (
            <span className="flex items-center gap-1">
              {agents.map((a) => (
                <AgentIcon key={a.name} kind={a.logo} className={cn("h-4 w-4 rounded-[3px]", !a.up && "opacity-40 grayscale")} />
              ))}
            </span>
          )}
          <ChevronsUpDown className="h-[14px] w-[14px] text-muted-foreground group-hover:text-primary" />
        </div>
      </DropdownMenuTrigger>
      <HeaderMenuItems position="right" classNameSize="w-[272px]">
        <div className="divide-y divide-foreground/10">
          <header className="flex w-full items-center justify-between gap-2 px-4 py-3">
            <span className="flex min-w-0 items-center gap-2">
              <Dot up={engine.up} />
              <span className="truncate text-base font-semibold">{engine.label}</span>
            </span>
            <span className={cn("shrink-0 text-xs", engine.up ? "text-accent-emerald-foreground" : "text-destructive")}>
              {engine.up ? "실행 중" : "멈춤"}
            </span>
          </header>

          {(missingKey || noHook) && (
            <div>
              {noHook && (
                <HeaderMenuItemButton onClick={() => navigate(PAGES.hooks)} icon="Webhook">
                  <span className="flex items-center gap-2 font-medium" data-testid="menu_nudge_hooks">
                    <i className="h-2 w-2 rounded-full bg-destructive" />
                    훅 연결
                  </span>
                </HeaderMenuItemButton>
              )}
              {missingKey && (
                <HeaderMenuItemButton onClick={openEngine} icon="KeyRound">
                  <span className="flex items-center gap-2 font-medium" data-testid="menu_nudge_key">
                    <i className="h-2 w-2 rounded-full bg-accent-amber-foreground" />
                    키 등록
                  </span>
                </HeaderMenuItemButton>
              )}
            </div>
          )}

          <div className="flex h-[44px] items-center justify-between px-4 text-sm">
            <span>판정</span>
            <span className={cn("text-xs", missingKey ? "text-accent-amber-foreground" : "text-muted-foreground")}>
              {engine.jev ? "Jev" : missingKey ? "Jev · 키 없음" : "로컬 규칙"}
            </span>
          </div>

          {agents.length > 0 && (
            <div className="py-1">
              {agents.map((a) => (
                <div key={a.name} className="flex h-9 items-center gap-2 px-4 text-sm">
                  <AgentIcon kind={a.logo} />
                  <span className="min-w-0 flex-1 truncate">{a.name}</span>
                  <Dot up={a.up} className="h-1.5 w-1.5" />
                </div>
              ))}
            </div>
          )}

          <div>
            <HeaderMenuItemButton onClick={() => navigate(PAGES.hooks)} icon="Webhook">
              <span data-testid="menu_hooks_button">훅</span>
            </HeaderMenuItemButton>
            <HeaderMenuItemButton onClick={openEngine} icon="Server">
              <span data-testid="menu_settings_button" id="menu_settings_button">
                엔진
              </span>
            </HeaderMenuItemButton>
            <HeaderMenuItemButton onClick={startTour} icon="Compass">
              <span data-testid="menu_tour_button">둘러보기</span>
            </HeaderMenuItemButton>
          </div>

          <div className="flex items-center justify-between px-4 py-[6.5px] text-sm">
            <span>테마</span>
            <div className="relative top-[1px] float-right">
              <ThemeButtons />
            </div>
          </div>
        </div>
      </HeaderMenuItems>
    </HeaderMenu>
  );
}

export default EngineMenu;
