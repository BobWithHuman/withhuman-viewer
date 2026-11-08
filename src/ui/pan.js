// Moving and zooming the line map as a map app does, in the page itself so the map is not
// drawn again for every step of the hand: drag (with a glide after letting go), the wheel or
// two fingers to move, a pinch, Ctrl or Cmd with the wheel to zoom about the pointer.
// `fxMap.frame` brings a turn, and the Call picked in it, into view; `fxMap.zoomBy` zooms
// about the middle. A click that ends a drag does not reach the map.
window.fxMap = window.fxMap || (() => {
  const S = { x: 0, y: 0, k: 1, el: null, layer: null, raf: 0, pinch: false };
  const apply = (glide) => {
    if (!S.layer) return;
    S.layer.style.transition = glide ? 'transform .5s cubic-bezier(.22,.8,.24,1)' : 'none';
    S.layer.style.transform = `translate(${S.x}px, ${S.y}px) scale(${S.k})`;
  };
  const zoomAt = (f, cx, cy, glide) => {
    const k = Math.min(2.5, Math.max(0.05, S.k * f));
    const r = k / S.k;
    S.x = cx - (cx - S.x) * r;
    S.y = cy - (cy - S.y) * r;
    S.k = k;
    apply(glide);
  };
  const attach = () => {
    const el = document.getElementById('fx-line');
    const layer = document.getElementById('fx-layer');
    if (!el || !layer) return false;
    S.layer = layer;
    S.el = el;
    apply(false);
    if (el.__fx) return true;
    el.__fx = true;
    let down = null, moved = false, last = null, vx = 0, vy = 0;
    el.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      cancelAnimationFrame(S.raf);
      down = { x: e.clientX, y: e.clientY, sx: S.x, sy: S.y };
      moved = false;
      last = { x: e.clientX, y: e.clientY, t: performance.now() };
      vx = vy = 0;
    });
    window.addEventListener('pointermove', (e) => {
      if (!down) return;
      const dx = e.clientX - down.x, dy = e.clientY - down.y;
      if (!moved && Math.hypot(dx, dy) < 4) return;
      if (!moved) { moved = true; el.classList.add('dragging'); }
      S.x = down.sx + dx;
      S.y = down.sy + dy;
      const t = performance.now(), dt = Math.max(1, t - last.t);
      vx = (e.clientX - last.x) / dt;
      vy = (e.clientY - last.y) / dt;
      last = { x: e.clientX, y: e.clientY, t };
      apply(false);
    });
    window.addEventListener('pointerup', () => {
      if (!down) return;
      const was = moved;
      down = null;
      el.classList.remove('dragging');
      if (!was) return;
      el.__drag = performance.now();
      // A glide after letting go, slowing to a stop.
      let gx = vx * 16, gy = vy * 16;
      const step = () => {
        gx *= 0.92; gy *= 0.92;
        if (Math.hypot(gx, gy) < 0.3) return;
        S.x += gx; S.y += gy;
        apply(false);
        S.raf = requestAnimationFrame(step);
      };
      S.raf = requestAnimationFrame(step);
    });
    el.addEventListener('click', (e) => {
      if (el.__drag && performance.now() - el.__drag < 250) { e.stopPropagation(); e.preventDefault(); }
    }, true);
    el.addEventListener('wheel', (e) => {
      e.preventDefault();
      cancelAnimationFrame(S.raf);
      const r = el.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) {
        if (!S.pinch) zoomAt(Math.exp(-e.deltaY * 0.01), e.clientX - r.left, e.clientY - r.top, false);
        return;
      }
      let dx = e.deltaX, dy = e.deltaY;
      if (e.shiftKey && !dx) { dx = dy; dy = 0; }
      S.x -= dx; S.y -= dy;
      apply(false);
    }, { passive: false });
    // A trackpad pinch, as WebKit reports it.
    let g = 1;
    el.addEventListener('gesturestart', (e) => { e.preventDefault(); S.pinch = true; g = 1; });
    el.addEventListener('gesturechange', (e) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      zoomAt(e.scale / g, e.clientX - r.left, e.clientY - r.top, false);
      g = e.scale;
    });
    el.addEventListener('gestureend', (e) => { e.preventDefault(); S.pinch = false; });
    return true;
  };
  return {
    attach,
    zoomBy: (f) => {
      if (!attach()) return;
      const r = S.el.getBoundingClientRect();
      zoomAt(f, r.width / 2, r.height / 2, true);
    },
    // The whole map in view, centred.
    fit: () => {
      if (!attach()) return;
      const svg = S.layer.querySelector('svg');
      const w = +svg.getAttribute('width'), h = +svg.getAttribute('height');
      const r = S.el.getBoundingClientRect();
      const k = Math.min(2.5, Math.max(0.05, Math.min(r.width / w, (r.height - 70) / h) * 0.94));
      S.k = k;
      S.x = (r.width - w * k) / 2;
      S.y = (r.height - 70 - h * k) / 2;
      apply(true);
    },
    // The box (x0, y0)-(x1, y1) in view, centred, no closer than full size: the way back from a
    // Call to what told it, however far apart.
    box: (x0, y0, x1, y1) => {
      if (!attach()) return;
      const r = S.el.getBoundingClientRect();
      const w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
      const k = Math.min(1, Math.max(0.05, Math.min((r.width - 120) / w, (r.height - 190) / h)));
      S.k = k;
      S.x = r.width / 2 - (x0 + w / 2) * k;
      S.y = (r.height - 70) / 2 - (y0 + h / 2) * k;
      apply(true);
    },
    // (ax, ay): the turn's zone corner, to the top left; (tx, ty): the Call picked, near the
    // middle when it would lie outside the view.
    // A turn on a row running left (rev) is put at the view's right, its zone's right edge
    // (ax + aw) there, so the way it goes, leftwards, is in view; either way some of what came
    // before shows above it.
    frame: (ax, ay, tx, ty, rev, aw) => {
      if (!attach()) return;
      const r = S.el.getBoundingClientRect();
      let x = rev ? r.width * 0.92 - (ax + (aw || 0)) * S.k : r.width * 0.08 - ax * S.k;
      let y = r.height * 0.22 - ay * S.k;
      if (tx !== null) {
        const px = tx * S.k + x, py = ty * S.k + y;
        if (px > r.width - 160 || px < 40) x = r.width * 0.5 - tx * S.k;
        if (py > r.height - 160 || py < 40) y = r.height * 0.45 - ty * S.k;
      }
      S.x = x; S.y = y;
      apply(true);
    },
  };
})();
