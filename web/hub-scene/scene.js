// The model on the security team's main page: people, their machines, the agents on them and
// the places outside those agents reached, stood on a floor as matte clay pieces under a soft
// studio light, joined by dashed lines that run the way the Calls went. It turns when dragged,
// lights what the piece under the pointer is joined to, and tells the page which piece was
// picked. The page gives it the pieces and where they stand (admin/hub.rs); this file only
// draws them. Built with `npm run build` into src/admin/hub_scene.js.
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";

const CLAY = 0xf4f2ec;
const FLOOR = 0xf7f5f0;
const LANE = 0xecebe4;
const GRAPHITE = 0x2c2d31;
const TONES = { plain: 0xb9b7ae, held: 0xf0b429, left: 0xe8622a };
const TIER_X = [-3.3, -1.1, 1.1, 3.3];
const still = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function clay(color = CLAY, roughness = 0.86) {
  return new THREE.MeshStandardMaterial({ color, roughness, metalness: 0, transparent: true });
}

function piece(geometry, material, x, y, z) {
  const m = new THREE.Mesh(geometry, material);
  m.position.set(x, y, z);
  m.castShadow = true;
  m.receiveShadow = true;
  return m;
}

// Each kind of piece, standing on the floor at its own origin. Returns the group and how far
// its front edge is from its middle, for the name beneath it, and its height, for its flag.
function figure(node) {
  const g = new THREE.Group();
  const dark = clay(GRAPHITE, 0.6);
  let front = 0.3;
  let tall = 0.6;
  if (node.tier === 0) {
    g.add(piece(new THREE.CapsuleGeometry(0.17, 0.3, 8, 20), clay(), 0, 0.32, 0));
    g.add(piece(new THREE.SphereGeometry(0.15, 28, 20), clay(), 0, 0.8, 0));
    front = 0.22;
    tall = 0.95;
  } else if (node.tier === 1) {
    g.add(piece(new RoundedBoxGeometry(0.92, 0.07, 0.62, 4, 0.03), clay(), 0, 0.035, 0.04));
    const lid = new THREE.Group();
    lid.add(piece(new RoundedBoxGeometry(0.92, 0.6, 0.05, 4, 0.025), clay(), 0, 0.3, 0));
    lid.add(piece(new RoundedBoxGeometry(0.8, 0.48, 0.012, 2, 0.006), dark, 0, 0.3, 0.03));
    lid.position.set(0, 0.06, -0.26);
    lid.rotation.x = -0.26;
    g.add(lid);
    front = 0.36;
    tall = 0.66;
  } else if (node.tier === 2) {
    g.add(piece(new RoundedBoxGeometry(0.56, 0.56, 0.56, 6, 0.13), node.shadow ? dark : clay(), 0, 0.28, 0));
    front = 0.3;
    tall = 0.56;
  } else if (node.label.endsWith("(MCP)")) {
    for (let k = 0; k < 3; k++) {
      g.add(piece(new THREE.CylinderGeometry(0.3, 0.3, 0.2, 40), clay(), 0, 0.1 + k * 0.26, 0));
    }
    front = 0.32;
    tall = 0.72;
  } else {
    g.add(piece(new RoundedBoxGeometry(0.5, 1.15, 0.5, 6, 0.1), clay(), 0, 0.575, 0));
    for (let k = 0; k < 3; k++) {
      g.add(piece(new RoundedBoxGeometry(0.3, 0.035, 0.012, 1, 0.005), dark, 0, 0.86 - k * 0.11, 0.252));
    }
    front = 0.27;
    tall = 1.15;
  }
  // A piece a value got out through stands on a coloured plinth; the rest only wear a flag.
  if (node.tone === "left") {
    const plinth = piece(new THREE.CylinderGeometry(0.5, 0.5, 0.03, 48), clay(TONES.left, 0.7), 0, 0.016, 0);
    plinth.castShadow = false;
    g.add(plinth);
  }
  return { g, front, tall };
}

// A round flag over a piece: an exclamation mark in the piece's tone, or a question mark on
// graphite for an agent running without the hooks.
function flag(node) {
  const c = document.createElement("canvas");
  c.width = c.height = 96;
  const x = c.getContext("2d");
  x.fillStyle = node.shadow ? "#2c2d31" : node.tone === "left" ? "#e8622a" : "#f0b429";
  x.beginPath();
  x.arc(48, 48, 40, 0, Math.PI * 2);
  x.fill();
  x.fillStyle = node.tone === "held" && !node.shadow ? "#3a2c00" : "#fff";
  x.font = "600 52px -apple-system, sans-serif";
  x.textAlign = "center";
  x.textBaseline = "middle";
  x.fillText(node.shadow ? "?" : "!", 48, 51);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  const s = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false }));
  s.scale.setScalar(0.34);
  return s;
}

