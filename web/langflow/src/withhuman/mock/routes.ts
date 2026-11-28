// Request handler of the mock backend. `handle` answers one request the way
// the Langflow API would, from in-memory state. Unknown routes get a harmless
// 200 (an empty list for GET, an empty object otherwise) and are logged once.

import { flowFor, isSession, sessionFlows } from "../flow/sessions";
import {
  buildAllTypes,
  initialProjects,
  PROJECT_ID,
  PROJECT_NAME,
  USER,
} from "./data";

type Json = any;
export type MockResponse = { status: number; data: Json };
type Answer = MockResponse | Promise<MockResponse>;
export type MockRequest = {
  method: string;
  path: string; // path below /api/v1 or /api/v2, without leading slash, e.g. "flows/123"
  version: "v1" | "v2" | "root";
  query: URLSearchParams;
  body: Json;
};

// `flows` holds only flows made in the page (Langflow's "new flow"); the sessions come from the
// app on every request (../flow/sessions.ts).
const state = {
  flows: [] as Json[],
  projects: initialProjects(),
  variables: [] as Json[],
  all: null as Json,
};

const ok = (data: Json): MockResponse => ({ status: 200, data });
const notFound = (): MockResponse => ({
  status: 404,
  data: { detail: "Not found" },
});

const NO_ENGINE: MockResponse = {
  status: 503,
  data: { detail: "No Langflow backend" },
};

const seenUnknown = new Set<string>();

function uuid(): string {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : "xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx".replace(/x/g, () =>
        Math.floor(Math.random() * 16).toString(16),
      );
}

function paginate(items: Json[], query: URLSearchParams) {
  const size = Number(query.get("size") ?? 12) || 12;
  const page = Number(query.get("page") ?? 1) || 1;
  return {
    items: items.slice((page - 1) * size, page * size),
    total: items.length,
    page,
    size,
    pages: Math.max(1, Math.ceil(items.length / size)),
  };
}

/** The sessions, newest first, then the flows made here. */
const allFlows = (): Json[] => [...sessionFlows(), ...state.flows];
const findFlow = (id: string) => allFlows().find((f) => f.id === id);

function flowsOf(folderId: string | null, query: URLSearchParams): Json[] {
  let flows = allFlows().filter((f) => !folderId || f.folder_id === folderId);
  if (query.get("is_component") === "true")
    flows = flows.filter((f) => f.is_component);
  if (query.get("is_flow") === "true")
    flows = flows.filter((f) => !f.is_component);
  const search = query.get("search")?.toLowerCase();
  if (search) flows = flows.filter((f) => f.name.toLowerCase().includes(search));
  return flows;
}

function projectWithFlows(p: Json) {
  const flows = allFlows().filter((f) => f.folder_id === p.id);
  return {
    ...p,
    flows: flows.filter((f) => !f.is_component).map((f) => f.id),
    components: flows.filter((f) => f.is_component).map((f) => f.id),
  };
}

const config = {
  type: "full",
  frontend_timeout: 0,
  max_file_size_upload: 1024,
  event_delivery: "polling",
  voice_mode_available: false,
  allow_custom_components: true,
  substitute_outdated_component_code: false,
  catalog_governance_enabled: false,
  mcp_base_url: "",
  enable_extension_reload: false,
  auto_saving: true,
  auto_saving_interval: 1000,
  health_check_max_retries: 5,
  feature_flags: {},
  webhook_polling_interval: 5000,
  serialization_max_items_length: 100,
  webhook_auth_enable: false,
  default_folder_name: PROJECT_NAME,
  hide_getting_started_progress: true,
  embedded_mode: false,
  hide_logout_button: false,
  hide_new_project_button: false,
  hide_new_flow_button: false,
  hide_starter_projects: true,
  mcp_servers_locked: false,
  custom_component_admin_only: false,
  a2a_enabled: false,
  agentic_experience: false,
  assistant_max_message_length: 10000,
  local_vector_store_available: false,
  configured_vector_store_backends: [],
  blocked_component_types: [],
};

