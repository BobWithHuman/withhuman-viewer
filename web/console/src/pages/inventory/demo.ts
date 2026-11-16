// What the page shows when it is opened on its own: an inventory of the same shape the app
// sends, made the same way every time from a fixed seed.

import { demoMcp } from "./mcp/demo";
import type { Data, Device, Key, KeyEvent, PersonRow, Project, Run, Shadow } from "./model";

const MIN = 60e3, HOUR = 60 * MIN, DAY = 24 * HOUR;

export function demo(now = Date.now()): Data {
  let seed = 20261114;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
  const pick = <T,>(list: T[]) => list[Math.floor(rnd() * list.length)];
  const hex = (n: number) => Array.from({ length: n }, () => Math.floor(rnd() * 16).toString(16)).join("");

  const people: [string, string][] = [
    ["dev.kim", "platform"], ["ops.choi", "platform"], ["sre.han", "platform"], ["infra.yoon", "platform"],
    ["data.jung", "data"], ["ml.seo", "data"], ["analytics.bae", "data"], ["etl.noh", "data"],
    ["web.lee", "web"], ["front.shin", "web"], ["ui.kwon", "web"], ["design.eng.moon", "web"], ["web.ahn", "web"],
    ["ios.park", "mobile"], ["android.lim", "mobile"], ["mobile.jang", "mobile"],
    ["sec.oh", "security"], ["audit.ryu", "security"],
    ["pay.hong", "payments"], ["billing.cho", "payments"], ["ledger.nam", "payments"],
    ["research.go", "lab"], ["intern.song", "lab"],
  ];
  const email = (u: string) => `${u}@example.com`;
  const repos: Record<string, string[]> = {
    platform: ["gateway", "collector", "infra-terraform", "deploy-scripts", "oncall-runbooks"],
    data: ["warehouse-dbt", "feature-store", "airflow-dags", "notebooks", "metrics-api"],
    web: ["console-web", "design-system", "landing", "docs-site", "admin-next"],
    mobile: ["ios-app", "android-app", "shared-kmp"],
    security: ["policy-rules", "detections", "audit-export"],
    payments: ["ledger-core", "billing-service", "pg-migrations", "reconcile-jobs"],
    lab: ["agent-bench", "prompt-evals", "scratch"],
  };
  const hosts = ["mbp", "mac-studio", "linux", "mba", "ws"];
  const models = ["claude-sonnet-4-5", "claude-opus-4-1", "claude-haiku-4-5", "gpt-5-codex"];
  const prompts = [
    "결제 웹훅 재시도 로직에 지수 백오프 추가", "PG 마이그레이션 0042 롤백 스크립트 작성", "대시보드 차트 툴팁이 잘리는 문제 수정",
    "CI에서 flaky한 e2e 테스트 격리", "feature store 스키마에 TTL 컬럼 추가", "iOS 푸시 권한 요청 흐름 정리",
    "terraform 모듈을 리전별로 분리", "README에 로컬 실행 방법 보강", "감사 로그 CSV 내보내기에 기간 필터",
    "airflow DAG 실패 알림을 Slack으로", "디자인 토큰을 Tailwind 4로 이전", "collector 메모리 누수 추적",
    "권한 모드별 세션 집계 쿼리 최적화", "Android 딥링크 라우팅 버그", "정책 규칙 테스트 케이스 추가",
  ];

  const devices: Device[] = [], projects: Project[] = [], sessions: Run[] = [];
  const shadow: Shadow[] = [], keys: Key[] = [], events: KeyEvent[] = [];

  // One or two machines a person; a few installed and never heard, a few heard but never
  // registered (a benchmark's label, an old install).
  people.forEach(([user, team], i) => {
    const n = i % 5 === 0 ? 2 : 1;
    for (let k = 0; k < n; k++) {
      const id = `${team}/${hex(8)}-${hex(4)}-${hex(4)}`;
      const never = i % 9 === 7 && k === 0;
      const stale = i % 6 === 4;
      const last = never ? null : now - (stale ? 9 * DAY + rnd() * 20 * DAY : rnd() < 0.45 ? rnd() * 5 * MIN : rnd() < 0.6 ? rnd() * 20 * HOUR : DAY + rnd() * 5 * DAY);
      const agent = rnd() < 0.82 ? "claude-code" : "codex";
      const os = pick(["macOS 15.6", "macOS 26.1", "Ubuntu 24.04", "macOS 15.5"]);
      devices.push({
        id, host: `${user.split(".").pop()}-${pick(hosts)}${k ? "-2" : ""}`, team, os, os_user: user.split(".").pop()!, agent,
        hook_version: pick(["0.9.4", "0.9.4", "0.9.3", "0.8.7"]), user: email(user),
        registered_at: now - (30 + rnd() * 90) * DAY, first_seen: never ? null : now - (20 + rnd() * 30) * DAY, last,
        sessions: 0, permission_mode: null, engine_mode: never ? null : i % 7 === 3 ? "shadow" : i % 11 === 5 ? "record" : "enforce",
      });
      const mine = repos[team].slice().sort(() => rnd() - 0.5).slice(0, 1 + Math.floor(rnd() * 3));
      for (const r of mine) projects.push({ device: id, project: `/Users/${user.split(".").pop()}/work/${r}`, agent, hook_version: "0.9.4", installed_at: now - (5 + rnd() * 80) * DAY });
      if (never) continue;
      const count = stale ? 1 + Math.floor(rnd() * 2) : 2 + Math.floor(rnd() * 6);
      for (let s = 0; s < count; s++) {
        const live = s === 0 && last! > now - 6 * MIN;
        const seen = s === 0 ? last! : now - (stale ? 9 * DAY : 0) - rnd() * 7 * DAY;
        const mode = team === "data" && rnd() < 0.5 ? "bypassPermissions" : team === "lab" && rnd() < 0.35 ? "bypassPermissions" : rnd() < 0.34 ? "acceptEdits" : rnd() < 0.2 ? "plan" : "default";
        const repo = pick(mine);
        sessions.push({
          id: `${hex(8)}-${hex(4)}-${hex(4)}-${hex(12)}`, device: id, host: devices.at(-1)!.host, user: email(user),
          title: rnd() < 0.8 ? pick(prompts) : null, folder: repo, opens: rnd() < 0.8, source: pick(["startup", "startup", "resume", "compact", "clear"]),
          permission_mode: mode, model: agent === "codex" ? "gpt-5-codex" : pick(models.slice(0, 3)),
          created_at: seen - (5 + rnd() * 90) * MIN, last_seen_at: seen, ended_at: live ? null : rnd() < 0.85 ? seen + 1000 : null,
          end_reason: live ? null : pick(["prompt_input_exit", "prompt_input_exit", "logout", "clear", "other"]),
        });
      }
    }
  });
  for (const d of devices) {
    const mine = sessions.filter((s) => s.device === d.id).sort((a, b) => b.last_seen_at! - a.last_seen_at!);
    d.sessions = mine.length;
    d.permission_mode = mine[0]?.permission_mode ?? null;
  }
  for (const [label, last] of [["bench/swe-agent-run-17", now - 3 * MIN], ["bench/nightly-evals", now - 2 * DAY], ["legacy/old-hook-install", now - 40 * DAY]] as const) {
    devices.push({ id: label, host: null, team: label.split("/")[0], os: null, os_user: null, agent: "claude-code", hook_version: null, user: null, registered_at: null, first_seen: last - DAY, last, sessions: 1, permission_mode: "bypassPermissions", engine_mode: "record" });
    sessions.push({ id: `${hex(8)}-${hex(4)}-${hex(4)}-${hex(12)}`, device: label, host: null, user: null, title: null, folder: null, opens: false, source: "startup", permission_mode: "bypassPermissions", model: "claude-sonnet-4-5", created_at: last - 40 * MIN, last_seen_at: last, ended_at: last > now - 10 * MIN ? null : last + 1000, end_reason: null });
  }
  // Two sessions that came in with no machine ID.
  for (let i = 0; i < 2; i++) sessions.push({ id: `${hex(8)}-${hex(4)}-${hex(4)}-${hex(12)}`, device: null, host: null, user: null, title: null, folder: null, opens: false, source: "startup", permission_mode: "default", model: "claude-sonnet-4-5", created_at: now - (3 + i * 20) * HOUR, last_seen_at: now - (2 + i * 20) * HOUR, ended_at: null, end_reason: null });

  // Projects a machine's check found with no hooks.
  const real = devices.filter((d) => d.registered_at && d.last);
  for (const i of [1, 4, 4, 8, 11, 13, 13, 13, 19, 22]) {
    const d = real[i % real.length];
    const who = d.os_user ?? "dev";
    shadow.push({ device: d.id, host: d.host, user: d.user, agent: rnd() < 0.55 ? "claude-code" : "codex", project: `/Users/${who}/${pick(["side", "tmp", "personal", "sandbox", "clients"])}/${pick(["scraper", "quick-fix", "poc-agent", "migration-test", "invoice-bot", "playground", "llm-notes"])}`, last_seen: now - rnd() * 3 * DAY });
  }
  shadow.sort((a, b) => b.last_seen! - a.last_seen!);

  // Keys: everyone has one; some rotated, two people stopped, one service account.
  const admins = ["sec.oh", "audit.ryu"];
  const rows: PersonRow[] = [];
  const everyone: [string, string | null][] = [...people.map(([u]) => [email(u), "person"] as [string, string]), ["ci-bot@example.com", "service"], ["nightly-evals@example.com", "service"]];
  everyone.forEach(([mail, kind], i) => {
    const mine: Key[] = [];
    const issue = (at: number, note: string | null) => {
      const k: Key = { id: `key_${hex(8)}`, email: mail, created_at: at, created_by: pick(admins), note, last_used_at: at + rnd() * (now - at), revoked_at: null, revoked_by: null, revoke_reason: null, first_denied_at: null, denied_count: null };
      mine.push(k);
      events.push({ kind: "issued", actor: k.created_by, reason: note, email: mail, key_id: k.id, at });
      return k;
    };
    const revoke = (k: Key, at: number, reason: string, denied: number) => {
      Object.assign(k, { revoked_at: at, revoked_by: pick(admins), revoke_reason: reason, last_used_at: at - rnd() * HOUR });
      events.push({ kind: "revoked", actor: k.revoked_by, reason, email: mail, key_id: k.id, at });
      if (denied) {
        Object.assign(k, { first_denied_at: at + 4 * MIN, denied_count: denied });
        for (let n = 0; n < Math.min(denied, 3); n++) events.push({ kind: "denied", actor: null, reason: null, email: mail, key_id: k.id, at: at + (4 + n * 7) * MIN });
      }
    };
    const first = issue(now - (40 + rnd() * 60) * DAY, i % 4 === 0 ? "신규 입사자" : i % 4 === 1 ? "온보딩" : null);
    if (i % 5 === 2) { revoke(first, now - (6 + rnd() * 20) * DAY, pick(["노트북 교체", "분기 키 교체", "키가 저장소에 커밋됨"]), i % 2 ? 0 : 2); issue(first.revoked_at! + 10 * MIN, "교체 발급"); }
    if (i === 6) revoke(first, now - 26 * HOUR, "승인됐지만 main 강제 푸시였음", 14);
    if (i === 22) revoke(first, now - 3 * DAY, "인턴 기간 종료", 5);
    keys.push(...mine);
    const active = mine.filter((k) => !k.revoked_at).length;
    const own = devices.filter((d) => d.user === mail).map((d) => d.id);
    // The CI's key is used from many machines.
    const used = kind === "service" && i % 2 === 1 ? devices.slice(0, 5).map((d) => d.id) : own;
    rows.push({
      email: mail, kind, devices: used, sessions: sessions.filter((s) => s.user === mail).length,
      active_keys: active, revoked_keys: mine.length - active, stopped: active === 0,
      last_used: Math.max(...mine.map((k) => k.last_used_at ?? 0)), denied: mine.reduce((a, k) => a + (k.denied_count ?? 0), 0),
    });
  });
  rows.sort((a, b) => Number(b.stopped) - Number(a.stopped) || (b.last_used ?? 0) - (a.last_used ?? 0));
  events.sort((a, b) => (b.at ?? 0) - (a.at ?? 0));

  return {
    status: "ready", error: null, now, source: { label: "데모 데이터", demo: true },
    admin: { api: "http://127.0.0.1:8787", has_token: false }, actor: "security-admin", reply: null,
    shadow_known: true, devices, projects, shadow, people: rows, keys, events, sessions,
    mcp: demoMcp(real.map((d) => ({ user: d.user!, client: d.host! })), now),
  };
}

