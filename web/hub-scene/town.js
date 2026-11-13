// 기록 as a route finder's map of a real part of town, drawn by MapLibre. The streets and
// buildings are a few blocks of Seongsu-dong, Seoul, from OpenStreetMap (district.json, which
// the page reads too). The page says which building each Call is, where each turn's request
// is, and each Call's way along the streets (admin/route.rs); this file draws the town tilted
// as a driving map is, raises the buildings that are Calls, and lights the way to the one
// picked: a blue line on a darker casing, the buildings it goes by in blue, a pin on the one
// it arrives at (green, red for a Call the engine stopped, amber for one it held), and the
// same way as directions in a card. A building picked is the app's selection; a request
// picked is its turn.
import maplibregl from "maplibre-gl";
import sheet from "maplibre-gl/dist/maplibre-gl.css";
import district from "../../src/admin/district.json";

// A width on the ground in metres as the line widths that keep it at every zoom: at this
// latitude a pixel is 7.57 m at zoom 14 and halves with each zoom after.
const metres = (m) => ["interpolate", ["exponential", 2], ["zoom"], 14, m / 7.57, 20, m / 0.118];
const WIDTHS = { major: 10, minor: 6, lane: 3.6 };
const TONES = {
  none: "#f1eff4",
  dim: "#dcdbe8",
  call: "#ffffff",
  way: "#8fb4fb",
  end: "#3b82f6",
  blocked: "#f0958c",
  held: "#f3c867",
};

function lines(kind) {
  return { type: "FeatureCollection", features: district.roads.filter((r) => r.kind === kind).map((r) => ({ type: "Feature", properties: {}, geometry: { type: "LineString", coordinates: r.points } })) };
}

function style() {
  const layers = [{ id: "land", type: "background", paint: { "background-color": "#e9e4e2" } }];
  const sources = {
    buildings: {
      type: "geojson",
      promoteId: "id",
      data: { type: "FeatureCollection", features: district.buildings.map((b, id) => ({ type: "Feature", properties: { id, height: b.height }, geometry: { type: "Polygon", coordinates: [b.ring] } })) },
    },
    route: { type: "geojson", data: { type: "FeatureCollection", features: [] } },
  };
  const round = { "line-cap": "round", "line-join": "round" };
  // Pavements under every roadway, so where streets meet they join as one.
  for (const kind of ["lane", "minor", "major"]) {
    sources[kind] = { type: "geojson", data: lines(kind) };
    layers.push({ id: `pave-${kind}`, type: "line", source: kind, layout: round, paint: { "line-color": "#f6f5fa", "line-width": metres(WIDTHS[kind] + 3) } });
  }
  for (const kind of ["lane", "minor", "major"]) {
    layers.push({ id: `road-${kind}`, type: "line", source: kind, layout: round, paint: { "line-color": "#b6b8da", "line-width": metres(WIDTHS[kind]) } });
  }
  layers.push({ id: "paint-major", type: "line", source: "major", paint: { "line-color": "#f6e3a0", "line-width": metres(0.3) } });
  layers.push({ id: "paint-minor", type: "line", source: "minor", paint: { "line-color": "#fbfbff", "line-width": metres(0.22), "line-dasharray": [14, 12] } });
  layers.push({ id: "way-casing", type: "line", source: "route", layout: round, paint: { "line-color": "#1d4fbf", "line-width": metres(5.2) } });
  layers.push({ id: "way", type: "line", source: "route", layout: round, paint: { "line-color": "#3b82f6", "line-width": metres(3.6) } });
  const tone = ["coalesce", ["feature-state", "tone"], "none"];
  layers.push({
    id: "buildings",
    type: "fill-extrusion",
    source: "buildings",
    paint: {
      "fill-extrusion-color": ["match", tone, ...Object.entries(TONES).flat().slice(2), TONES.none],
      // A building that is no Call stays low, so the ones that are stand out of the town.
      "fill-extrusion-height": ["case", ["boolean", ["feature-state", "used"], false], ["max", 11, ["min", ["get", "height"], 24]], ["min", ["*", ["get", "height"], 0.3], 4.5]],
      "fill-extrusion-opacity": 1,
      "fill-extrusion-vertical-gradient": true,
    },
  });
  return { version: 8, sources, layers };
}

