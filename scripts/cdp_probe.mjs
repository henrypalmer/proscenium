// Quick CDP probe: list targets, dump page URL/body and console errors.
const DEBUG_URL = "http://127.0.0.1:9222";
const targets = await (await fetch(`${DEBUG_URL}/json`)).json();
console.log(targets.map((t) => ({ type: t.type, url: t.url, title: t.title })));
const page = targets.find((t) => t.type === "page" && !t.url.startsWith("devtools"));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let id = 1;
const pending = new Map();
const events = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  else if (d.method === "Log.entryAdded" || d.method === "Runtime.consoleAPICalled" || d.method === "Runtime.exceptionThrown") events.push(JSON.stringify(d.params).slice(0, 400));
};
const send = (method, params = {}) => new Promise((res) => { const i = id++; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
await send("Runtime.enable");
await send("Log.enable");
const out = await send("Runtime.evaluate", { expression: `JSON.stringify({ url: location.href, root: document.getElementById('root')?.childElementCount, bodyText: document.body.innerText.slice(0, 300), hasStores: typeof window.__proscenium })`, returnByValue: true });
console.log(out.result?.result?.value);
await new Promise((r) => setTimeout(r, 1500));
console.log("console/log events:", events.slice(0, 10));
ws.close();
