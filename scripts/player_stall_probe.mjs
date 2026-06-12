// Open the player on the first channel, then dump full mpv state samples.
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

await ev(`(async () => {
  const until = async (p, ms = 15000) => {
    const s = Date.now();
    while (Date.now() - s < ms) { const v = p(); if (v) return v; await new Promise(r => setTimeout(r, 200)); }
    return null;
  };
  const card = await until(() => document.querySelector('[data-testid="channel-card"]'));
  card.click();
  return true;
})()`);

for (let i = 0; i < 4; i++) {
  await wait(2500);
  console.log(JSON.stringify(await ev("window.__proscenium.player.getState().mpv")));
}
ws.close();
