// 기록 as a map seen from above at an angle: a session's requests and Calls stood as white
// blocks on a dotted floor, in the order made, a path running from each to the next. A block
// picked is drawn round in black, and what told the agent to make that Call is lit in green:
// the pads under the request and the Calls it followed, and the path from each to the next.
// Calls the engine stopped stand on a red pad, those it held on an amber one. The page gives
// the blocks and where they stand (admin/route.rs); this file only draws them.
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";

const WHITE = 0xffffff;
const EDGE = 0xc9ced3;
const INK = 0x15171a;
const ROAD = 0xe1e4e7;
const GREEN = 0x86e0a8;
const PADS = { blocked: 0xf5a19a, held: 0xf6cf7a };

const white = () => new THREE.MeshStandardMaterial({ color: WHITE, roughness: 0.9, metalness: 0, transparent: true });
const grey = () => new THREE.MeshStandardMaterial({ color: 0xdfe2e5, roughness: 0.9, metalness: 0, transparent: true });

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
    const hex = solid(new THREE.CylinderGeometry(0.72, 0.72, 0.34, 6), white(), 0, 0.17, 0);
    hex.rotation.y = Math.PI / 6;
    g.add(hex);
    tall = 0.34;
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
  // Drawn edges, so a white block reads on a white floor; black and doubled when picked.
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

// A flat band along the floor through the points given, its corners rounded.
function band(points, width, y, color) {
  const path = new THREE.CurvePath();
  const v = points.map(([x, z]) => new THREE.Vector3(x, y, z));
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
  if (!(long > 0.01)) return null;
  const pts = path.getSpacedPoints(Math.max(2, Math.ceil(long / 0.12)));
  const pos = [];
  const index = [];
  pts.forEach((p, i) => {
    const a = pts[Math.max(0, i - 1)];
    const b = pts[Math.min(pts.length - 1, i + 1)];
    const nx = -(b.z - a.z);
    const nz = b.x - a.x;
    const n = Math.hypot(nx, nz) || 1;
    pos.push(p.x + (nx / n) * (width / 2), y, p.z + (nz / n) * (width / 2));
    pos.push(p.x - (nx / n) * (width / 2), y, p.z - (nz / n) * (width / 2));
    if (i) index.push(2 * i - 2, 2 * i - 1, 2 * i, 2 * i - 1, 2 * i + 1, 2 * i);
  });
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geo.setIndex(index);
  return new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color, transparent: true, side: THREE.DoubleSide, depthWrite: false }));
}

