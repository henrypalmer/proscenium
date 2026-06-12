// Opens the player, waits for playback, then triggers the context-menu
// "Open in External Player" path. The window screenshot is taken from
// PowerShell afterwards.
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

// External player first (player not open): right-click first channel → second item.
const external = await ev(`(async () => {
  const until = async (p, ms = 15000) => {
    const s = Date.now();
    while (Date.now() - s < ms) { const v = p(); if (v) return v; await new Promise(r => setTimeout(r, 200)); }
    return null;
  };
  const card = await until(() => document.querySelector('[data-testid="channel-card"]'));
  const rect = card.getBoundingClientRect();
  card.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rect.left + 100, clientY: rect.top + 20 }));
  await new Promise(r => setTimeout(r, 300));
  const items = [...document.querySelectorAll('[data-testid="context-menu"] [role="menuitem"]')];
  const labels = items.map(b => b.textContent);
  items.find(b => b.textContent === 'Open in External Player')?.click();
  return labels;
})()`);
console.log("context menu:", JSON.stringify(external));
await wait(2500);

// Now open the built-in player and let it play.
await ev(`document.querySelector('[data-testid="channel-card"]').click()`);
await ev(`(async () => {
  const until = async (p, ms = 20000) => {
    const s = Date.now();
    while (Date.now() - s < ms) { const v = p(); if (v) return v; await new Promise(r => setTimeout(r, 200)); }
    return null;
  };
  await until(() => {
    const s = window.__proscenium.player.getState().mpv;
    return s && s.playing && s.position > 1;
  });
  return true;
})()`);
console.log("player playing; ready for screenshot");
ws.close();