function mount(container) {
  if (!document.getElementById("tw-sheet")) {
    const tag = document.createElement("style");
    tag.id = "tw-sheet";
    tag.textContent = sheet;
    document.head.append(tag);
  }
  const holder = document.createElement("div");
  holder.className = "tw-map";
  const tip = document.createElement("div");
  tip.className = "rt-tip";
  tip.hidden = true;
  const guide = document.createElement("aside");
  guide.className = "rt-guide";
  guide.hidden = true;
  const credit = document.createElement("div");
  credit.className = "tw-credit";
  credit.textContent = district.credit;
  container.append(holder, tip, guide, credit);

  const map = new maplibregl.Map({
    container: holder,
    style: style(),
    center: district.centre,
    zoom: 16.4,
    pitch: 56,
    bearing: -24,
    maxPitch: 70,
    minZoom: 14.5,
    maxZoom: 19.5,
    attributionControl: false,
    renderWorldCopies: false,
  });
  map.on("error", (e) => console.error("town map", e.error ?? e));
  const ready = new Promise((done) => map.once("load", done));
  const api = { container, onPick: null };
  let nodes = [];
  let byBuilding = new Map();
  let picked = null;
  let lit = [];
  let marks = [];
  let asks = [];
  let alive = true;

  const label = (text, name) => {
    const el = document.createElement("div");
    el.className = name;
    el.textContent = text;
    return el;
  };
  const mark = (el, at, anchor = "bottom") => new maplibregl.Marker({ element: el, anchor }).setLngLat(at).addTo(map);

  // What each building is drawn as: a Call's own colour, the way's when it is on it, faded
  // when the way lit (or the turn picked) is not its.
  function paint() {
    const on = new Set(lit);
    const asked = picked != null && nodes[picked]?.kind === "ask" ? nodes[picked].turn : null;
    nodes.forEach((n, i) => {
      if (n.building == null) return;
      const own = n.state ?? "call";
      const tone = i === picked ? (n.state ?? "end") : on.has(i) ? (n.state ?? "way") : asked != null ? (n.turn === asked ? own : "dim") : lit.length > 1 ? "dim" : own;
      map.setFeatureState({ source: "buildings", id: n.building }, { used: true, tone });
    });
    asks.forEach(({ el, i }) => {
      el.classList.toggle("on", i === picked || on.has(i));
      el.classList.toggle("dim", !(i === picked || on.has(i)) && (lit.length > 1 || asked != null));
    });
  }

  // The way lit: its line, the names of the places on it in order, the pin where it arrives,
  // and the directions.
  function relight() {
    marks.forEach((m) => m.remove());
    marks = [];
    lit = picked != null && nodes[picked] ? [...(nodes[picked].way ?? [picked])] : [];
    const end = nodes[picked];
    const route = lit.length > 1 && end.route ? end.route : null;
    map.getSource("route").setData({ type: "FeatureCollection", features: route ? [{ type: "Feature", properties: {}, geometry: { type: "LineString", coordinates: route } }] : [] });
    paint();
    guide.replaceChildren();
    guide.hidden = !route;
    guide.className = `rt-guide ${end?.state ?? ""}`;
    if (!route) return;
    let no = 0;
    const calls = lit.filter((i) => nodes[i].seq != null).length;
    const head = document.createElement("header");
    const title = document.createElement("strong");
    title.textContent = `${end.label} · ${end.tip}`;
    const sum = document.createElement("span");
    const verdict = end.state === "blocked" ? "도착지에서 차단됨" : end.state === "held" ? "도착지에서 확인 요청" : "허용됨";
    sum.textContent = calls > 1 ? `호출 ${calls - 1}곳을 거쳐 도착 · ${verdict}` : `요청에서 바로 도착 · ${verdict}`;
    const all = document.createElement("button");
    all.type = "button";
    all.textContent = "전체 경로";
    all.addEventListener("click", () => show());
    head.append(title, sum, all);
    const list = document.createElement("ol");
    lit.forEach((i, k) => {
      const n = nodes[i];
      const li = document.createElement("li");
      const dot = document.createElement("i");
      const body = document.createElement("div");
      const name = document.createElement("b");
      const what = document.createElement("span");
      if (n.kind === "words") {
        // Not a place but why the way goes on: the words read at the Call before.
        li.className = "why";
        name.textContent = "이 결과를 읽고 다음 호출로";
        what.textContent = `${n.tip} · ${n.note}`;
      } else {
        no++;
        dot.textContent = no;
        const role = k === 0 ? "출발" : i === picked ? "도착" : "경유";
        if (i === picked) li.className = "end";
        name.textContent = `${role} · ${n.label}`;
        what.textContent = n.tip;
        li.tabIndex = 0;
        const go = () => map.easeTo({ center: n.at, zoom: Math.max(map.getZoom(), 18), duration: 600 });
        li.addEventListener("click", go);
        li.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && go());
        // The same number on the map, over the building.
        if (n.kind !== "ask") {
          const el = label(n.label, i === picked ? "tw-step end" : "tw-step");
          el.dataset.step = no;
          marks.push(mark(el, n.at, "top"));
        }
      }
      body.append(name, what);
      if (k === 0 && n.intent) {
        const intent = document.createElement("span");
        intent.className = "intent";
        intent.textContent = `의도 · ${n.intent}`;
        body.append(intent);
      }
      li.append(dot, body);
      list.append(li);
    });
    guide.append(head, list);
    // The pin stands on the building it arrives at.
    const pin = document.createElement("div");
    pin.className = `tw-pin ${end.state ?? ""}`;
    pin.append(document.createElement("i"));
    marks.push(mark(pin, end.at));
  }

  // Brings the way lit into view, clear of the directions card; a request picked, its turn.
  function show() {
    const end = nodes[picked];
    if (!end) return;
    const places = end.route ?? nodes.filter((n) => n.turn === end.turn && n.at).map((n) => n.at);
    if (!places.length) return;
    const box = places.reduce((b, p) => b.extend(p), new maplibregl.LngLatBounds(places[0], places[0]));
    map.fitBounds(box, { padding: { top: 150, bottom: 110, left: 90, right: guide.hidden ? 90 : 350 }, maxZoom: 17.4, duration: 700 });
  }

  api.setData = (next) =>
    ready.then(() => {
      if (!alive) return;
      map.removeFeatureState({ source: "buildings" });
      asks.forEach((a) => a.marker.remove());
      nodes = next.nodes;
      picked = null;
      byBuilding = new Map(nodes.map((n, i) => [n.building, i]).filter(([b]) => b != null));
      asks = nodes.flatMap((n, i) => {
        if (n.kind !== "ask") return [];
        const el = label(n.label, "tw-ask");
        el.addEventListener("click", (e) => {
          e.stopPropagation();
          api.onPick?.(n.pick);
        });
        return [{ el, i, marker: mark(el, n.at) }];
      });
      relight();
      api.fit();
    });
  api.setPicked = (i) =>
    ready.then(() => {
      if (!alive || i === picked) return;
      picked = i;
      relight();
      show();
    });
  api.zoomBy = (f) => map.zoomTo(map.getZoom() + Math.log2(f), { duration: 250 });
  api.fit = () => {
    const places = nodes.filter((n) => n.at).map((n) => n.at);
    if (!places.length) return;
    const box = places.reduce((b, p) => b.extend(p), new maplibregl.LngLatBounds(places[0], places[0]));
    map.fitBounds(box, { padding: { top: 150, bottom: 110, left: 90, right: 90 }, maxZoom: 17.6, duration: 500 });
  };

  // A building under the pointer says what Call it is; picked, it is the app's selection.
  const under = (e) => {
    const hit = map.queryRenderedFeatures(e.point, { layers: ["buildings"] })[0];
    return hit ? byBuilding.get(hit.id) : undefined;
  };
  map.on("mousemove", (e) => {
    const i = under(e);
    const n = i == null ? null : nodes[i];
    map.getCanvas().style.cursor = n?.pick != null ? "pointer" : "";
    tip.hidden = !n;
    if (!n) return;
    tip.replaceChildren();
    for (const [tag, text] of [["small", n.label], ["strong", n.tip], ["span", n.note]]) {
      if (!text) continue;
      const el = document.createElement(tag);
      el.textContent = text;
      tip.append(el);
    }
    tip.style.transform = `translate(-50%, -100%) translate(${e.point.x}px, ${e.point.y - 14}px)`;
  });
  map.on("mouseout", () => (tip.hidden = true));
  map.on("click", (e) => {
    const i = under(e);
    if (i != null && nodes[i].pick != null) api.onPick?.(nodes[i].pick);
  });

  const sized = new ResizeObserver(() => map.resize());
  sized.observe(container);

  api.dispose = () => {
    alive = false;
    sized.disconnect();
    map.remove();
    holder.remove();
    tip.remove();
    guide.remove();
    credit.remove();
  };
  return api;
}

window.WHRoute = { mount };
