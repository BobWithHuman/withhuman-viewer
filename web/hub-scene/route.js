// 기록 as a town seen from above at an angle, its roads the way the engine traced each Call back
// to the request. Each turn has a wide street of its own that runs round the town's blocks,
// its request (the agent's intent a pin on it) where it starts; the turns lie one above the
// last, a road up the left joining their requests. Every Call is a building at the side of a
// road: along its turn's street when the engine traced it to the request, along the road that
// forks off beside another Call's building when the engine traced it to that Call. In a long
// turn the Calls that only followed the request stand folded into one large building, which
// opens when picked. The roads are a driving map's: lavender roadway with painted lines, pale
// pavement, white buildings with lavender glass. A building picked is drawn round in black,
// as is the request its way starts at (a request can be picked too, which keeps its whole
// turn clear and fades the others), and the way to it is lit as a route finder draws a route:
// a blue line on a darker casing down the roads, fork by fork, marks moving along it, a ring
// round the request and a pin over the Call it arrives at (green, red for a Call the engine
// stopped, amber for one it held); the words of a tool result that told a Call stand at the
// corner where its fork leaves. The same way is given as directions in a card; a line of it
// picked brings that building to the middle. The page gives the buildings, the roads and each
// Call's way (admin/route.rs); this file only draws them.
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";

const WHITE = 0xffffff;
const EDGE = 0xbcbee0;
const INK = 0x15171a;
// The floor as a driving map paints it: lavender-grey roadway with white markings, a pale
// pavement along it, lots the pavement's colour edged a little darker.
const ROAD = 0xb3b5d9;
const PAVE = 0xf3f2f8;
const PAINT = 0xfbfbff;
const CENTRE = 0xf3dc8a;
const LOT = 0xf3f2f8;
const KERB = 0xd9d9ea;
const MARK = 0xdfe0f1;
// The way lit, as a route finder draws one: a blue line on a wider, darker one.
const ROUTE = 0x3b82f6;
const CASING = 0x1d4fbf;
const GREEN = 0x3fc47c;
// The way lit to a Call the engine stopped or held.
const WAYS = { blocked: 0xe2574b, held: 0xdb9c1c };
const ROUTE_LOT = 0xbcd6fb;
const LOTS = { blocked: 0xf6aaa3, held: 0xf7d488 };
// The eye looks down a diagonal of the floor: one step to the right on the page is this much
// along x and back along z.
const TURN = Math.SQRT1_2;
const ON_STREET = new Set(["ask"]);

const white = () => new THREE.MeshStandardMaterial({ color: WHITE, roughness: 0.86, metalness: 0, transparent: true });
// A block's doors, dials and slits: the lavender glass of the buildings on a driving map.
const grey = () => new THREE.MeshStandardMaterial({ color: 0xb4b8e2, roughness: 0.7, metalness: 0, transparent: true });

function solid(geometry, material, x, y, z) {
  const m = new THREE.Mesh(geometry, material);
  m.position.set(x, y, z);
  m.castShadow = true;
  return m;
}

