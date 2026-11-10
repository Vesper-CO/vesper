// The daytime scene used by Light mode: a warm sun with slowly turning rays, drifting blocky clouds,
// birds, butterflies that flee from the cursor, and the same blocky hills as the night scene
// (so switching themes feels like the sun coming up over the same world).
//
// All the artwork lives in separate files in src/assets/day/:
//   sky.svg  sun.svg  sun-rays.svg  cloud-1..3.svg  hills-back/mid/front.svg  tree.svg
// Each one is rasterised once at the current window size ("baked") so the per-frame cost is a few
// plain drawImage calls, not SVG rendering.
const DW = 1500, DH = 900;
const rnd = (a, b) => a + Math.random() * (b - a);
const NAMES = ['sky', 'sun', 'sun-rays', 'cloud-1', 'cloud-2', 'cloud-3', 'hills-back', 'hills-mid', 'hills-front', 'tree'];
// Hill images are cropped to save memory: [left x, top y, width, height] in the 1500x900 design space.
const HILLS = [['hills-back', -80, 610, 1730, 390, 6], ['hills-mid', -80, 685, 1730, 315, 12], ['hills-front', -80, 765, 1730, 235, 22]];
const CLOUDS = [['cloud-1', 420, 120], ['cloud-2', 300, 100], ['cloud-3', 520, 140]];
const WING = ['#ff9ec4', '#ffd166', '#8fd3ff', '#ffffff', '#c3a6ff', '#ff8f5e'];

