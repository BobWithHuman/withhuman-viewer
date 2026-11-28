// Installs the mock backend before any Langflow module runs: every axios
// request and every fetch to the Langflow API (or to an outside host) is
// answered in-page by ./routes, so the frontend boots with no server and
// nothing leaves the page. Import this module first; delete it (and the
// import) to talk to a real backend again.

import axios, {
  AxiosError,
  AxiosHeaders,
  type AxiosResponse,
  type InternalAxiosRequestConfig,
} from "axios";
import rocketUrl from "./assets/046-rocket.svg?url";
import { handle, type MockRequest } from "./routes";

const API_RE = /\/api\/(v1|v2)\/(.*)$/;
const LOG = false;

// A base usable with `new URL()` when the page has an opaque origin
// (`about:srcdoc` inside a `null`-origin host reports origin "null").
const FALLBACK_BASE = "http://localhost";

function pageBase(): string {
  const origin = window.location.origin;
  return origin && origin !== "null" ? origin : FALLBACK_BASE;
}

function parseBody(body: unknown): unknown {
  if (typeof body !== "string") return body ?? null;
  try {
    return JSON.parse(body);
  } catch {
    return body;
  }
}

/** Turns a URL into a mock request, or null when it is not ours to answer. */
function toRequest(
  method: string,
  rawUrl: string,
  params: Record<string, unknown> | undefined,
  body: unknown,
): MockRequest | "external" | null {
  if (/^(data|blob):/i.test(rawUrl)) return null;
  let url: URL;
  try {
    url = new URL(rawUrl, pageBase() + "/");
  } catch {
    return null;
  }
  const isPageOrigin = url.origin === pageBase();
  if (!isPageOrigin && /^https?:$/.test(url.protocol)) return "external";
  for (const [k, v] of Object.entries(params ?? {})) {
    if (v !== undefined && v !== null) url.searchParams.set(k, String(v));
  }
  const api = url.pathname.match(API_RE);
  if (api) {
    return {
      method: method.toUpperCase(),
      path: api[2],
      version: api[1] as "v1" | "v2",
      query: url.searchParams,
      body: parseBody(body),
    };
  }
  if (/\/health(_check)?\/?$/.test(url.pathname)) {
    return {
      method: method.toUpperCase(),
      path: "health",
      version: "root",
      query: url.searchParams,
      body: null,
    };
  }
  return null;
}

/** Answers for hosts outside the page (GitHub stars, Discord counts, ...). */
function externalAnswer(): unknown {
  return {};
}

// --- axios ----------------------------------------------------------------

async function mockAdapter(
  config: InternalAxiosRequestConfig,
): Promise<AxiosResponse> {
  const full = axios.getUri({ ...config, params: undefined });
  const req = toRequest(
    config.method ?? "get",
    full,
    config.params,
    config.data,
  );
  let status = 200;
  let data: unknown = {};
  if (req === "external") {
    data = externalAnswer();
  } else if (req) {
    const res = await handle(req);
    status = res.status;
    data = structuredClone(res.data);
    if (LOG) console.debug("[withhuman mock]", req.method, full, status);
  } else {
    // A same-origin, non-API URL (static file). Nothing to serve.
    status = 404;
    data = "";
  }
  const response: AxiosResponse = {
    data,
    status,
    statusText: status === 200 ? "OK" : "Error",
    headers: new AxiosHeaders({ "content-type": "application/json" }),
    config,
    request: {},
  };
  if (status >= 200 && status < 300) return response;
  throw new AxiosError(
    `Request failed with status code ${status}`,
    status >= 500 ? AxiosError.ERR_BAD_RESPONSE : AxiosError.ERR_BAD_REQUEST,
    config,
    {},
    response,
  );
}

// `axios.create()` copies the defaults, so every instance created after this
// line (Langflow's `api` included) uses the mock adapter too.
axios.defaults.adapter = mockAdapter;

// --- fetch ----------------------------------------------------------------

const realFetch = window.fetch.bind(window);

window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
  const url =
    typeof input === "string"
      ? input
      : input instanceof URL
        ? input.href
        : input.url;
  const method = init?.method ?? (input instanceof Request ? input.method : "GET");
  const req = toRequest(method, url, undefined, init?.body);
  if (req === null) return realFetch(input as RequestInfo, init);
  const res =
    req === "external" ? { status: 200, data: externalAnswer() } : await handle(req);
  if (LOG && req !== "external") console.debug("[withhuman mock] fetch", method, url, res.status);
  return new Response(JSON.stringify(res.data ?? {}), {
    status: res.status,
    headers: { "content-type": "application/json" },
  });
};