// Each kind of building, standing at its own origin with its front toward the eye. Returns its
// group, its height, its drawn edges and black outline, and the three materials it is made of
// (walls, glass, edges), which fade together.
function block(kind, pinned) {
  const g = new THREE.Group();
  const wall = white();
  const glass = grey();
  const line = new THREE.LineBasicMaterial({ color: EDGE, transparent: true });
  // A part of the walls: edged, and outlined when the building is picked.
  const box = (w, h, d, x, y, z, r = 0.035) => {
    const m = solid(new RoundedBoxGeometry(w, h, d, 3, Math.min(r, w / 2.2, h / 2.2, d / 2.2)), wall, x, y, z);
    g.add(m);
    return m;
  };
  // A detail of glass or metal: neither edged nor outlined.
  const part = (geometry, x, y, z) => {
    const m = solid(geometry, glass, x, y, z);
    m.userData.detail = true;
    g.add(m);
    return m;
  };
  // Bands of windows round the two walls seen, on a body w by d standing from `foot` to `top`.
  const windows = (w, d, foot, top, rows, x = 0, z = 0) => {
    for (let k = 0; k < rows; k++) {
      const y = foot + ((k + 0.5) * (top - foot)) / rows;
      part(new THREE.BoxGeometry(w * 0.8, Math.min(0.11, (top - foot) / rows / 2), 0.02), x, y, z + d / 2 + 0.006);
      part(new THREE.BoxGeometry(0.02, Math.min(0.11, (top - foot) / rows / 2), d * 0.8), x + w / 2 + 0.006, y, z);
    }
  };
  let tall = 1;
  if (kind === "ask") {
    for (const [r, h, y] of [[0.86, 0.26, 0.13], [0.56, 0.2, 0.36]]) {
      const hex = solid(new THREE.CylinderGeometry(r, r, h, 6), wall, 0, y, 0);
      hex.rotation.y = Math.PI / 6;
      g.add(hex);
    }
    tall = 0.46;
    if (pinned) {
      // A pin stood on the request: where the agent said it was going with it.
      part(new THREE.CylinderGeometry(0.035, 0.035, 0.7, 12), 0, 0.81, 0);
      g.add(solid(new THREE.SphereGeometry(0.2, 32, 20), wall, 0, 1.3, 0));
      tall = 1.5;
    }
  } else if (kind === "words") {
    // A page stood on its edge: the words read out of a tool result.
    const page = box(0.5, 0.64, 0.05, 0, 0.34, 0, 0.02);
    page.rotation.y = Math.PI / 4;
    for (let k = 0; k < 3; k++) {
      const row = part(new THREE.BoxGeometry(0.3 - k * 0.06, 0.03, 0.012), 0.022, 0.5 - k * 0.12, 0.022);
      row.rotation.y = Math.PI / 4;
    }
    tall = 0.66;
  } else if (kind === "read") {
    // A reading room: a low hall behind a row of columns, a lantern on its roof.
    box(1.36, 0.1, 1.36, 0, 0.05, 0, 0.02);
    box(1.1, 0.52, 0.96, 0, 0.36, -0.07);
    for (let k = -1.5; k <= 1.5; k++) part(new THREE.CylinderGeometry(0.035, 0.035, 0.52, 10), k * 0.3, 0.36, 0.55);
    part(new THREE.BoxGeometry(0.02, 0.26, 0.6), 0.556, 0.38, -0.07);
    box(1.28, 0.08, 1.28, 0, 0.66, 0, 0.02);
    box(0.62, 0.2, 0.5, 0, 0.8, -0.07);
    windows(0.62, 0.5, 0.72, 0.9, 1, 0, -0.07);
    tall = 0.92;
  } else if (kind === "change") {
    // An office: a tower of floors on a wider ground floor, a stair head on its roof.
    box(1.12, 0.26, 1.12, 0, 0.13, 0);
    part(new THREE.BoxGeometry(0.3, 0.2, 0.02), 0, 0.1, 0.566);
    box(0.92, 1.36, 0.92, 0, 0.94, 0);
    windows(0.92, 0.92, 0.3, 1.58, 5);
    box(0.98, 0.06, 0.98, 0, 1.65, 0, 0.02);
    box(0.32, 0.16, 0.32, -0.18, 1.76, -0.18, 0.02);
    tall = 1.84;
  } else if (kind === "run") {
    // A machine hall: a wide shed with a shutter, a strip of high windows and vents on the roof.
    box(1.26, 0.78, 1.14, 0, 0.39, 0);
    part(new THREE.BoxGeometry(0.5, 0.48, 0.02), -0.2, 0.24, 0.576);
    for (let k = 0; k < 4; k++) part(new THREE.BoxGeometry(0.44, 0.012, 0.024), -0.2, 0.1 + k * 0.11, 0.578);
    part(new THREE.BoxGeometry(1.0, 0.09, 0.02), 0, 0.64, 0.576);
    part(new THREE.BoxGeometry(0.02, 0.09, 0.9), 0.636, 0.64, 0);
    box(1.32, 0.06, 1.2, 0, 0.81, 0, 0.02);
    for (const x of [-0.3, 0.14]) part(new THREE.CylinderGeometry(0.1, 0.1, 0.16, 20), x, 0.92, -0.2);
    box(0.34, 0.14, 0.3, 0.38, 0.91, 0.24, 0.02);
    tall = 1.0;
  } else if (kind === "outside") {
    // A tower out to the world: a slim shaft on a podium, a mast and a dish on top.
    box(1.16, 0.3, 1.16, 0, 0.15, 0);
    part(new THREE.BoxGeometry(0.9, 0.1, 0.02), 0, 0.16, 0.586);
    part(new THREE.BoxGeometry(0.02, 0.1, 0.9), 0.586, 0.16, 0);
    box(0.72, 1.7, 0.72, 0, 1.15, 0);
    windows(0.72, 0.72, 0.36, 1.96, 6);
    box(0.78, 0.06, 0.78, 0, 2.03, 0, 0.02);
    part(new THREE.CylinderGeometry(0.02, 0.02, 0.5, 8), -0.14, 2.31, -0.14);
    const dish = part(new THREE.SphereGeometry(0.17, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2), 0.14, 2.16, 0.12);
    dish.rotation.set(Math.PI / 2.6, 0, -Math.PI / 5);
    tall = 2.56;
  } else if (kind === "delegate") {
    // Two wings joined by a bridge: work handed to another.
    box(0.5, 1.3, 1.0, -0.36, 0.65, 0);
    box(0.5, 1.0, 1.0, 0.36, 0.5, 0);
    windows(0.5, 1.0, 0.1, 1.24, 4, -0.36, 0);
    windows(0.5, 1.0, 0.1, 0.94, 3, 0.36, 0);
    box(0.3, 0.16, 0.4, 0, 0.7, 0, 0.02);
    box(0.56, 0.05, 1.06, -0.36, 1.32, 0, 0.02);
    box(0.56, 0.05, 1.06, 0.36, 1.02, 0, 0.02);
    tall = 1.36;
  } else if (kind === "many") {
    // A hall: many Calls under one roof, doors along the two sides seen, a raised middle.
    box(2.8, 0.1, 2.8, 0, 0.05, 0, 0.02);
    box(2.6, 0.9, 2.6, 0, 0.55, 0);
    for (let k = -2; k <= 2; k++) {
      part(new THREE.BoxGeometry(0.28, 0.46, 0.02), k * 0.48, 0.33, 1.306);
      part(new THREE.BoxGeometry(0.02, 0.46, 0.28), 1.306, 0.33, k * 0.48);
    }
    part(new THREE.BoxGeometry(2.2, 0.08, 0.02), 0, 0.8, 1.306);
    part(new THREE.BoxGeometry(0.02, 0.08, 2.2), 1.306, 0.8, 0);
    box(2.68, 0.07, 2.68, 0, 1.03, 0, 0.02);
    box(1.8, 0.36, 1.8, 0, 1.24, 0);
    windows(1.8, 1.8, 1.1, 1.38, 1);
    box(1.88, 0.06, 1.88, 0, 1.45, 0, 0.02);
    tall = 1.5;
  } else {
    // A kiosk: one room under an overhanging roof, a door and a window.
    box(0.96, 0.56, 0.96, 0, 0.28, 0);
    part(new THREE.BoxGeometry(0.24, 0.36, 0.02), -0.2, 0.18, 0.486);
    part(new THREE.BoxGeometry(0.3, 0.16, 0.02), 0.2, 0.34, 0.486);
    part(new THREE.BoxGeometry(0.02, 0.16, 0.6), 0.486, 0.34, 0);
    box(1.16, 0.07, 1.16, 0, 0.6, 0, 0.02);
    tall = 0.66;
  }
  // Drawn edges, so a white wall reads against a white wall; black and doubled when picked.
  const lines = [];
  const hull = [];
  for (const o of [...g.children]) {
    if (!o.isMesh || o.userData.detail) continue;
    const e = new THREE.LineSegments(new THREE.EdgesGeometry(o.geometry, 28), line);
    e.position.copy(o.position);
    e.rotation.copy(o.rotation);
    lines.push(e);
    const h = new THREE.Mesh(o.geometry, new THREE.MeshBasicMaterial({ color: INK, side: THREE.BackSide }));
    h.position.copy(o.position);
    h.rotation.copy(o.rotation);
    h.scale.setScalar(1.045);
    h.visible = false;
    hull.push(h);
  }
  g.add(...lines, ...hull);
  return { g, tall, lines, hull, line, mats: [wall, glass, line] };
}