/** The demo's answer to a request, as the app would give it: the keys changed, and a `reply`. */
export function demoAnswer(d: Data, m: Record<string, unknown>, req: number): Data {
  const email = String(m.email ?? ""), at = Date.now(), actor = String(m.actor ?? "");
  let keys = d.keys ?? [], people = d.people ?? [], events = d.events ?? [];
  let reply: Data["reply"] = { req, ok: true, done: "admin" };
  if (m.type === "issue") {
    const id = `key_${at.toString(16).slice(-8)}`;
    keys = [...keys, { id, email, created_at: at, created_by: actor, note: String(m.note ?? "") || null, last_used_at: null, revoked_at: null, revoked_by: null, revoke_reason: null, first_denied_at: null, denied_count: null }];
    events = [{ kind: "issued", actor, reason: null, email, key_id: id, at }, ...events];
    if (!people.some((p) => p.email === email)) people = [...people, { email, kind: "person", devices: [], sessions: 0, active_keys: 0, revoked_keys: 0, stopped: false, last_used: null, denied: 0 }];
    reply = { req, ok: true, done: "issued", email, key_id: id, key: `wh_live_${id.slice(4)}${"k3J9xQ7mVt2LpB8sYw4NcR6zHd1FgA5u"}` };
  } else if (m.type === "revoke_key" || m.type === "stop") {
    const hit = (k: { id: string; email: string; revoked_at: number | null }) => !k.revoked_at && (m.type === "stop" ? k.email === email : k.id === m.key_id);
    const n = keys.filter(hit).length;
    events = [...keys.filter(hit).map((k) => ({ kind: "revoked", actor, reason: String(m.reason ?? ""), email, key_id: k.id, at })), ...events];
    keys = keys.map((k) => (hit(k) ? { ...k, revoked_at: at, revoked_by: actor, revoke_reason: String(m.reason ?? "") } : k));
    reply = m.type === "stop" ? { req, ok: true, done: "stopped", email, count: n } : { req, ok: true, done: "revoked", email, key_id: String(m.key_id) };
  }
  people = people.map((p) => {
    const mine = keys.filter((k) => k.email === p.email), active = mine.filter((k) => !k.revoked_at).length;
    return { ...p, active_keys: active, revoked_keys: mine.length - active, stopped: active === 0 && mine.length > 0 };
  });
  let devices = d.devices;
  if (m.type === "owner") {
    devices = devices?.map((x) => (x.id === m.device ? { ...x, owner: email || null } : x));
    reply = { req, ok: true, done: "owner" };
  }
  return { ...d, devices, keys, people, events, reply };
}