function dashes() {
  const c = document.createElement("canvas");
  c.width = 64;
  c.height = 8;
  const x = c.getContext("2d");
  x.fillStyle = "#fff";
  x.beginPath();
  x.roundRect(6, 1, 34, 6, 3);
  x.fill();
  const tex = new THREE.CanvasTexture(c);
  tex.wrapS = THREE.RepeatWrapping;
  return tex;
}

function mount(container) {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.VSMShadowMap;
  renderer.toneMapping = THREE.NeutralToneMapping;
  renderer.toneMappingExposure = 1.02;
  const canvas = renderer.domElement;
  container.append(canvas);
  const labels = document.createElement("div");
  labels.className = "hub-labels";
  const tip = document.createElement("div");
  tip.className = "hub-tip";
  tip.hidden = true;
  container.append(labels, tip);

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.42;
  // The page's own ground shows through wherever the scene draws nothing.
  renderer.setClearColor(0x000000, 0);
  const camera = new THREE.PerspectiveCamera(24, 1, 0.1, 100);

  const key = new THREE.DirectionalLight(0xfff6ea, 2.5);
  key.position.set(-6.5, 10, 5.5);
  key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048);
  key.shadow.radius = 9;
  key.shadow.blurSamples = 16;
  key.shadow.bias = -0.0004;
  Object.assign(key.shadow.camera, { left: -8, right: 8, top: 8, bottom: -8, near: 1, far: 30 });
  const rim = new THREE.DirectionalLight(0xffffff, 0.5);
  rim.position.set(6, 5, -7);
  scene.add(key, rim);

  // The floor: a slab that floats over the page, a lane for each tier, and a soft shadow under it.
  const slab = piece(new RoundedBoxGeometry(9.5, 0.24, 8.9, 6, 0.12), clay(FLOOR, 0.92), 0, -0.12, 0);
  slab.castShadow = true;
  scene.add(slab);
  for (const x of TIER_X) {
    const lane = new THREE.Mesh(new RoundedBoxGeometry(1.5, 0.02, 7.9, 3, 0.01), clay(LANE, 0.95));
    lane.position.set(x, 0.002, 0);
    lane.receiveShadow = true;
    scene.add(lane);
  }
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(60, 60), new THREE.ShadowMaterial({ opacity: 0.16 }));
  ground.rotation.x = -Math.PI / 2;
  ground.position.y = -0.62;
  ground.receiveShadow = true;
  scene.add(ground);

  const root = new THREE.Group();
  scene.add(root);
  const dashTex = dashes();
  const ring = new THREE.Mesh(
    new THREE.RingGeometry(0.7, 0.76, 64),
    new THREE.MeshBasicMaterial({ color: 0x2c2d31, transparent: true, opacity: 0.85, depthWrite: false }),
  );
  ring.rotation.x = -Math.PI / 2;
  ring.position.y = 0.045;
  ring.visible = false;
  scene.add(ring);

  let data = { nodes: [], links: [], tiers: [] };
  let nodes = [];
  let links = [];
  let captions = [];
  let layer = null;
  let picked = null;
  let hover = null;
  const view = { az: 1.02, pol: 1.1, r: 29, toAz: 0.6, toPol: 1.02, toR: 25.5 };
  const api = { container, onPick: null };

  function chain(i) {
    const seen = new Set([i]);
    for (const forward of [true, false]) {
      const edge = [i];
      while (edge.length) {
        const n = edge.pop();
        for (const l of data.links) {
          const [a, b] = forward ? [l.from, l.to] : [l.to, l.from];
          if (a === n && !seen.has(b)) {
            seen.add(b);
            edge.push(b);
          }
        }
      }
    }
    return seen;
  }

  // What is lit: the chain of the piece under the pointer, else of the one picked, else the layer.
  function relight() {
    const focus = hover ?? picked;
    const lit = focus != null && nodes[focus] ? chain(focus) : layer ? new Set(layer) : null;
    nodes.forEach((n, i) => (n.goal = !lit || lit.has(i) ? 1 : 0.16));
    links.forEach((l) => (l.goal = !lit || (lit.has(l.from) && lit.has(l.to)) ? 1 : 0.1));
    ring.visible = picked != null && !!nodes[picked];
    if (ring.visible) ring.position.set(nodes[picked].x, 0.045, nodes[picked].z);
    wake();
  }

  function clear() {
    root.traverse((o) => {
      o.geometry?.dispose();
      if (o.material) {
        o.material.map?.dispose();
        o.material.dispose();
      }
    });
    root.clear();
    labels.replaceChildren();
    nodes = [];
    links = [];
    captions = [];
  }

  api.setData = (next) => {
    clear();
    data = next;
    hover = null;
    tip.hidden = true;
    next.nodes.forEach((n, i) => {
      const { g, front, tall } = figure(n);
      g.position.set(n.x, 0, n.z);
      g.traverse((o) => (o.userData.at = i));
      root.add(g);
      let mark = null;
      if (n.tone !== "plain" || n.shadow) {
        mark = flag(n);
        mark.position.set(n.x, tall + 0.36, n.z);
        root.add(mark);
      }
      const name = document.createElement("span");
      // An agent with the hooks is named by its machine beside it; one without is named itself.
      name.textContent = n.tier === 2 && !n.shadow ? "" : n.label;
      labels.append(name);
      nodes.push({ ...n, g, mark, name, front, tall, at: 1, goal: 1, phase: i * 0.9 });
    });
    next.tiers.forEach((t, k) => {
      const el = document.createElement("span");
      el.className = "tier";
      el.textContent = t;
      labels.append(el);
      captions.push({ el, x: TIER_X[k] });
    });
    for (const l of next.links) {
      const a = next.nodes[l.from];
      const b = next.nodes[l.to];
      const dx = b.x - a.x;
      const dz = b.z - a.z;
      const long = Math.hypot(dx, dz);
      const map = dashTex.clone();
      map.needsUpdate = true;
      map.repeat.set(long / 0.3, 1);
      const line = new THREE.Mesh(
        new THREE.PlaneGeometry(long, l.tone === "plain" ? 0.045 : 0.065),
        new THREE.MeshBasicMaterial({ map, color: TONES[l.tone], transparent: true, depthWrite: false }),
      );
      line.rotation.set(-Math.PI / 2, 0, 0);
      line.rotation.z = -Math.atan2(dz, dx);
      line.position.set((a.x + b.x) / 2, 0.03, (a.z + b.z) / 2);
      line.renderOrder = 1;
      root.add(line);
      const heads = [];
      if (l.tone !== "plain") {
        const shape = new THREE.Shape().moveTo(-0.08, -0.085).lineTo(0.09, 0).lineTo(-0.08, 0.085).closePath();
        for (let k = 0; k < 3; k++) {
          const head = new THREE.Mesh(
            new THREE.ShapeGeometry(shape),
            new THREE.MeshBasicMaterial({ color: TONES[l.tone], transparent: true, depthWrite: false }),
          );
          head.rotation.x = -Math.PI / 2;
          head.rotation.z = -Math.atan2(dz, dx);
          head.renderOrder = 2;
          root.add(head);
          heads.push(head);
        }
      }
      links.push({ ...l, line, map, heads, a, dx, dz, long, at: 1, goal: 1 });
    }
    relight();
  };
  api.setLit = (indices) => {
    layer = indices;
    relight();
  };
  api.setPicked = (i) => {
    picked = i;
    relight();
  };

  // Turning: a drag moves where the eye wants to be, and the eye follows.
  let drag = null;
  let moved = 0;
  const ray = new THREE.Raycaster();
  const at = new THREE.Vector2();
  function under(e) {
    const box = canvas.getBoundingClientRect();
    at.set(((e.clientX - box.left) / box.width) * 2 - 1, -((e.clientY - box.top) / box.height) * 2 + 1);
    ray.setFromCamera(at, camera);
    const hit = ray.intersectObjects(nodes.map((n) => n.g), true)[0];
    return hit ? hit.object.userData.at : null;
  }
  canvas.addEventListener("pointerdown", (e) => {
    drag = { x: e.clientX, y: e.clientY, az: view.toAz, pol: view.toPol };
    moved = 0;
    canvas.setPointerCapture(e.pointerId);
    container.classList.add("turning");
  });
  canvas.addEventListener("pointermove", (e) => {
    if (drag) {
      moved = Math.max(moved, Math.abs(e.clientX - drag.x) + Math.abs(e.clientY - drag.y));
      view.toAz = THREE.MathUtils.clamp(drag.az - (e.clientX - drag.x) * 0.006, -1.3, 1.3);
      view.toPol = THREE.MathUtils.clamp(drag.pol - (e.clientY - drag.y) * 0.004, 0.55, 1.3);
      tip.hidden = true;
      wake();
      return;
    }
    const i = under(e);
    if (i !== hover) {
      hover = i;
      canvas.style.cursor = i == null ? "" : "pointer";
      if (i == null) {
        tip.hidden = true;
      } else {
        const n = nodes[i];
        tip.replaceChildren();
        for (const [tag, text] of [["small", data.tiers[n.tier]], ["strong", n.label], ["span", n.tip]]) {
          const el = document.createElement(tag);
          el.textContent = text;
          tip.append(el);
        }
        tip.hidden = false;
      }
      relight();
    }
  });
  const release = (e) => {
    if (drag && moved < 5) {
      const i = under(e);
      if (i != null || picked != null) api.onPick?.(i === picked ? null : i);
    }
    drag = null;
    container.classList.remove("turning");
  };
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointerleave", () => {
    if (hover != null) {
      hover = null;
      tip.hidden = true;
      relight();
    }
  });
  canvas.addEventListener(
    "wheel",
    (e) => {
      if (!e.ctrlKey && Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;
      e.preventDefault();
      view.toR = THREE.MathUtils.clamp(view.toR + e.deltaY * 0.012, 16, 32);
      wake();
    },
    { passive: false },
  );

  const p = new THREE.Vector3();
  function place(el, x, y, z) {
    p.set(x, y, z).project(camera);
    el.style.transform = `translate(-50%, 0) translate(${((p.x + 1) / 2) * canvas.clientWidth}px, ${((1 - p.y) / 2) * canvas.clientHeight}px)`;
  }

  let last = performance.now();
  let live = true;
  let quiet = 0;
  function wake() {
    quiet = 0;
  }
  function frame(now) {
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    const calm = still();
    // Nothing moving and nothing flowing: leave the last frame up.
    if (calm && quiet > 40) return;
    quiet++;
    const ease = 1 - Math.exp(-dt * 7);
    view.az += (view.toAz - view.az) * ease;
    view.pol += (view.toPol - view.pol) * ease;
    view.r += (view.toR - view.r) * ease;
    camera.position.set(
      view.r * Math.sin(view.pol) * Math.sin(view.az),
      view.r * Math.cos(view.pol) - 0.5,
      view.r * Math.sin(view.pol) * Math.cos(view.az),
    );
    camera.lookAt(0.35, -0.45, 0);
    const t = now / 1000;
    nodes.forEach((n, i) => {
      n.at += (n.goal - n.at) * ease;
      n.g.traverse((o) => o.material && (o.material.opacity = n.at));
      if (n.mark) {
        n.mark.material.opacity = n.at;
        n.mark.position.y = n.tall + 0.36 + (calm ? 0 : Math.sin(t * 2.2 + n.phase) * 0.035);
      }
      n.name.style.opacity = n.at < 0.5 ? 0.25 : 1;
      place(n.name, n.x, 0, n.z + n.front + 0.16);
      if (i === hover) {
        p.set(n.x, n.tall + (n.mark ? 0.62 : 0.2), n.z).project(camera);
        tip.style.transform = `translate(-50%, -100%) translate(${((p.x + 1) / 2) * canvas.clientWidth}px, ${((1 - p.y) / 2) * canvas.clientHeight - 8}px)`;
      }
    });
    for (const c of captions) place(c.el, c.x, 0, 4.2);
    for (const l of links) {
      l.at += (l.goal - l.at) * ease;
      l.line.material.opacity = l.at * (l.tone === "plain" ? 0.75 : 1);
      if (!calm) l.map.offset.x -= dt * (l.tone === "plain" ? 0.5 : 1.3);
      l.heads.forEach((head, k) => {
        const along = calm ? 0.4 + k * 0.1 : (t * 0.22 + k * 0.085) % 1;
        const fade = Math.min(1, along * 6, (1 - along) * 6);
        head.position.set(l.a.x + l.dx * along, 0.034, l.a.z + l.dz * along);
        head.material.opacity = l.at * fade;
      });
    }
    renderer.render(scene, camera);
  }

  function resize() {
    const w = container.clientWidth;
    const h = container.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    wake();
  }
  const sized = new ResizeObserver(resize);
  sized.observe(container);
  resize();
  const run = () => renderer.setAnimationLoop(live && !document.hidden ? frame : null);
  const seen = new IntersectionObserver(([entry]) => {
    live = entry.isIntersecting;
    last = performance.now();
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
    clear();
    dashTex.dispose();
    pmrem.dispose();
    renderer.dispose();
    canvas.remove();
    labels.remove();
    tip.remove();
  };
  return api;
}

window.WHScene = { mount };