// Evenly spaced places along the floor through the points given, the corners rounded.
function trace(points, round = 0.42) {
  const v = [];
  for (const [x, z] of points) {
    const p = new THREE.Vector3(x, 0, z);
    if (!v.length || v[v.length - 1].distanceTo(p) > 0.04) v.push(p);
  }
  // Only the corners: a place on the straight between two others would hold its corner's
  // rounding to the short stretch beside it.
  for (let i = v.length - 2; i > 0; i--) {
    const [a, b, c] = [v[i - 1], v[i], v[i + 1]];
    if (Math.abs((b.x - a.x) * (c.z - b.z) - (b.z - a.z) * (c.x - b.x)) < 1e-9) v.splice(i, 1);
  }
  if (v.length < 2) return [];
  const path = new THREE.CurvePath();
  const r = round;
  let from = v[0];
  for (let i = 1; i < v.length; i++) {
    const here = v[i];
    if (i === v.length - 1) {
      path.add(new THREE.LineCurve3(from, here));
      break;
    }
    const next = v[i + 1];
    const cut = Math.min(r, from.distanceTo(here) / 2, here.distanceTo(next) / 2);
    const before = here.clone().add(from.clone().sub(here).setLength(cut));
    const after = here.clone().add(next.clone().sub(here).setLength(cut));
    path.add(new THREE.LineCurve3(from, before));
    path.add(new THREE.QuadraticBezierCurve3(before, here, after));
    from = after;
  }
  const long = path.getLength();
  return long > 0.01 ? path.getSpacedPoints(Math.max(2, Math.ceil(long / 0.12))) : [];
}

function flat(geometry, color, y, order) {
  const m = new THREE.Mesh(geometry, new THREE.MeshBasicMaterial({ color, transparent: true, side: THREE.DoubleSide, depthWrite: false }));
  m.position.y = y;
  m.renderOrder = order;
  return m;
}

// A flat band along the places given.
function ribbon(pts, width, y, color, order) {
  if (pts.length < 2) return null;
  const pos = [];
  const index = [];
  pts.forEach((p, i) => {
    const a = pts[Math.max(0, i - 1)];
    const b = pts[Math.min(pts.length - 1, i + 1)];
    const nx = -(b.z - a.z);
    const nz = b.x - a.x;
    const n = Math.hypot(nx, nz) || 1;
    pos.push(p.x + (nx / n) * (width / 2), 0, p.z + (nz / n) * (width / 2));
    pos.push(p.x - (nx / n) * (width / 2), 0, p.z - (nz / n) * (width / 2));
    if (i) index.push(2 * i - 2, 2 * i - 1, 2 * i, 2 * i - 1, 2 * i + 1, 2 * i);
  });
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geo.setIndex(index);
  return flat(geo, color, y, order);
}

function rounded(side, r) {
  const h = side / 2;
  const s = new THREE.Shape();
  s.moveTo(-h + r, -h);
  s.lineTo(h - r, -h).quadraticCurveTo(h, -h, h, -h + r);
  s.lineTo(h, h - r).quadraticCurveTo(h, h, h - r, h);
  s.lineTo(-h + r, h).quadraticCurveTo(-h, h, -h, h - r);
  s.lineTo(-h, -h + r).quadraticCurveTo(-h, -h, -h + r, -h);
  return s;
}