function pad(round, color) {
  const geo = round ? new THREE.CircleGeometry(1.0, 56) : new THREE.ShapeGeometry(rounded(1.9, 0.12));
  const m = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color, transparent: true, depthWrite: false }));
  m.rotation.x = -Math.PI / 2;
  return m;
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
  container.append(labels, tip);

  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0xe9ecef, 2.1));
  const sun = new THREE.DirectionalLight(0xffffff, 1.5);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.bias = -0.0012;
  sun.shadow.normalBias = 0.04;
  scene.add(sun, sun.target);
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(400, 400), new THREE.ShadowMaterial({ opacity: 0.13 }));
  ground.rotation.x = -Math.PI / 2;
  ground.receiveShadow = true;
  scene.add(ground);
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 400);
  const root = new THREE.Group();
  scene.add(root);

  const EYE = new THREE.Vector3(1, 0.82, 1).normalize().multiplyScalar(90);
  const view = { x: 0, z: 0, zoom: 1, toX: 0, toZ: 0, toZoom: 1, span: 9 };
  let data = { nodes: [] };
  let nodes = [];
  let roads = [];
  let lit = [];
  let picked = null;
  let hover = null;
  let dirty = 0;
  const api = { container, onPick: null };
  const wake = () => (dirty = 30);

  function clear(group) {
    group.traverse((o) => {
      o.geometry?.dispose();
      o.material?.dispose();
    });
    group.clear();
  }

  // What told the agent to make the Call picked: the Calls it followed, back to the request.
  function chain(i) {
    const seen = [];
    let at = i;
    while (at != null && !seen.includes(at)) {
      seen.push(at);
      at = data.nodes[at].by;
    }
    const ask = data.nodes.findIndex((n) => n.kind === "ask" && n.turn === data.nodes[i].turn);
    if (ask >= 0 && !seen.includes(ask)) seen.push(ask);
    return seen;
  }

  // A path from one block to another that keeps to the aisles between the rows.
  function aisle(a, b) {
    if (Math.abs(a.z - b.z) < 0.01) {
      const z = a.z + data.aisle;
      return [[a.x, a.z], [a.x, z], [b.x, z], [b.x, b.z]];
    }
    const z = b.z + (a.z < b.z ? -data.aisle : data.aisle);
    return Math.abs(a.x - b.x) < 0.01 ? [[a.x, a.z], [b.x, b.z]] : [[a.x, a.z], [a.x, z], [b.x, z], [b.x, b.z]];
  }

  const glow = new THREE.Group();
  scene.add(glow);
  function relight() {
    clear(glow);
    lit = picked != null && nodes[picked] ? chain(picked) : [];
    const on = new Set(lit);
    nodes.forEach((n, i) => {
      n.goal = !lit.length || on.has(i) ? 1 : 0.42;
      n.hull.forEach((h) => (h.visible = i === picked));
      n.lines.forEach((l) => l.material.color.setHex(i === picked ? INK : EDGE));
      n.name.classList.toggle("on", on.has(i));
      if (on.has(i) && !n.state) {
        const p = pad(n.kind === "ask", GREEN);
        p.position.set(n.x, 0.012, n.z);
        glow.add(p);
      }
    });
    for (let k = 0; k + 1 < lit.length; k++) {
      const to = nodes[lit[k]];
      const from = nodes[lit[k + 1]];
      const next = Math.abs(lit[k] - lit[k + 1]) === 1 && from.turn === to.turn;
      const way = band(next ? [[from.x, from.z], [to.x, to.z]] : aisle(from, to), 0.2, 0.02, GREEN);
      if (way) glow.add(way);
    }
    roads.forEach((r) => (r.goal = lit.length ? 0.55 : 1));
    wake();
  }

  function fit() {
    if (!nodes.length) return;
    const xs = nodes.map((n) => n.x);
    const zs = nodes.map((n) => n.z);
    view.toX = (Math.min(...xs) + Math.max(...xs)) / 2;
    view.toZ = (Math.min(...zs) + Math.max(...zs)) / 2;
    const wide = Math.max(...xs) - Math.min(...xs) + Math.max(...zs) - Math.min(...zs);
    view.span = Math.max(4.6, Math.min(wide * 0.42 + 3.2, 16));
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
    nodes = next.nodes.map((n, i) => {
      const b = block(n.kind);
      b.g.position.set(n.x, 0, n.z);
      b.g.traverse((o) => (o.userData.at = i));
      root.add(b.g);
      if (n.state) {
        const p = pad(false, PADS[n.state]);
        p.position.set(n.x, 0.01, n.z);
        root.add(p);
      }
      const name = document.createElement("span");
      name.textContent = n.label;
      if (n.kind === "ask") name.classList.add("ask");
      if (n.state) name.classList.add(n.state);
      labels.append(name);
      return { ...n, ...b, name, at: 1, goal: 1 };
    });
    // The path from each block to the next, a grey band with a white middle.
    roads = [];
    nodes.forEach((n, i) => {
      const to = nodes[i + 1];
      if (!to || to.turn !== n.turn) return;
      for (const [width, color, y] of [[0.3, ROAD, 0.004], [0.2, WHITE, 0.006]]) {
        const way = band([[n.x, n.z], [to.x, to.z]], width, y, color);
        if (way) {
          root.add(way);
          roads.push({ way, at: 1, goal: 1 });
        }
      }
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
    // On a map too long to see whole, the one picked is brought to the middle.
    if (i != null && nodes[i] && nodes.length > 14) {
      view.toX = nodes[i].x;
      view.toZ = nodes[i].z;
    }
  };
  api.zoomBy = (f) => {
    view.toZoom = THREE.MathUtils.clamp(view.toZoom * f, 0.45, 3.2);
    wake();
  };
  api.fit = fit;

  const ray = new THREE.Raycaster();
  const at = new THREE.Vector2();
  function under(e) {
    const box = canvas.getBoundingClientRect();
    at.set(((e.clientX - box.left) / box.width) * 2 - 1, -((e.clientY - box.top) / box.height) * 2 + 1);
    ray.setFromCamera(at, camera);
    const hit = ray.intersectObjects(nodes.map((n) => n.g), true).find((h) => h.object.isMesh && h.object.visible);
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
    canvas.style.cursor = i == null ? "" : "pointer";
    tip.hidden = i == null;
    if (i != null) {
      tip.replaceChildren();
      for (const [tag, text] of [["small", nodes[i].label], ["strong", nodes[i].tip], ["span", nodes[i].note]]) {
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
  function place(el, x, y, z, up) {
    p.set(x, y, z).project(camera);
    el.style.transform = `translate(-50%, ${up ? "-100%" : "0"}) translate(${((p.x + 1) / 2) * canvas.clientWidth}px, ${((1 - p.y) / 2) * canvas.clientHeight}px)`;
  }
  let last = performance.now();
  function frame(now) {
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
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
    sun.position.set(view.x - 16, 30, view.z + 6);
    sun.target.position.set(view.x, 0, view.z);
    const reach = (view.span * Math.max(1, aspect)) / view.zoom + 6;
    Object.assign(sun.shadow.camera, { left: -reach, right: reach, top: reach, bottom: -reach, near: 1, far: 90 });
    sun.shadow.camera.updateProjectionMatrix();
    nodes.forEach((n, i) => {
      if (Math.abs(n.goal - n.at) > 0.004) dirty = Math.max(dirty, 2);
      n.at += (n.goal - n.at) * ease;
      n.g.traverse((o) => o.material && o.material.transparent && (o.material.opacity = n.at));
      n.name.style.opacity = n.at < 0.7 ? 0.4 : 1;
      place(n.name, n.x + 0.55, 0, n.z + 0.95, false);
      if (i === hover) place(tip, n.x, n.tall + 0.35, n.z, true);
    });
    for (const r of roads) {
      r.at += (r.goal - r.at) * ease;
      r.way.material.opacity = r.at;
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
  };
  return api;
}

window.WHRoute = { mount };