// --- EventSource / WebSocket: inert stand-ins that never connect ------------

class InertEventSource extends EventTarget {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  readonly CONNECTING = 0;
  readonly OPEN = 1;
  readonly CLOSED = 2;
  readyState = 0;
  withCredentials = false;
  onopen: ((e: Event) => void) | null = null;
  onmessage: ((e: MessageEvent) => void) | null = null;
  onerror: ((e: Event) => void) | null = null;
  constructor(readonly url: string) {
    super();
  }
  close() {
    this.readyState = 2;
  }
}

class InertWebSocket extends EventTarget {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  readonly CONNECTING = 0;
  readonly OPEN = 1;
  readonly CLOSING = 2;
  readonly CLOSED = 3;
  readyState = 0;
  binaryType: BinaryType = "blob";
  bufferedAmount = 0;
  extensions = "";
  protocol = "";
  onopen: ((e: Event) => void) | null = null;
  onmessage: ((e: MessageEvent) => void) | null = null;
  onerror: ((e: Event) => void) | null = null;
  onclose: ((e: CloseEvent) => void) | null = null;
  constructor(readonly url: string) {
    super();
  }
  send() {}
  close() {
    this.readyState = 3;
  }
}

(window as any).EventSource = InertEventSource;
(window as any).WebSocket = InertWebSocket;

// --- opaque origin helpers -----------------------------------------------

// Langflow builds URLs on `window.location.origin` (`new URL(url, origin)`,
// `new URL(path, origin + base)`); with an opaque origin that string is
// "null" and the constructor throws, so such bases get a fixed origin.
if (window.location.origin === "null") {
  const NativeURL = window.URL;
  class OpaqueSafeURL extends NativeURL {
    constructor(url: string | URL, base?: string | URL) {
      super(
        url,
        typeof base === "string" && /^null(\/|$)/.test(base)
          ? FALLBACK_BASE + base.slice(4)
          : base,
      );
    }
  }
  window.URL = OpaqueSafeURL as typeof URL;
}

// Langflow reads localStorage at module load; give it an in-memory store
// when the host denies storage to this frame.
try {
  window.localStorage.getItem("isDark");
} catch {
  const mem = new Map<string, string>();
  const storage = {
    get length() {
      return mem.size;
    },
    key: (i: number) => [...mem.keys()][i] ?? null,
    getItem: (k: string) => (mem.has(k) ? mem.get(k)! : null),
    setItem: (k: string, v: string) => void mem.set(k, String(v)),
    removeItem: (k: string) => void mem.delete(k),
    clear: () => mem.clear(),
  };
  Object.defineProperty(window, "localStorage", { value: storage });
}

// Langflow follows the OS theme until the user picks one in its account
// menu (light / dark / system, kept in "themePreference"). Start in light
// instead; the toggle keeps working and its choice is kept.
try {
  if (window.localStorage.getItem("themePreference") === null) {
    window.localStorage.setItem("themePreference", "light");
    window.localStorage.setItem("isDark", "false");
  }
} catch {}

// --- static backend files shown with <img> --------------------------------

// <img> loads cannot be answered by the fetch hook; point the default profile
// picture (served by the backend in Langflow) at an inlined copy instead.

const IMAGE_FILES: Array<[RegExp, string]> = [
  [/\/files\/profile_pictures\//, rocketUrl],
];

function swapImage(img: HTMLImageElement) {
  const src = img.getAttribute("src") ?? "";
  for (const [re, url] of IMAGE_FILES) {
    if (re.test(src) && src !== url) img.setAttribute("src", url);
  }
}

new MutationObserver((records) => {
  for (const r of records) {
    if (r.type === "attributes" && r.target instanceof HTMLImageElement) {
      swapImage(r.target);
    }
    r.addedNodes.forEach((n) => {
      if (n instanceof HTMLImageElement) swapImage(n);
      else if (n instanceof Element) n.querySelectorAll("img").forEach(swapImage);
    });
  }
}).observe(document.documentElement, {
  subtree: true,
  childList: true,
  attributes: true,
  attributeFilter: ["src"],
});