// The places a way off to one side of the places given.
function beside(pts, off) {
  return pts.map((p, i) => {
    const a = pts[Math.max(0, i - 1)];
    const b = pts[Math.min(pts.length - 1, i + 1)];
    const n = Math.hypot(b.x - a.x, b.z - a.z) || 1;
    return new THREE.Vector3(p.x - ((b.z - a.z) / n) * off, 0, p.z + ((b.x - a.x) / n) * off);
  });
}

// A road along the places given: pavement under roadway, and the paint on it. A street has a
// yellow line down its middle and a broken white line down each half; a road forking off, one
// broken white line. Returns its flat pieces.
function road(pts, wide) {
  const out = [ribbon(pts, wide + 0.64, 0.002, PAVE, 0), ribbon(pts, wide, 0.004, ROAD, 1)];
  // Seven places painted, six left bare: the places are 0.12 apart.
  const broken = (line) => {
    for (let k = 4; k + 7 < line.length; k += 13) {
      // Not round a corner, where a line set to one side folds over itself.
      const [a, b, c, d] = [line[k], line[k + 1], line[k + 5], line[k + 6]];
      const straight = (b.x - a.x) * (d.x - c.x) + (b.z - a.z) * (d.z - c.z) > 0.985 * Math.hypot(b.x - a.x, b.z - a.z) * Math.hypot(d.x - c.x, d.z - c.z);
      if (straight) out.push(ribbon(line.slice(k, k + 7), 0.06, 0.0055, PAINT, 2));
    }
  };
  if (wide >= 1.8) {
    out.push(ribbon(pts, 0.07, 0.0055, CENTRE, 2));
    broken(beside(pts, wide / 4));
    broken(beside(pts, -wide / 4));
  } else {
    broken(pts);
  }
  return out.filter(Boolean);
}

// The ground a block stands on: a square lot off the street, a round one on it.
function lot(round, size, color, y, order) {
  const m = flat(round ? new THREE.CircleGeometry(size / 2, 56) : new THREE.ShapeGeometry(rounded(size, 0.2)), color, y, order);
  m.rotation.x = -Math.PI / 2;
  return m;
}

// A mark on the street pointing the way it runs.
function arrow(x, z, dx, dz, color = MARK, scale = 1) {
  const s = new THREE.Shape();
  s.moveTo(0.17, 0);
  s.lineTo(-0.09, 0.15);
  s.lineTo(-0.02, 0);
  s.lineTo(-0.09, -0.15);
  s.closePath();
  const m = flat(new THREE.ShapeGeometry(s), color, 0.006, 3);
  m.scale.setScalar(scale);
  m.rotation.set(-Math.PI / 2, 0, Math.atan2(-dz, dx));
  m.position.x = x;
  m.position.z = z;
  return m;
}

