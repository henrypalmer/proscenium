// Capture the WebView's framebuffer (with alpha) during playback and report
// pixel alpha at the center (video area) vs. inside the controls bar.
import { writeFileSync } from "node:fs";
import zlib from "node:zlib";

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const targets = await (await fetch("http://127.0.0.1:9222/json")).json();
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let id = 1;
const pending = new Map();
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
};
const send = (method, params = {}) =>
  new Promise((res) => { const i = id++; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (e) =>
  (await send("Runtime.evaluate", { expression: e, awaitPromise: true, returnByValue: true })).result?.result?.value;

// Open the player and wait for playback.
await ev(`(async () => {
  const until = async (p, ms = 15000) => {
    const s = Date.now();
    while (Date.now() - s < ms) { const v = p(); if (v) return v; await new Promise(r => setTimeout(r, 200)); }
    return null;
  };
  (await until(() => document.querySelector('[data-testid="channel-card"]'))).click();
  await until(() => {
    const s = window.__proscenium.player.getState().mpv;
    return s && s.playing && s.position > 1;
  });
  // Poke the controls so the bottom bar is visible in the capture.
  window.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));
  return true;
})()`);

// Transparent default background so alpha survives the capture.
await send("Emulation.setDefaultBackgroundColorOverride", {
  color: { r: 0, g: 0, b: 0, a: 0 },
});
await wait(300);
const shot = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
const png = Buffer.from(shot.result.data, "base64");
writeFileSync(process.env.TEMP + "\\webview-alpha.png", png);

// Minimal PNG decode to sample alpha: use zlib + parse IHDR/IDAT.
function samplePixels(buf, points) {
  // PNG: 8-byte sig, then chunks. Collect IDAT, read IHDR for dims.
  let off = 8;
  let width = 0, height = 0, idat = [];
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") { width = data.readUInt32BE(0); height = data.readUInt32BE(4); }
    if (type === "IDAT") idat.push(data);
    off += 12 + len;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * 4 + 1;
  // Un-filter just the rows we need (filter types complicate random access;
  // do a full sequential unfilter).
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(width * 4);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * stride];
    const row = raw.subarray(y * stride + 1, (y + 1) * stride);
    const cur = Buffer.alloc(width * 4);
    for (let x = 0; x < width * 4; x++) {
      const a = x >= 4 ? cur[x - 4] : 0;
      const b = prev[x];
      const c = x >= 4 ? prev[x - 4] : 0;
      let v = row[x];
      if (filter === 1) v = (v + a) & 0xff;
      else if (filter === 2) v = (v + b) & 0xff;
      else if (filter === 3) v = (v + ((a + b) >> 1)) & 0xff;
      else if (filter === 4) {
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v = (v + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 0xff;
      }
      cur[x] = v;
    }
    cur.copy(out, y * width * 4);
    prev = cur;
  }
  return points.map(([px, py]) => {
    const x = Math.floor(px * width), y = Math.floor(py * height);
    const i = (y * width + x) * 4;
    return { at: [px, py], r: out[i], g: out[i + 1], b: out[i + 2], a: out[i + 3] };
  });
}

const samples = samplePixels(png, [
  [0.5, 0.4],   // video area (should be transparent)
  [0.25, 0.45], // video area
  [0.5, 0.97],  // controls bar (should be opaque-ish)
]);
console.log(JSON.stringify({ samples }, null, 2));
ws.close();
