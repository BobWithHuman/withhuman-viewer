// 기록 as a map seen from above at an angle. One street runs through the session from left to
// right; each turn's request, the agent's intent and its answer stand on it, and the Calls
// stand off it on lots of their own, each at the end of a lane. A block picked is drawn round
// in black, and what told the agent to make that Call is lit in green: the lots under the
// blocks it followed, the way between them along the street, and, in the Call's lane, the
// words of the tool result that told it. The same way is written out as a line of names at
// the foot. Calls the engine stopped stand on a red lot, those it held on an amber one. The
// page gives the blocks and where they stand (admin/route.rs); this file only draws them.
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";

const WHITE = 0xffffff;
const EDGE = 0xb9bfc6;
const INK = 0x15171a;
const STREET = 0xfbfcfc;
const LOT = 0xf1f3f5;
const KERB = 0xd3d7dc;
const MARK = 0xbcc2c9;
const GREEN = 0x3fc47c;
const GREEN_LOT = 0xa9ebc4;
const LOTS = { blocked: 0xf6aaa3, held: 0xf7d488 };
// The eye looks down a diagonal of the floor: one step to the right on the page is this much
// along x and back along z.
const TURN = Math.SQRT1_2;
const ON_STREET = new Set(["ask", "intent", "say"]);

const white = () => new THREE.MeshStandardMaterial({ color: WHITE, roughness: 0.86, metalness: 0, transparent: true });
const grey = () => new THREE.MeshStandardMaterial({ color: 0xd5d9de, roughness: 0.9, metalness: 0, transparent: true });

function solid(geometry, material, x, y, z) {
  const m = new THREE.Mesh(geometry, material);
  m.position.set(x, y, z);
  m.castShadow = true;
  return m;
}

// Each kind of block, standing at its own origin; returns its meshes' group and its height.
function block(kind) {
  const g = new THREE.Group();
  let tall = 1;
  if (kind === "ask") {
    for (const [r, h, y] of [[0.78, 0.26, 0.13], [0.5, 0.2, 0.36]]) {
      const hex = solid(new THREE.CylinderGeometry(r, r, h, 6), white(), 0, y, 0);
      hex.rotation.y = Math.PI / 6;
      g.add(hex);
    }
    tall = 0.46;
  } else if (kind === "intent") {
    // A pin stood in the street: where the agent said it was going.
    g.add(solid(new THREE.CylinderGeometry(0.46, 0.46, 0.1, 40), white(), 0, 0.05, 0));
    g.add(solid(new THREE.CylinderGeometry(0.035, 0.035, 0.86, 12), grey(), 0, 0.53, 0));
    g.add(solid(new THREE.SphereGeometry(0.21, 32, 20), white(), 0, 1.08, 0));
    tall = 1.3;
  } else if (kind === "say") {
    g.add(solid(new THREE.CylinderGeometry(0.05, 0.05, 0.34, 12), grey(), 0, 0.17, 0));
    g.add(solid(new RoundedBoxGeometry(1.0, 0.6, 0.6, 6, 0.24), white(), 0, 0.64, 0));
    tall = 0.94;
  } else if (kind === "words") {
    // A page stood on its edge: the words read out of a tool result.
    const page = solid(new RoundedBoxGeometry(0.5, 0.64, 0.05, 3, 0.02), white(), 0, 0.34, 0);
    page.rotation.y = Math.PI / 4;
    g.add(page);
    for (let k = 0; k < 3; k++) {
      const row = solid(new RoundedBoxGeometry(0.3 - k * 0.06, 0.03, 0.012, 1, 0.004), grey(), 0.022, 0.5 - k * 0.12, 0.022);
      row.rotation.y = Math.PI / 4;
      g.add(row);
    }
    tall = 0.66;
  } else if (kind === "read") {
    g.add(solid(new RoundedBoxGeometry(1.0, 0.16, 1.3, 3, 0.04), white(), 0, 0.08, 0));
    g.add(solid(new RoundedBoxGeometry(0.9, 0.12, 1.18, 3, 0.04), white(), 0.06, 0.24, -0.05));
    tall = 0.3;
  } else if (kind === "change") {
    g.add(solid(new RoundedBoxGeometry(0.95, 1.55, 0.95, 4, 0.07), white(), 0, 0.775, 0));
    g.add(solid(new RoundedBoxGeometry(0.03, 0.5, 0.05, 1, 0.01), grey(), -0.34, 1.05, 0.49));
    tall = 1.55;
  } else if (kind === "run") {
    g.add(solid(new RoundedBoxGeometry(1.1, 1.0, 1.1, 4, 0.1), white(), 0, 0.5, 0));
    const dial = solid(new THREE.CylinderGeometry(0.26, 0.26, 0.03, 40), grey(), 0, 0.5, 0.56);
    dial.rotation.x = Math.PI / 2;
    g.add(dial);
    tall = 1.0;
  } else if (kind === "outside") {
    g.add(solid(new RoundedBoxGeometry(0.8, 1.8, 0.8, 4, 0.06), white(), 0, 0.9, 0));
    for (let k = 0; k < 3; k++) g.add(solid(new RoundedBoxGeometry(0.46, 0.04, 0.03, 1, 0.01), grey(), 0, 1.5 - k * 0.14, 0.41));
    tall = 1.8;
  } else if (kind === "delegate") {
    g.add(solid(new RoundedBoxGeometry(1.0, 1.3, 0.5, 6, 0.22), white(), 0, 0.65, 0));
    tall = 1.3;
  } else {
    g.add(solid(new RoundedBoxGeometry(1.0, 0.5, 1.0, 5, 0.16), white(), 0, 0.5, 0));
    for (const sx of [-0.3, 0.3]) g.add(solid(new THREE.CylinderGeometry(0.035, 0.035, 0.3, 12), grey(), sx, 0.15, 0.2));
    tall = 0.75;
  }
  // Drawn edges, so a white block reads on a pale floor; black and doubled when picked.
  const lines = [];
  g.traverse((o) => {
    if (!o.isMesh) return;
    const e = new THREE.LineSegments(
      new THREE.EdgesGeometry(o.geometry, 28),
      new THREE.LineBasicMaterial({ color: EDGE, transparent: true }),
    );
    e.position.copy(o.position);
    e.rotation.copy(o.rotation);
    lines.push(e);
  });
  lines.forEach((e) => g.add(e));
  const hull = [];
  g.traverse((o) => {
    if (!o.isMesh) return;
    const h = new THREE.Mesh(o.geometry, new THREE.MeshBasicMaterial({ color: INK, side: THREE.BackSide }));
    h.position.copy(o.position);
    h.rotation.copy(o.rotation);
    h.scale.setScalar(1.05);
    h.visible = false;
    hull.push(h);
  });
  hull.forEach((h) => g.add(h));
  return { g, tall, lines, hull };
}