export function createDay({ layers, groundAt, onReady }) {
  const ready = {}, baked = {};
  let loading = false;

  function load() {
    if (loading) return; loading = true;
    for (const n of NAMES) {
      const im = new Image(); im.decoding = 'async';
      im.onload = () => { ready[n] = im; onReady?.(); };
      im.onerror = () => console.warn('missing day asset', n);
      im.src = `assets/day/${n}.svg`;
    }
  }

  // Rasterise an asset to w x h design units at k device pixels per unit; redo only when the size changes.
  function sprite(name, w, h, k) {
    const im = ready[name]; if (!im) return null;
    let b = baked[name];
    if (!b || b.k !== k || b.w !== w || b.h !== h) {
      const c = document.createElement('canvas');
      c.width = Math.max(1, Math.ceil(w * k)); c.height = Math.max(1, Math.ceil(h * k));
      c.getContext('2d').drawImage(im, 0, 0, c.width, c.height);
      b = baked[name] = { c, k, w, h };
    }
    return b.c;
  }

  const clouds = Array.from({ length: 7 }, (_, i) => {
    const [name, w, h] = CLOUDS[i % 3], s = rnd(0.7, 1.25);
    return { name, w: w * s, h: h * s, x: (i / 7) * (DW + 700) - 350 + rnd(-60, 60), y: rnd(70, 400), v: rnd(5, 14), a: rnd(0.8, 1), par: rnd(5, 14) };
  });
  const motes = Array.from({ length: 26 }, () => ({ x: rnd(0, DW), y: rnd(120, 800), r: rnd(1.2, 2.8), p: rnd(0, 6.28), s: rnd(0.4, 1.2), v: rnd(4, 12) }));
  const flies = Array.from({ length: 16 }, (_, i) => ({ x: rnd(0, DW), y: rnd(560, 830), vx: rnd(-10, 10), vy: rnd(-8, 8), p: rnd(0, 6.28), s: rnd(0.8, 2), c: WING[i % WING.length] }));
  let flock = null, nextFlock = rnd(3, 7);

  function bird(ctx, x, y, flap) {
    const w = 11, up = Math.sin(flap) * 7;
    ctx.beginPath(); ctx.moveTo(x - w, y - up); ctx.quadraticCurveTo(x - 4, y - up * 0.2 - 3, x, y); ctx.quadraticCurveTo(x + 4, y - up * 0.2 - 3, x + w, y - up); ctx.stroke();
  }

  // e: { t, dt, W, H, dpr, scale, ox, oy, k, mx, my, cx, cy, alpha, rise, reduce }
  function draw(ctx, e) {
    load();
    const { t, dt, W, H, dpr, scale, ox, oy, k, mx, my, cx, cy, alpha, rise } = e;
    const A = (a = 1) => (ctx.globalAlpha = a * alpha);

    // sky (screen space)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const sky = sprite('sky', W, H, dpr);
    A();
    if (sky) ctx.drawImage(sky, 0, 0, W, H);
    else { const g = ctx.createLinearGradient(0, 0, 0, H); g.addColorStop(0, '#4fa9f5'); g.addColorStop(0.7, '#bfe6ff'); g.addColorStop(1, '#fff1c9'); ctx.fillStyle = g; ctx.fillRect(0, 0, W, H); }

    ctx.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * ox, dpr * oy);

    // sun: warm glow, turning rays, disc. It rises from below the horizon while the theme switches.
    const sx = 1070 - mx * 5, sy = 330 + Math.sin(t * 0.6) * 5 - my * 3 + rise; // rise is 0 now (kept so the sun could be animated again)
    const gl = ctx.createRadialGradient(sx, sy, 0, sx, sy, 760);
    gl.addColorStop(0, `rgba(255,246,196,${0.55 + Math.sin(t * 0.7) * 0.05})`); gl.addColorStop(0.35, 'rgba(255,236,160,.2)'); gl.addColorStop(1, 'rgba(255,236,160,0)');
    A(); ctx.fillStyle = gl; ctx.fillRect(-200, -200, DW + 400, DH + 400);

    const RAY = 900, SUN = 470;
    const rays = sprite('sun-rays', RAY, RAY, k);
    if (rays) {
      ctx.save(); ctx.translate(sx, sy); ctx.rotate(t * 0.035); A(0.9 + Math.sin(t * 0.9) * 0.1);
      ctx.drawImage(rays, -RAY / 2, -RAY / 2, RAY, RAY); ctx.restore();
    }
    const disc = sprite('sun', SUN, SUN, k);
    if (disc) { const pulse = 1 + Math.sin(t * 0.8) * 0.012; A(); ctx.drawImage(disc, sx - (SUN * pulse) / 2, sy - (SUN * pulse) / 2, SUN * pulse, SUN * pulse); }

    // sunlit dust floating up
    ctx.fillStyle = '#fffbe0';
    for (const m of motes) {
      m.y -= m.v * dt; if (m.y < 60) { m.y = 820; m.x = rnd(0, DW); }
      A(0.25 + 0.35 * Math.sin(t * m.s + m.p) ** 2);
      ctx.beginPath(); ctx.arc(m.x - mx * 6 + Math.sin(t * 0.4 + m.p) * 14, m.y - my * 4, m.r, 0, 6.283); ctx.fill();
    }

    // clouds drift right and wrap around
    for (const c of clouds) {
      c.x += c.v * dt; if (c.x > DW + 120) { c.x = -c.w - 80; c.y = rnd(70, 400); }
      const base = CLOUDS.find((x) => x[0] === c.name); const sp = sprite(c.name, base[1] * 1.25, base[2] * 1.25, k);
      if (sp) { A(c.a); ctx.drawImage(sp, c.x - mx * c.par, c.y - my * c.par * 0.5, c.w, c.h); }
    }

    // a little flock of birds crosses the sky now and then
    nextFlock -= dt;
    if (!flock && nextFlock <= 0 && !e.reduce) { flock = { x: -80, y: rnd(120, 300), v: rnd(70, 110), n: 3 + Math.floor(Math.random() * 3), p: rnd(0, 6) }; nextFlock = rnd(9, 18); }
    if (flock) {
      flock.x += flock.v * dt;
      ctx.strokeStyle = '#33465f'; ctx.lineWidth = 2.2; ctx.lineCap = 'round'; ctx.lineJoin = 'round'; A(0.75);
      for (let i = 0; i < flock.n; i++) { const o = Math.ceil(i / 2) * (i % 2 ? 1 : -1); bird(ctx, flock.x - Math.abs(o) * 34, flock.y + o * 22 + Math.sin(t + i) * 3, t * 7 + flock.p + i * 0.7); }
      if (flock.x > DW + 160) flock = null;
    }

    // hills back to front; oaks and butterflies live on the middle layer
    HILLS.forEach(([name, x, y, w, h, par], i) => {
      const dx = -mx * par, dy = -my * par * 0.5;
      const sp = sprite(name, w, h, k);
      if (sp) { A(); ctx.drawImage(sp, x + dx, y + dy, w, h); }
      if (i !== 1) return;
      const tree = sprite('tree', 120, 210, k);
      if (tree) {
        for (const tx of [150, 470, 900, 1300]) {
          const g = groundAt(layers[1], tx) + 24 + dy;
          ctx.save(); ctx.translate(tx + dx + 6, g); ctx.rotate(Math.sin(t * 0.8 + tx) * 0.012); A();
          ctx.drawImage(tree, -60, -210, 120, 210); ctx.restore();
        }
      }
      for (const f of flies) {
        f.p += dt * f.s;
        f.vx += Math.cos(f.p) * 16 * dt; f.vy += Math.sin(f.p * 1.3) * 12 * dt;
        const ddx = f.x - cx, ddy = f.y - cy, d2 = ddx * ddx + ddy * ddy;
        if (d2 < 150 * 150) { const d = Math.sqrt(d2) || 1, push = (1 - d / 150) * 420 * dt; f.vx += (ddx / d) * push; f.vy += (ddy / d) * push; }
        f.vx *= 0.985; f.vy *= 0.985; f.x += f.vx * dt; f.y += f.vy * dt;
        if (f.x < -20) f.x = DW + 20; if (f.x > DW + 20) f.x = -20; f.y = Math.max(520, Math.min(840, f.y));
        const flap = Math.abs(Math.cos(t * 9 + f.p * 4)), bx = f.x + dx, by = f.y + dy;
        A(); ctx.fillStyle = f.c;
        ctx.beginPath(); ctx.ellipse(bx - 4 * flap, by - 2, 5 * flap + 0.5, 6, -0.3, 0, 6.283); ctx.ellipse(bx + 4 * flap, by - 2, 5 * flap + 0.5, 6, 0.3, 0, 6.283); ctx.fill();
        ctx.fillStyle = '#2b3a55'; ctx.fillRect(bx - 1, by - 5, 2, 9);
      }
    });
    ctx.globalAlpha = 1;
  }

  // Bake every sprite ahead of time so switching to Light doesn't have to do it on the first frame.
  function warm({ W, H, dpr, k }) {
    sprite('sky', W, H, dpr); sprite('sun-rays', 900, 900, k); sprite('sun', 470, 470, k); sprite('tree', 120, 210, k);
    for (const [n, , , w, h] of HILLS) sprite(n, w, h, k);
    for (const [n, w, h] of CLOUDS) sprite(n, w * 1.25, h * 1.25, k);
  }

  return { draw, load, warm };
}