function findComponent(code: string | undefined, module: string | undefined): Json {
  state.all ??= buildAllTypes();
  for (const category of Object.values<Json>(state.all)) {
    for (const node of Object.values<Json>(category)) {
      if (code && node?.template?.code?.value === code) return node;
    }
  }
  for (const flow of state.flows) {
    for (const n of flow?.data?.nodes ?? []) {
      if (code && n?.data?.node?.template?.code?.value === code) return n.data.node;
    }
  }
  for (const category of Object.values<Json>(state.all)) {
    for (const node of Object.values<Json>(category)) {
      if (module && node?.metadata?.module === module) return node;
    }
  }
  return null;
}

function updatedComponent(body: Json): Json {
  const metadata = body?.frontend_node?.metadata;
  const base = findComponent(body?.code, metadata?.module);
  const template = structuredClone(body?.template ?? base?.template ?? {});
  if (body?.field && template[body.field] && "field_value" in body) {
    template[body.field].value = body.field_value;
  }
  return {
    ...(base ? structuredClone(base) : {}),
    ...(metadata ? { metadata } : {}),
    template,
    ...(body?.tool_mode !== undefined ? { tool_mode: body.tool_mode } : {}),
  };
}

/** One flow: a session (asked of the app if it is not the one open) or one made here. */
async function oneFlow(id: string): Promise<MockResponse> {
  if (isSession(id)) {
    // A session the app could not hand (slow, or gone) opens empty rather than failing: Langflow
    // draws the flow page only around a flow, so an error would take the session list with it.
    const empty = () => {
      const m = sessionFlows().find((f) => f.id === id);
      return ok({ ...m, data: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } } });
    };
    try {
      return (await flowFor(id).then((flow) => (flow ? ok(flow) : null))) ?? empty();
    } catch (e) {
      console.warn("[withhuman mock]", e);
      return empty();
    }
  }
  const f = state.flows.find((x) => x.id === id);
  return f ? ok(f) : notFound();
}

type Route = [string, RegExp, (req: MockRequest, m: RegExpMatchArray) => Answer];