// Evenly spaced places along the floor through the points given, the corners rounded.
function trace(points) {
  const v = [];
  for (const [x, z] of points) {
    const p = new THREE.Vector3(x, 0, z);
    if (!v.length || v[v.length - 1].distanceTo(p) > 0.04) v.push(p);
  }
  if (v.length < 2) return [];
  const path = new THREE.CurvePath();
  const r = 0.42;
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

// The ground a block stands on: a square lot off the street, a round one on it.
function lot(round, size, color, y, order) {
  const m = flat(round ? new THREE.CircleGeometry(size / 2, 56) : new THREE.ShapeGeometry(rounded(size, 0.2)), color, y, order);
  m.rotation.x = -Math.PI / 2;
  return m;
}

// A mark on the street pointing the way it runs.
function arrow(x, z, dx, dz) {
  const s = new THREE.Shape();
  s.moveTo(0.17, 0);
  s.lineTo(-0.09, 0.15);
  s.lineTo(-0.02, 0);
  s.lineTo(-0.09, -0.15);
  s.closePath();
  const m = flat(new THREE.ShapeGeometry(s), MARK, 0.006, 3);
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
  const trail = document.createElement("ol");
  trail.className = "rt-trail";
  trail.hidden = true;
  container.append(labels, tip, trail);

  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0xdfe3e8, 2.0));
  const sun = new THREE.DirectionalLight(0xffffff, 1.7);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.bias = -0.0012;
  sun.shadow.normalBias = 0.04;
  scene.add(sun, sun.target);
  // The shadows lie just over the street and the lots, so a block's shadow falls on them too.
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(2400, 2400), new THREE.ShadowMaterial({ opacity: 0.16 }));
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
  let data = { nodes: [], street: { from: 0, step: 1, points: [] } };
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

  // What told the agent to make the Call picked, each block naming the one before it.
  function chain(i) {
    const seen = [];
    let at = i;
    while (at != null && data.nodes[at] && !seen.includes(at)) {
      seen.push(at);
      at = data.nodes[at].by;
    }
    return seen;
  }

  // The way from one block to another: down its lane, along the street, up the other's lane.
  function way(a, b) {
    if (Math.abs(a.u - b.u) < 1e-6) return [[a.x, a.z], [b.x, b.z]];
    const s = data.street;
    const lo = Math.min(a.u, b.u) + 0.2;
    const hi = Math.max(a.u, b.u) - 0.2;
    const mid = [];
    for (let k = Math.max(0, Math.ceil((lo - s.from) / s.step)); k < s.points.length && s.from + k * s.step <= hi; k++) mid.push(s.points[k]);
    if (a.u > b.u) mid.reverse();
    return [[a.x, a.z], [a.jx, a.jz], ...mid, [b.jx, b.jz], [b.x, b.z]];
  }

  function relight() {
    clear(glow);
    lit = picked != null && nodes[picked] ? chain(picked) : [];
    const on = new Set(lit);
    nodes.forEach((n, i) => {
      n.goal = n.ghost ? (on.has(i) ? 1 : 0) : !lit.length || on.has(i) ? 1 : 0.42;
      n.hull.forEach((h) => (h.visible = i === picked));
      n.lines.forEach((l) => l.material.color.setHex(i === picked ? INK : EDGE));
      n.name.classList.toggle("on", on.has(i));
      n.name.removeAttribute("data-step");
      if (on.has(i) && !n.state) {
        const p = n.ghost ? lot(true, 0.9, GREEN_LOT, 0.012, 4) : lot(ON_STREET.has(n.kind), n.size, GREEN_LOT, 0.012, 4);
        p.position.x = n.x;
        p.position.z = n.z;
        glow.add(p);
      }
    });
    // From the request to the Call picked, as one way: its band, and the places along it.
    flow = [];
    for (let k = lit.length - 1; k > 0; k--) {
      const pts = trace(way(nodes[lit[k]], nodes[lit[k - 1]]));
      const band = ribbon(pts, 0.24, 0.016, GREEN, 5);
      if (band) glow.add(band);
      flow.push(...pts);
    }
    dots = [];
    if (flow.length > 8 && !still.matches) {
      const count = Math.max(1, Math.round((flow.length * 0.12) / 4.5));
      for (let k = 0; k < count; k++) {
        const d = flat(new THREE.CircleGeometry(0.075, 20), WHITE, 0.02, 6);
        d.rotation.x = -Math.PI / 2;
        glow.add(d);
        dots.push(d);
      }
    }
    if (!dots.length) flow = null;
    // The same way written out, from the request to the Call picked.
    trail.replaceChildren();
    trail.hidden = lit.length < 2;
    [...lit].reverse().forEach((i, k) => {
      const n = nodes[i];
      n.name.dataset.step = k + 1;
      const li = document.createElement("li");
      if (i === picked) li.className = "end";
      if (n.kind === "words") li.classList.add("words");
      const no = document.createElement("i");
      no.textContent = k + 1;
      const what = document.createElement("span");
      what.textContent = n.label;
      li.append(no, what);
      trail.append(li);
    });
    // Which names give way when two would overlap: the lit way first, then the street's own.
    const rank = (i) => (on.has(i) ? 0 : ON_STREET.has(nodes[i].kind) ? 1 : nodes[i].state ? 2 : 3);
    order = nodes.map((_, i) => i).sort((a, b) => rank(a) - rank(b) || a - b);
    ways.forEach((w) => (w.goal = lit.length ? 0.7 : 1));
    stale = true;
    wake();
  }

  // The whole session when it fits; when it is longer than that, its start.
  function fit() {
    if (!nodes.length) return;
    const us = nodes.map((n) => n.u);
    const lo = Math.min(...us);
    const hi = Math.max(...us);
    const aspect = canvas.clientWidth / Math.max(1, canvas.clientHeight);
    const need = ((hi - lo) / 2 + 3) / aspect;
    view.span = THREE.MathUtils.clamp(need, 5.4, 8.2);
    const u = need <= 8.2 ? (lo + hi) / 2 : lo + view.span * aspect - 3.5;
    view.toX = u * TURN;
    view.toZ = -u * TURN;
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
    // The street through the session, and marks on it pointing the way it runs.
    const pts = next.street.points;
    const run = trace(pts);
    add(ribbon(run, 1.02, 0.002, KERB, 0));
    add(ribbon(run, 0.92, 0.004, STREET, 1));
    const onStreet = next.nodes.filter((n) => ON_STREET.has(n.kind)).map((n) => n.u);
    for (let k = 3; k + 1 < pts.length; k += 6) {
      const u = next.street.from + k * next.street.step;
      if (onStreet.some((s) => Math.abs(s - u) < 1.5)) continue;
      add(arrow(pts[k][0], pts[k][1], pts[k + 1][0] - pts[k - 1][0], pts[k + 1][1] - pts[k - 1][1]));
    }
    nodes = next.nodes.map((n, i) => {
      const b = block(n.kind);
      b.g.position.set(n.x, 0, n.z);
      b.g.traverse((o) => (o.userData.at = i));
      root.add(b.g);
      const street = ON_STREET.has(n.kind);
      const size = street ? 1.9 : 1.84;
      if (!n.ghost) {
        // Its lane to the street, and the lot it stands on.
        if (!street) {
          const lane = trace([[n.jx, n.jz], [n.x, n.z]]);
          add(ribbon(lane, 0.4, 0.002, KERB, 0));
          add(ribbon(lane, 0.3, 0.004, STREET, 1));
        }
        if (!n.state) {
          const kerb = lot(street, size + 0.1, KERB, 0.006, 2);
          kerb.position.x = n.x;
          kerb.position.z = n.z;
          add(kerb);
        }
        const ground = lot(street, size, n.state ? LOTS[n.state] : street ? STREET : LOT, 0.008, 3);
        ground.position.x = n.x;
        ground.position.z = n.z;
        add(ground);
      }
      const name = document.createElement("span");
      name.textContent = n.label;
      name.classList.add(street || n.ghost ? n.kind : "call");
      if (n.state) name.classList.add(n.state);
      labels.append(name);
      return { ...n, ...b, name, size, wide: name.offsetWidth, at: n.ghost ? 0 : 1, goal: n.ghost ? 0 : 1, shown: true };
    });
    fit();
    view.x = view.toX;
    view.z = view.toZ;
    relight();
  };
  api.setPicked = (i) => {
    if (i === picked) return;
    picked = i;
    relight();
    // On a map too long to see whole, the way lit is brought to the middle.
    if (i != null && nodes[i] && lit.length) {
      const us = lit.map((k) => nodes[k].u);
      const u = (Math.min(...us) + Math.max(...us)) / 2;
      const half = (view.span * canvas.clientWidth) / Math.max(1, canvas.clientHeight) / view.zoom;
      const at = (view.toX - view.toZ) * TURN;
      if (Math.min(...us) < at - half + 2 || Math.max(...us) > at + half - 2) {
        view.toX = u * TURN;
        view.toZ = -u * TURN;
      }
    }
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
    canvas.style.cursor = i == null ? "" : nodes[i].seq != null ? "pointer" : "default";
    tip.hidden = i == null;
    if (i != null) {
      tip.replaceChildren();
      for (const [tag, text] of [["small", nodes[i].kind === "words" ? "도구 결과의 문단" : nodes[i].label], ["strong", nodes[i].tip], ["span", nodes[i].note]]) {
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
      if (i != null && nodes[i].seq != null) api.onPick?.(nodes[i].seq);
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
      n.g.traverse((o) => o.material && o.material.transparent && (o.material.opacity = n.at));
      // Its name sits under it on the page; the words' name beside them, clear of the lane.
      const [sx, sy] = n.ghost ? project(n.x + 0.42, 0, n.z + 0.42) : project(n.x + 0.72, 0, n.z + 0.72);
      n.sx = sx;
      n.sy = sy;
      n.name.style.transform = `translate(-50%, 0) translate(${n.sx}px, ${n.sy}px)`;
      // Where the block itself is on the page, from its top to its foot.
      const [bx, top] = project(n.x, n.tall, n.z);
      const foot = project(n.x + 0.5, 0, n.z + 0.5)[1];
      const half = (project(n.x + 0.5, 0, n.z - 0.5)[0] - bx) * 1.05;
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
        const show = n.ghost ? lead : lead || !(boxes.some(over) || behind());
        if (show) boxes.push(box);
        n.name.style.opacity = !show ? 0 : n.goal < 0.7 ? 0.4 : 1;
      }
    }
    for (const w of ways) {
      w.at += (w.goal - w.at) * ease;
      w.way.material.opacity = w.at;
    }
    if (flow) {
      // Marks that run along the lit way toward the Call picked.
      clock += dt;
      const long = flow.length * 0.12;
      dots.forEach((d, k) => {
        const t = ((clock * 2.4) / long + k / dots.length) % 1;
        const at = flow[Math.floor(t * (flow.length - 1))];
        d.position.x = at.x;
        d.position.z = at.z;
      });
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
    trail.remove();
  };
  return api;
}

window.WHRoute = { mount };
