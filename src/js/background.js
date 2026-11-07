// The Vesper banner, alive: twinkling stars, a breathing moon, shooting stars,
// blocky hills with mouse parallax, and fireflies that scatter away from the cursor.
const DW = 1500, DH = 900;
const rnd = (a, b) => a + Math.random() * (b - a);

export function startBackground(canvas) {
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let W = 0, H = 0, dpr = 1, scale = 1, ox = 0, oy = 0;
  const mouse = { x: 0, y: 0, tx: 0, ty: 0, px: -999, py: -999 };

  const stars = Array.from({ length: 170 }, () => ({ x: rnd(0, DW), y: rnd(0, 620), r: rnd(0.6, 2.2), p: rnd(0, 6.28), s: rnd(0.6, 2.4), a: rnd(0.35, 1) }));
  const sparks = [[1215, 215, 34], [330, 170, 20], [690, 95, 14], [120, 330, 12], [1380, 120, 16]].map(([x, y, s]) => ({ x, y, s, p: rnd(0, 6.28) }));

  // Deterministic blocky terrain, like the banner.
  function layer(base, amp, step, seed, color, par) {
    let h = base, s = seed;
    const rand = () => ((s = (s * 16807) % 2147483647) / 2147483647);
    const cols = [];
    for (let x = -80; x < DW + 80; x += step * 2) { h += Math.floor(rand() * 3 - 1) * step; h = Math.max(base - amp, Math.min(base + amp, h)); cols.push([x, h]); }
    return { cols, step, color, par };
  }
  const layers = [layer(690, 70, 30, 3, '#2c2370', 6), layer(755, 60, 30, 4, '#1f1855', 12), layer(820, 45, 30, 5, '#120f33', 22)];
  const groundAt = (L, x) => { let g = L.cols[0][1]; for (const [cx, h] of L.cols) if (cx <= x) g = h; return g; };

  const flies = Array.from({ length: 28 }, () => ({ x: rnd(0, DW), y: rnd(560, 830), vx: rnd(-8, 8), vy: rnd(-6, 6), p: rnd(0, 6.28), s: rnd(0.8, 2) }));
  const glow = document.createElement('canvas'); glow.width = glow.height = 48;
  { const g = glow.getContext('2d'), r = g.createRadialGradient(24, 24, 0, 24, 24, 24); r.addColorStop(0, 'rgba(220,210,255,1)'); r.addColorStop(0.25, 'rgba(170,150,255,.55)'); r.addColorStop(1, 'rgba(143,124,255,0)'); g.fillStyle = r; g.fillRect(0, 0, 48, 48); }

  let shoot = null, nextShoot = rnd(2, 5);

  function resize() {
    dpr = Math.min(devicePixelRatio || 1, 1.5);
    W = innerWidth; H = innerHeight;
    canvas.width = W * dpr; canvas.height = H * dpr;
    canvas.style.width = W + 'px'; canvas.style.height = H + 'px';
    scale = Math.max(W / DW, H / DH);
    ox = (W - DW * scale) / 2; oy = (H - DH * scale) / 2;
  }
  addEventListener('resize', resize); resize();
  addEventListener('pointermove', (e) => { mouse.tx = (e.clientX / W) * 2 - 1; mouse.ty = (e.clientY / H) * 2 - 1; mouse.px = e.clientX; mouse.py = e.clientY; });

  function spark(x, y, s, a) {
    ctx.globalAlpha = a; ctx.fillStyle = '#fff'; ctx.beginPath();
    ctx.moveTo(x, y - s); ctx.lineTo(x + s * 0.26, y - s * 0.26); ctx.lineTo(x + s, y); ctx.lineTo(x + s * 0.26, y + s * 0.26);
    ctx.lineTo(x, y + s); ctx.lineTo(x - s * 0.26, y + s * 0.26); ctx.lineTo(x - s, y); ctx.lineTo(x - s * 0.26, y - s * 0.26); ctx.fill();
  }

  let last = 0;
  function frame(ts) {
    const t = ts / 1000, dt = Math.min(0.05, t - last || 0.016); last = t;
    mouse.x += (mouse.tx - mouse.x) * 0.05; mouse.y += (mouse.ty - mouse.y) * 0.05;
    const mx = mouse.x + Math.sin(t * 0.18) * 0.25, my = mouse.y + Math.cos(t * 0.15) * 0.12;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const sky = ctx.createLinearGradient(0, 0, 0, H);
    sky.addColorStop(0, '#0b0b17'); sky.addColorStop(0.55, '#1b1745'); sky.addColorStop(1, '#3a2d86');
    ctx.fillStyle = sky; ctx.fillRect(0, 0, W, H);

    ctx.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * ox, dpr * oy);

    // moon glow + moon
    const mxp = 1070 - mx * 5, myp = 330 + Math.sin(t * 0.6) * 5 - my * 3;
    const gl = ctx.createRadialGradient(mxp, myp, 0, mxp, myp, 560);
    gl.addColorStop(0, `rgba(154,139,255,${0.38 + Math.sin(t * 0.7) * 0.07})`); gl.addColorStop(1, 'rgba(154,139,255,0)');
    ctx.globalAlpha = 1; ctx.fillStyle = gl; ctx.fillRect(-200, -200, DW + 400, DH + 400);

    // stars
    for (const s of stars) {
      ctx.globalAlpha = s.a * (0.55 + 0.45 * Math.sin(t * s.s + s.p));
      ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(s.x - mx * 3, s.y - my * 2, s.r, 0, 6.283); ctx.fill();
    }
    for (const k of sparks) spark(k.x - mx * 4, k.y - my * 3, k.s * (0.85 + 0.15 * Math.sin(t * 1.6 + k.p)), 0.55 + 0.45 * Math.sin(t * 1.6 + k.p));

    // shooting star
    nextShoot -= dt;
    if (!shoot && nextShoot <= 0 && !reduce) { shoot = { x: rnd(200, 1100), y: rnd(30, 220), vx: rnd(520, 760), vy: rnd(220, 340), life: 0 }; nextShoot = rnd(6, 13); }
    if (shoot) {
      shoot.life += dt; shoot.x += shoot.vx * dt; shoot.y += shoot.vy * dt;
      const a = Math.max(0, 1 - shoot.life / 1.1), len = 0.16;
      const g = ctx.createLinearGradient(shoot.x, shoot.y, shoot.x - shoot.vx * len, shoot.y - shoot.vy * len);
      g.addColorStop(0, `rgba(255,255,255,${a})`); g.addColorStop(1, 'rgba(255,255,255,0)');
      ctx.globalAlpha = 1; ctx.strokeStyle = g; ctx.lineWidth = 2.5; ctx.lineCap = 'round';
      ctx.beginPath(); ctx.moveTo(shoot.x, shoot.y); ctx.lineTo(shoot.x - shoot.vx * len, shoot.y - shoot.vy * len); ctx.stroke();
      if (shoot.life > 1.1) shoot = null;
    }

    // crescent moon (clip out the second circle)
    ctx.save(); ctx.globalAlpha = 1; ctx.beginPath();
    ctx.rect(-400, -400, DW + 800, DH + 800); ctx.arc(mxp + 100, myp - 50, 165, 0, 6.283); ctx.clip('evenodd');
    const mg = ctx.createLinearGradient(mxp - 200, myp - 200, mxp + 200, myp + 200);
    mg.addColorStop(0, '#d8d1ff'); mg.addColorStop(1, '#8f7cff');
    ctx.fillStyle = mg; ctx.beginPath(); ctx.arc(mxp - 20, myp + 10, 215, 0, 6.283); ctx.fill(); ctx.restore();

    // terrain, back to front, with parallax; trees stand on the middle layer
    layers.forEach((L, i) => {
      const dx = -mx * L.par, dy = -my * L.par * 0.5;
      ctx.globalAlpha = 1; ctx.fillStyle = L.color;
      for (const [x, h] of L.cols) ctx.fillRect(x + dx, h + dy, L.step * 2 + 1, DH + 400 - h);
      if (i === 1) {
        ctx.fillStyle = '#120f33';
        for (const tx of [150, 470, 900, 1300]) {
          const g = groundAt(L, tx) + 20 + dy, x = tx + dx, sway = Math.sin(t * 0.8 + tx) * 1.2;
          ctx.fillRect(x, g - 110, 12, 110); ctx.fillRect(x - 24 + sway, g - 170, 60, 32); ctx.fillRect(x - 12 + sway, g - 202, 36, 32); ctx.fillRect(x - 24 + sway, g - 138, 60, 30);
        }
      }
      if (i === 1) {
        // fireflies float between the middle and front hills and flee from the cursor
        const cx = (mouse.px - ox) / scale, cy = (mouse.py - oy) / scale;
        for (const f of flies) {
          f.p += dt * f.s;
          f.vx += Math.cos(f.p) * 14 * dt; f.vy += Math.sin(f.p * 1.3) * 10 * dt;
          const ddx = f.x - cx, ddy = f.y - cy, d2 = ddx * ddx + ddy * ddy;
          if (d2 < 150 * 150) { const d = Math.sqrt(d2) || 1, push = (1 - d / 150) * 420 * dt; f.vx += (ddx / d) * push; f.vy += (ddy / d) * push; }
          f.vx *= 0.985; f.vy *= 0.985; f.x += f.vx * dt; f.y += f.vy * dt;
          if (f.x < -20) f.x = DW + 20; if (f.x > DW + 20) f.x = -20; f.y = Math.max(520, Math.min(840, f.y));
          ctx.globalAlpha = 0.5 + 0.5 * Math.sin(t * 2 + f.p * 3);
          ctx.drawImage(glow, f.x + dx - 12, f.y + dy - 12, 24, 24);
        }
      }
    });
    ctx.globalAlpha = 1;
    if (!reduce) requestAnimationFrame(frame);
  }
  if (reduce) frame(1000); else requestAnimationFrame(frame);
  document.addEventListener('visibilitychange', () => { if (!document.hidden && !reduce) { last = 0; requestAnimationFrame(frame); } });
}