const routes: Route[] = [
  // --- auth / user -------------------------------------------------------
  ["GET", /^auto_login$/, () =>
    ok({ ...USER, access_token: "mock-access-token", refresh_token: "auto", token_type: "bearer" })],
  ["GET", /^session$/, () => ok({ authenticated: true, user: USER })],
  ["GET", /^users\/whoami$/, () => ok(USER)],
  ["PATCH", /^users\/[^/]+$/, (r) => ok({ ...USER, ...r.body })],
  ["POST", /^refresh$/, () => ok({ access_token: "mock-access-token", refresh_token: "auto", token_type: "bearer" })],
  ["POST", /^logout$/, () => ok({ message: "Logout successful" })],
  ["GET", /^authz\/me\/permissions$/, () => ok({})],

  // --- server info -------------------------------------------------------
  ["GET", /^version$/, () => ok({ version: "1.13.4", main_version: "1.13.4", package: "Langflow" })],
  ["GET", /^config$/, () => ok(config)],
  ["GET", /^health(_check)?$/, () => ok({ status: "ok", chat: "ok", db: "ok" })],

  // --- component palette -------------------------------------------------
  ["GET", /^all$/, () => ok((state.all ??= buildAllTypes()))],
  ["GET", /^starter-projects\/?$/, () => ok([])],
  ["GET", /^flows\/basic_examples\/?$/, () => ok([])],

  // --- projects (folders) ------------------------------------------------
  ["GET", /^projects\/?$/, () => ok(state.projects.map(projectWithFlows))],
  ["GET", /^projects\/([^/]+)$/, (r, m) => {
    const p = state.projects.find((x) => x.id === m[1]);
    if (!p) return notFound();
    return ok({ folder: projectWithFlows(p), flows: paginate(flowsOf(p.id, r.query), r.query) });
  }],
  ["POST", /^projects\/?$/, (r) => {
    const p = { id: uuid(), parent_id: null, description: "", owner_username: USER.username, is_owner: true, ...r.body };
    delete p.flows; delete p.components;
    state.projects.push(p);
    return ok(projectWithFlows(p));
  }],
  ["PATCH", /^projects\/([^/]+)$/, (r, m) => {
    const p = state.projects.find((x) => x.id === m[1]);
    if (!p) return notFound();
    Object.assign(p, { name: r.body?.name ?? p.name, description: r.body?.description ?? p.description });
    return ok(projectWithFlows(p));
  }],
  ["DELETE", /^projects\/([^/]+)$/, (_r, m) => {
    state.projects = state.projects.filter((x) => x.id !== m[1]);
    state.flows = state.flows.filter((f) => f.folder_id !== m[1]);
    return ok({});
  }],

  // --- flows -------------------------------------------------------------
  ["GET", /^flows\/?$/, (r) => {
    if (r.query.get("components_only") === "true") return ok([]);
    const flows = flowsOf(r.query.get("folder_id"), r.query).filter((f) => !f.is_component);
    if (r.query.get("get_all") === "false" || r.query.has("page")) return ok(paginate(flows, r.query));
    return ok(flows);
  }],
  ["GET", /^flows\/public_flow\/([^/]+)$/, (_r, m) => oneFlow(m[1])],
  ["GET", /^flows\/([^/]+)\/versions\/?$/, () => ok({ entries: [], max_entries: 50 })],
  ["GET", /^flows\/([^/]+)\/note_translations$/, () => ok({})],
  ["GET", /^flows\/([^/]+)\/version-state$/, (_r, m) => {
    const f = findFlow(m[1]);
    if (!f) return notFound();
    return ok({ version_token: f.version_token ?? null, last_modified_by: USER.id, last_modified_by_username: USER.username, updated_at: f.updated_at ?? null });
  }],
  ["GET", /^flows\/([^/]+)$/, (_r, m) => oneFlow(m[1])],
  ["POST", /^flows\/?$/, (r) => {
    const now = new Date().toISOString();
    const f = { ...r.body, id: uuid(), user_id: USER.id, folder_id: r.body?.folder_id ?? PROJECT_ID, updated_at: now, date_created: now, access_type: "PRIVATE", version_token: "mock-1" };
    state.flows.unshift(f);
    return ok(f);
  }],
  ["PATCH", /^flows\/([^/]+)$/, (r, m) => {
    // A session is the engine's record: nothing the page sends changes it.
    if (isSession(m[1])) return oneFlow(m[1]);
    const f = state.flows.find((x) => x.id === m[1]);
    if (!f) return notFound();
    Object.assign(f, r.body, { id: f.id, updated_at: new Date().toISOString() });
    return ok(f);
  }],
  ["DELETE", /^flows\/?$/, (r) => {
    const ids: string[] = Array.isArray(r.body) ? r.body : [];
    state.flows = state.flows.filter((f) => !ids.includes(f.id));
    return ok({ deleted: ids.length });
  }],
  ["DELETE", /^flows\/([^/]+)$/, (_r, m) => {
    state.flows = state.flows.filter((f) => f.id !== m[1]);
    return ok({ message: "Flow deleted successfully" });
  }],

  // --- variables / keys / store -----------------------------------------
  ["GET", /^variables\/?$/, () => ok(state.variables)],
  ["POST", /^variables\/?$/, (r) => {
    const v = { id: uuid(), default_fields: [], ...r.body };
    state.variables.push(v);
    return ok(v);
  }],
  ["PATCH", /^variables\/([^/]+)$/, (r, m) => {
    const v = state.variables.find((x) => x.id === m[1]);
    if (v) Object.assign(v, r.body);
    return ok(v ?? {});
  }],
  ["DELETE", /^variables\/([^/]+)$/, (_r, m) => {
    state.variables = state.variables.filter((x) => x.id !== m[1]);
    return ok({});
  }],
  ["GET", /^api_key\/?$/, () => ok({ total_count: 0, user_id: USER.id, api_keys: [] })],
  ["GET", /^store\/check\/?$/, () => ok({ enabled: false })],
  ["GET", /^store\/check\/api_key$/, () => ok({ has_api_key: false, is_valid: false })],
  ["GET", /^store\/tags$/, () => ok([])],

  // --- monitor -----------------------------------------------------------
  ["GET", /^monitor\/builds$/, () => ok({ vertex_builds: {} })],
  ["GET", /^monitor\/messages\/sessions$/, () => ok([])],
  ["GET", /^monitor\/messages$/, () => ok([])],
  ["GET", /^monitor\/transactions$/, (r) => ok(paginate([], r.query))],
  ["GET", /^monitor\/traces$/, (r) => ok({ traces: [], ...paginate([], r.query) })],

  // --- files / models / mcp ---------------------------------------------
  ["GET", /^files\/?$/, () => ok([])],
  ["GET", /^files\/profile_pictures\/list$/, () => ok({ files: [] })],
  ["GET", /^models\/enabled_models$/, () => ok({ enabled_models: {} })],
  ["GET", /^models\/provider-variable-mapping$/, () => ok({})],
  ["GET", /^models\/?$/, () => ok([])],
  ["GET", /^mcp\/servers$/, () => ok([])],
  ["GET", /^mcp\/project\/([^/]+)$/, (_r, m) => ok({
    tools: allFlows().filter((f) => f.folder_id === m[1] && !f.is_component).map((f) => ({
      id: f.id, name: f.name, description: f.description, mcp_enabled: Boolean(f.mcp_enabled),
      action_name: f.name.toLowerCase().replace(/[^a-z0-9]+/g, "_"), action_description: f.description,
    })),
    auth_settings: { auth_type: "none" },
    server_name: `lf-${PROJECT_NAME.toLowerCase().replace(/[^a-z0-9]+/g, "_")}`,
  })],
  ["GET", /^mcp\/project\/([^/]+)\/installed$/, () => ok([])],
  // Running a flow needs the real engine: fail fast so Langflow shows its own
  // build error instead of polling forever.
  ["POST", /^(build|run|webhook|responses|agentic|workflows)(\/|$)/, () => NO_ENGINE],
  ["GET", /^build\//, () => NO_ENGINE],
  ["GET", /^flows\/([^/]+)\/events$/, () => ok({ events: [], settled: true })],
  ["GET", /^workflows\/pending$/, () => ok([])],
  // OSS pass-through: every requested action is allowed on every resource.
  ["POST", /^authz\/me\/permissions$/, (r) => {
    const actions = r.body?.actions?.length ? r.body.actions : ["read", "write", "execute", "delete", "create"];
    const ids: string[] = r.body?.resource_ids ?? [];
    return ok({ resource_type: r.body?.resource_type, permissions: Object.fromEntries(ids.map((id) => [id, actions])) });
  }],
  // A field changed (or asked for a refresh): send the component back with
  // the template the page sent, as the backend does when nothing is computed.
  ["POST", /^custom_component\/update$/, (r) => ok(updatedComponent(r.body))],
  ["GET", /^knowledge-base-storage\/status$/, () =>
    ok({ stores: [], inventory: { complete: true, issues: 0, details: [] }, is_admin: true, running: false, revision: "0" })],
  ["GET", /^knowledge_bases\/?$/, () => ok([])],
  ["GET", /^memories\/?$/, () => ok([])],
];

export function handle(req: MockRequest): Answer {
  const path = req.path.replace(/^\/+/, "").replace(/\/+$/, (s) => (s ? "/" : ""));
  for (const [method, re, fn] of routes) {
    if (method !== req.method) continue;
    const m = path.match(re) ?? path.replace(/\/$/, "").match(re);
    if (m) {
      try {
        return fn({ ...req, path }, m);
      } catch (e) {
        console.warn("[withhuman mock] handler failed", req.method, path, e);
        return ok({});
      }
    }
  }
  const key = `${req.method} ${req.version}/${path}`;
  if (!seenUnknown.has(key)) {
    seenUnknown.add(key);
    console.info("[withhuman mock] unknown route answered empty:", key);
  }
  return ok(req.method === "GET" ? [] : {});
}