function mount(container) {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x000000, 0);
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;
  const canvas = renderer.domElement;
  container.append(canvas);
  const labels = document.createElement("div");
  labels.className = "rt-labels";
  const tip = document.createElement("div");
  tip.className = "rt-tip";
  tip.hidden = true;
  // The way lit, as a route finder gives directions: where it starts, each place it goes
  // through, where it ends. A line picked brings its block to the middle of the map.
  const guide = document.createElement("aside");
  guide.className = "rt-guide";
  guide.hidden = true;
  container.append(labels, tip, guide);

  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0xc3c2ea, 2.0));
  const sun = new THREE.DirectionalLight(0xfffaf2, 1.8);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.bias = -0.0012;
  sun.shadow.normalBias = 0.04;
  scene.add(sun, sun.target);
  // The shadows lie just over the street and the lots, so a block's shadow falls on them too.
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(2400, 2400), new THREE.ShadowMaterial({ color: 0x4b4d9a, opacity: 0.2 }));
  ground.rotation.x = -Math.PI / 2;
  ground.position.y = 0.03;
  ground.renderOrder = 10;
  ground.receiveShadow = true;
  scene.add(ground);
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 400);
  const root = new THREE.Group();
  const glow = new THREE.Group();
  scene.add(root, glow);

  const EYE = new THREE.Vector3(1, 0.82, 1).normalize().multiplyScalar(90);
  const still = window.matchMedia("(prefers-reduced-motion: reduce)");
  const view = { x: 0, z: 0, zoom: 1, toX: 0, toZ: 0, toZoom: 1, span: 6 };
  let data = { nodes: [], roads: [] };
  let nodes = [];
  let ways = [];
  let lit = [];
  let order = [];
  let flow = null;
  let dots = [];
  let clock = 0;
  let picked = null;
  let hover = null;
  let dirty = 0;
  let stale = true;
  let seenAt = "";
  const api = { container, onPick: null };
  const wake = () => (dirty = 30);

  function clear(group) {
    group.traverse((o) => {
      o.geometry?.dispose();
      o.material?.dispose();
    });
    group.clear();
  }

  // The way to the Call picked, the Call first and the request last.
  function chain(i) {
    // A Call carries the engine's whole way to it, from the request.
    if (data.nodes[i]?.way) return [...data.nodes[i].way].reverse();
    const seen = [];
    let at = i;
    while (at != null && data.nodes[at] && !seen.includes(at)) {
      seen.push(at);
      at = data.nodes[at].by;
    }
    return seen;
  }

  function relight() {
    clear(glow);
    lit = picked != null && nodes[picked] ? chain(picked) : [];
    const on = new Set(lit);
    const asked = picked != null && nodes[picked]?.kind === "ask" ? nodes[picked].turn : null;
    nodes.forEach((n, i) => {
      // A request picked keeps its whole turn in view and lets the other turns fade.
      const whole = asked != null && n.turn === asked;
      n.goal = n.ghost ? (on.has(i) ? 1 : 0) : !lit.length || on.has(i) || whole ? 1 : 0.42;
      // Drawn round in black: the Call picked, and the request its way starts at.
      const marked = i === picked || (on.has(i) && n.kind === "ask");
      n.hull.forEach((h) => (h.visible = marked));
      n.line.color.setHex(marked ? INK : EDGE);
      n.name.classList.toggle("on", on.has(i));
      n.name.removeAttribute("data-step");
      if (on.has(i) && !n.state) {
        const p = n.ghost ? lot(true, 0.9, ROUTE_LOT, 0.012, 5) : lot(ON_STREET.has(n.kind), n.size, ROUTE_LOT, 0.012, 5);
        p.position.x = n.x;
        p.position.z = n.z;
        glow.add(p);
      }
    });
    // The way to the Call picked down the roads, as the page gave it: the line on its casing,
    // and the places along it.
    flow = lit.length > 1 && nodes[picked].route ? trace(nodes[picked].route, 0.9) : [];
    const casing = ribbon(flow, 0.86, 0.014, CASING, 6);
    const line = ribbon(flow, 0.66, 0.017, ROUTE, 7);
    if (casing) glow.add(casing, line);
    // Marks on the line pointing the way it goes, moving along it unless motion is unwelcome.
    dots = [];
    if (flow.length > 8) {
      const count = Math.max(1, Math.round((flow.length * 0.12) / 2.6));
      for (let k = 0; k < count; k++) {
        const d = arrow(0, 0, 1, 0, WHITE, 1.1);
        d.position.y = 0.021;
        d.renderOrder = 8;
        glow.add(d);
        dots.push(d);
      }
      point(0);
    }
    if (!dots.length || still.matches) flow = dots.length ? flow : null;
    // A pin over the Call the way ends at, in the way's colour.
    if (lit.length > 1) {
      const end = nodes[picked];
      const tone = new THREE.MeshStandardMaterial({ color: WAYS[end.state] ?? GREEN, roughness: 0.55 });
      // Where it leaves from: a ring round the request, as a route finder marks the start.
      const start = nodes[lit[lit.length - 1]];
      const ring = flat(new THREE.RingGeometry(1.28, 1.46, 72), ROUTE, 0.019, 8);
      ring.rotation.x = -Math.PI / 2;
      ring.position.x = start.x;
      ring.position.z = start.z;
      glow.add(ring);
      const pin = new THREE.Group();
      const head = new THREE.Mesh(new THREE.SphereGeometry(0.34, 32, 20), tone);
      head.position.y = 0.66;
      const point = new THREE.Mesh(new THREE.ConeGeometry(0.2, 0.5, 28), tone);
      point.rotation.x = Math.PI;
      point.position.y = 0.25;
      const eye = new THREE.Mesh(new THREE.SphereGeometry(0.11, 20, 14), new THREE.MeshBasicMaterial({ color: WHITE }));
      eye.position.set(0.19, 0.72, 0.19);
      pin.add(head, point, eye);
      pin.position.set(end.x, end.tall + 0.3, end.z);
      glow.add(pin);
    }
    // The directions: the request it leaves from, each Call it goes through with the words
    // that sent it on, and the Call it arrives at.
    const steps = [...lit].reverse();
    guide.replaceChildren();
    guide.hidden = steps.length < 2;
    guide.className = `rt-guide ${nodes[picked]?.state ?? ""}`;
    if (steps.length > 1) {
      const end = nodes[picked];
      const calls = steps.filter((i) => nodes[i].seq != null).length;
      const head = document.createElement("header");
      const title = document.createElement("strong");
      title.textContent = `${end.label} · ${end.tip}`;
      const sum = document.createElement("span");
      const verdict = end.state === "blocked" ? "도착지에서 차단됨" : end.state === "held" ? "도착지에서 확인 요청" : "허용됨";
      sum.textContent = calls > 1 ? `호출 ${calls - 1}곳을 거쳐 도착 · ${verdict}` : `요청에서 바로 도착 · ${verdict}`;
      const all = document.createElement("button");
      all.type = "button";
      all.textContent = "전체 경로";
      all.addEventListener("click", () => show(true));
      head.append(title, sum, all);
      const list = document.createElement("ol");
      let no = 0;
      steps.forEach((i, k) => {
        const n = nodes[i];
        const li = document.createElement("li");
        const mark = document.createElement("i");
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
          n.name.dataset.step = no;
          mark.textContent = no;
          const role = k === 0 ? "출발" : i === picked ? "도착" : "경유";
          if (i === picked) li.className = "end";
          name.textContent = `${role} · ${n.label}`;
          what.textContent = n.tip;
          li.tabIndex = 0;
          const go = () => focus(i);
          li.addEventListener("click", go);
          li.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && go());
        }
        body.append(name, what);
        if (k === 0 && n.intent) {
          const intent = document.createElement("span");
          intent.className = "intent";
          intent.textContent = `의도 · ${n.intent}`;
          body.append(intent);
        }
        li.append(mark, body);
        list.append(li);
      });
      guide.append(head, list);
    }
    // Which names give way when two would overlap: the lit way first, then the street's own.
    const rank = (i) => (on.has(i) ? 0 : ON_STREET.has(nodes[i].kind) || nodes[i].kind === "many" ? 1 : nodes[i].state ? 2 : 3);
    order = nodes.map((_, i) => i).sort((a, b) => rank(a) - rank(b) || a - b);
    ways.forEach((w) => (w.goal = (lit.length ? 0.7 : 1) * (w.most ?? 1)));
    stale = true;
    wake();
  }

  // The whole session when it fits; when it is longer than that, its start.
  function fit() {
    if (!nodes.length) return;
    const us = nodes.map((n) => (n.x - n.z) * TURN);
    const lo = Math.min(...us);
    const hi = Math.max(...us);
    const aspect = canvas.clientWidth / Math.max(1, canvas.clientHeight);
    const need = ((hi - lo) / 2 + 3) / aspect;
    // How far the roads reach from the street, toward the eye and away; half as tall on the page.
    const vs = nodes.map((n) => (n.x + n.z) * TURN);
    const near = Math.max(...vs);
    const far = Math.min(...vs);
    const deep = (near - far) / 4 + 2.4;
    view.span = THREE.MathUtils.clamp(Math.max(need, deep), 5.4, 9.5);
    const u = need <= view.span ? (lo + hi) / 2 : lo + view.span * aspect - 3.5;
    const v = (near + far) / 2;
    view.toX = (u + v) * TURN;
    view.toZ = (v - u) * TURN;
    view.toZoom = 1;
    wake();
  }

  api.setData = (next) => {
    clear(root);
    clear(glow);
    labels.replaceChildren();
    data = next;
    hover = null;
    picked = null;
    tip.hidden = true;
    ways = [];
    const add = (mesh) => {
      if (!mesh) return;
      root.add(mesh);
      ways.push({ way: mesh, at: 1, goal: 1 });
    };
    // The road up the left that joins the turns' requests, then every turn's street and forks.
    const asks = next.nodes.filter((n) => n.kind === "ask").map((n) => [n.x, n.z]);
    if (asks.length > 1) road(trace(asks), 2.1).forEach(add);
    // Its corners round enough that the inside of the bend does not fold over.
    for (const r of next.roads) road(trace(r.points, r.wide / 2 + 0.7), r.wide).forEach(add);
    nodes = next.nodes.map((n, i) => {
      const b = block(n.kind, !!n.intent);
      b.g.position.set(n.x, 0, n.z);
      b.g.traverse((o) => (o.userData.at = i));
      root.add(b.g);
      // A request stands on a round place in its street; a building on a lot beside its road.
      const street = ON_STREET.has(n.kind);
      const size = street ? 2.5 : n.kind === "many" ? 3.6 : 1.84;
      if (!n.ghost) {
        if (!n.state) {
          const kerb = lot(street, size + 0.12, KERB, 0.006, 3);
          kerb.position.x = n.x;
          kerb.position.z = n.z;
          add(kerb);
        }
        const ground = lot(street, size, n.state ? LOTS[n.state] : LOT, 0.008, 4);
        ground.position.x = n.x;
        ground.position.z = n.z;
        add(ground);
      }
      const name = document.createElement("span");
      name.textContent = n.label;
      name.classList.add(street || n.ghost || n.kind === "many" ? n.kind : "call");
      if (n.state) name.classList.add(n.state);
      labels.append(name);
      return { ...n, ...b, i, name, size, wide: name.offsetWidth, at: n.ghost ? 0 : 1, goal: n.ghost ? 0 : 1, shown: true };
    });
    fit();
    view.x = view.toX;
    view.z = view.toZ;
    relight();
  };
  // Stands the marks along the lit way as they are `t` seconds on, each pointing the way.
  function point(t) {
    if (!flow?.length) return;
    const long = flow.length * 0.12;
    dots.forEach((d, k) => {
      const at = Math.floor((((t * 1.6) / long + k / dots.length) % 1) * (flow.length - 2));
      const [here, next] = [flow[at], flow[at + 1]];
      d.position.x = here.x;
      d.position.z = here.z;
      if (next.x !== here.x || next.z !== here.z) d.rotation.z = Math.atan2(-(next.z - here.z), next.x - here.x);
    });
  }

  // Brings one block to the middle of the map, near enough to read.
  function focus(i) {
    const n = nodes[i];
    if (!n) return;
    view.toX = n.x;
    view.toZ = n.z;
    view.toZoom = Math.max(view.toZoom, 1.35);
    wake();
  }

  // Brings the way lit into view: always when `whole`, else only when some of it is outside.
  // Across the page by where its blocks stand, up and down by how far they are from the eye.
  function show(whole) {
    if (!lit.length) return;
    const us = lit.map((k) => (nodes[k].x - nodes[k].z) * TURN);
    const vs = lit.map((k) => (nodes[k].x + nodes[k].z) * TURN);
    const [u0, u1, v0, v1] = [Math.min(...us), Math.max(...us), Math.min(...vs), Math.max(...vs)];
    const zoom = whole ? 1.6 : view.toZoom;
    const wide = (view.span * canvas.clientWidth) / Math.max(1, canvas.clientHeight) / zoom;
    const deep = (view.span * 2) / zoom;
    const u = (view.toX - view.toZ) * TURN;
    const v = (view.toX + view.toZ) * TURN;
    // The directions card covers the map's right: the way keeps to the left of it.
    const card = guide.hidden ? 0 : (guide.offsetWidth + 40) / Math.max(1, canvas.clientWidth);
    if (whole || u0 < u - wide + 2.5 || u1 > u + wide * (1 - 2 * card) - 2.5 || v0 < v - deep + 5 || v1 > v + deep - 4) {
      // Zoomed out as far as the way needs, no closer than it was (or than reads well).
      const need = Math.max((u1 - u0 + 6) / (2 * wide * (1 - card) * zoom), (v1 - v0 + 9) / (2 * deep * zoom), 1);
      const next = Math.max(0.4, Math.min(zoom, zoom / need));
      view.toZoom = next;
      const half = (view.span * canvas.clientWidth) / Math.max(1, canvas.clientHeight) / next;
      const [cu, cv] = [(u0 + u1) / 2 + half * card, (v0 + v1) / 2];
      view.toX = (cu + cv) * TURN;
      view.toZ = (cv - cu) * TURN;
      wake();
    }
  }

  api.setPicked = (i) => {
    if (i === picked) return;
    picked = i;
    relight();
    if (i != null && nodes[i]) show(false);
  };
  api.zoomBy = (f) => {
    view.toZoom = THREE.MathUtils.clamp(view.toZoom * f, 0.4, 3.2);
    wake();
  };
  api.fit = fit;

  const ray = new THREE.Raycaster();
  const at = new THREE.Vector2();
  function under(e) {
    const box = canvas.getBoundingClientRect();
    at.set(((e.clientX - box.left) / box.width) * 2 - 1, -((e.clientY - box.top) / box.height) * 2 + 1);
    ray.setFromCamera(at, camera);
    const hit = ray.intersectObjects(nodes.filter((n) => n.g.visible).map((n) => n.g), true).find((h) => h.object.isMesh && h.object.visible);
    return hit ? hit.object.userData.at : null;
  }
  let drag = null;
  let moved = 0;
  const right = new THREE.Vector3();
  const fore = new THREE.Vector3();
  canvas.addEventListener("pointerdown", (e) => {
    drag = { x: e.clientX, y: e.clientY, cx: view.toX, cz: view.toZ };
    moved = 0;
    canvas.setPointerCapture(e.pointerId);
    container.classList.add("moving");
  });
  canvas.addEventListener("pointermove", (e) => {
    if (drag) {
      moved = Math.max(moved, Math.abs(e.clientX - drag.x) + Math.abs(e.clientY - drag.y));
      // A drag moves the floor under the pointer: across along the eye's right, up along its
      // forward, both laid flat on the floor.
      const per = ((camera.top - camera.bottom) / camera.zoom) / canvas.clientHeight;
      right.setFromMatrixColumn(camera.matrixWorld, 0).setY(0).normalize();
      fore.copy(EYE).setY(0).normalize().negate();
      const dx = -(e.clientX - drag.x) * per;
      const dy = ((e.clientY - drag.y) * per) / Math.sin(Math.atan2(EYE.y, Math.hypot(EYE.x, EYE.z)));
      view.toX = drag.cx + right.x * dx + fore.x * dy;
      view.toZ = drag.cz + right.z * dx + fore.z * dy;
      view.x = view.toX;
      view.z = view.toZ;
      tip.hidden = true;
      wake();
      return;
    }
    const i = under(e);
    if (i === hover) return;
    hover = i;
    canvas.style.cursor = i == null ? "" : nodes[i].pick != null ? "pointer" : "default";
    tip.hidden = i == null;
    if (i != null) {
      tip.replaceChildren();
      const n = nodes[i];
      const rows = [["small", n.kind === "words" ? "도구 결과의 문단" : n.label], ["strong", n.tip], ["span", n.note]];
      if (n.intent) rows.push(["small", "에이전트가 적은 의도"], ["strong", n.intent], ["span", "엔진은 호출을 이것과 견줍니다"]);
      for (const [tag, text] of rows) {
        if (!text) continue;
        const el = document.createElement(tag);
        el.textContent = text;
        tip.append(el);
      }
    }
    wake();
  });
  canvas.addEventListener("pointerup", (e) => {
    if (drag && moved < 5) {
      const i = under(e);
      // What the page gave the block to be picked by: a Call's seq, a request's or a large
      // block's own key.
      if (i != null && nodes[i].pick != null) api.onPick?.(nodes[i].pick);
    }
    drag = null;
    container.classList.remove("moving");
  });
  canvas.addEventListener("pointerleave", () => {
    hover = null;
    tip.hidden = true;
  });
  canvas.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      api.zoomBy(Math.exp(-e.deltaY * 0.0022));
    },
    { passive: false },
  );

  const p = new THREE.Vector3();
  function project(x, y, z) {
    p.set(x, y, z).project(camera);
    return [((p.x + 1) / 2) * canvas.clientWidth, ((1 - p.y) / 2) * canvas.clientHeight];
  }
  let last = performance.now();
  function frame(now) {
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    if (flow) dirty = Math.max(dirty, 2);
    if (dirty <= 0) return;
    dirty--;
    const ease = 1 - Math.exp(-dt * 8);
    view.x += (view.toX - view.x) * ease;
    view.z += (view.toZ - view.z) * ease;
    view.zoom += (view.toZoom - view.zoom) * ease;
    if (Math.abs(view.toX - view.x) + Math.abs(view.toZ - view.z) + Math.abs(view.toZoom - view.zoom) > 0.002) dirty = Math.max(dirty, 2);
    const aspect = canvas.clientWidth / Math.max(1, canvas.clientHeight);
    camera.left = -view.span * aspect;
    camera.right = view.span * aspect;
    camera.top = view.span;
    camera.bottom = -view.span;
    camera.zoom = view.zoom;
    camera.position.set(view.x + EYE.x, EYE.y, view.z + EYE.z);
    camera.lookAt(view.x, 0, view.z);
    camera.updateProjectionMatrix();
    camera.updateMatrixWorld();
    sun.position.set(view.x - 16, 30, view.z + 6);
    sun.target.position.set(view.x, 0, view.z);
    const reach = (view.span * Math.max(1, aspect)) / view.zoom + 6;
    Object.assign(sun.shadow.camera, { left: -reach, right: reach, top: reach, bottom: -reach, near: 1, far: 90 });
    sun.shadow.camera.updateProjectionMatrix();
    const here = `${view.x.toFixed(3)} ${view.z.toFixed(3)} ${view.zoom.toFixed(4)} ${canvas.clientWidth} ${canvas.clientHeight}`;
    if (here !== seenAt) {
      seenAt = here;
      stale = true;
    }
    nodes.forEach((n, i) => {
      if (Math.abs(n.goal - n.at) > 0.004) dirty = Math.max(dirty, 2);
      n.at += (n.goal - n.at) * ease;
      n.g.visible = n.at > 0.02;
      for (const m of n.mats) m.opacity = n.at;
      // Its name sits under it on the page; the words' name beside them, clear of the lane.
      const [sx, sy] = n.ghost ? project(n.x + 0.42, 0, n.z + 0.42) : project(n.x + (n.kind === "many" ? 1.5 : 0.72), 0, n.z + (n.kind === "many" ? 1.5 : 0.72));
      n.sx = sx;
      n.sy = sy;
      n.name.style.transform = `translate(-50%, 0) translate(${n.sx}px, ${n.sy}px)`;
      // Where the block itself is on the page, from its top to its foot.
      const [bx, top] = project(n.x, n.tall, n.z);
      const reach = n.kind === "many" ? 1.35 : 0.5;
      const foot = project(n.x + reach, 0, n.z + reach)[1];
      const half = (project(n.x + reach, 0, n.z - reach)[0] - bx) * 1.05;
      n.box = [bx - half, top, bx + half, foot];
      if (i === hover) {
        const [tx, ty] = project(n.x, n.tall + 0.35, n.z);
        tip.style.transform = `translate(-50%, -100%) translate(${tx}px, ${ty}px)`;
      }
    });
    if (stale) {
      stale = false;
      // A name shows only where no name before it in the order already is.
      const boxes = [];
      for (const i of order) {
        const n = nodes[i];
        const lead = lit.includes(i);
        const box = [n.sx - n.wide / 2 - 3, n.sy - 1, n.sx + n.wide / 2 + (lead ? 22 : 3), n.sy + 19];
        const over = (b) => box[0] < b[2] && b[0] < box[2] && box[1] < b[3] && b[1] < box[3];
        // Nor where a block nearer the eye stands over it.
        const behind = () => nodes.some((m) => m !== n && m.g.visible && m.x + m.z > n.x + n.z + 0.4 && over(m.box));
        const must = lead && (i === picked || n.ghost || ON_STREET.has(n.kind));
        const show = must || (!n.ghost && !boxes.some(over) && (lead || !behind()));
        if (show) boxes.push(box);
        n.name.style.opacity = !show ? 0 : n.goal < 0.7 ? 0.4 : 1;
      }
    }
    for (const w of ways) {
      w.at += (w.goal - w.at) * ease;
      w.way.material.opacity = w.at;
    }
    if (flow && dots.length && !still.matches) {
      clock += dt;
      point(clock);
    }
    renderer.render(scene, camera);
  }

  function resize() {
    const w = container.clientWidth;
    const h = container.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h, false);
    wake();
  }
  const sized = new ResizeObserver(resize);
  sized.observe(container);
  resize();
  let live = true;
  const run = () => {
    last = performance.now();
    wake();
    renderer.setAnimationLoop(live && !document.hidden ? frame : null);
  };
  const seen = new IntersectionObserver(([entry]) => {
    live = entry.isIntersecting;
    run();
  });
  seen.observe(container);
  document.addEventListener("visibilitychange", run);
  canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    renderer.setAnimationLoop(null);
    container.classList.add("lost");
  });
  run();

  api.dispose = () => {
    renderer.setAnimationLoop(null);
    sized.disconnect();
    seen.disconnect();
    document.removeEventListener("visibilitychange", run);
    clear(root);
    clear(glow);
    renderer.dispose();
    canvas.remove();
    labels.remove();
    tip.remove();
    guide.remove();
  };
  return api;
}

window.WHRoute = { mount };
