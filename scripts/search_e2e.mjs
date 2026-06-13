// Milestone 6 e2e: drives the real Proscenium app through WebView2 remote
// debugging (CDP) and verifies the global search acceptance criteria —
// Ctrl+F from multiple sections, sub-300ms results, grouping, type filter,
// "Show all" expander, no-results state, result navigation/playback, and
// network silence while searching.
// Usage: node search_e2e.mjs   (app must be running with
//        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222)

const DEBUG_URL = "http://127.0.0.1:9222";
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  for (let i = 0; i < 40; i++) {
    try {
      const targets = await (await fetch(`${DEBUG_URL}/json`)).json();
      const page = targets.find((t) => t.type === "page" && !t.url.startsWith("devtools"));
      if (page) return new WebSocket(page.webSocketDebuggerUrl);
    } catch {}
    await wait(500);
  }
  throw new Error("could not reach the WebView2 debugger");
}

const ws = await connect();
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});

let nextId = 1;
const pending = new Map();
const networkRequests = [];
let captureNetwork = false;
ws.onmessage = (msg) => {
  const data = JSON.parse(msg.data);
  if (data.id && pending.has(data.id)) {
    pending.get(data.id)(data);
    pending.delete(data.id);
  } else if (captureNetwork && data.method === "Network.requestWillBeSent") {
    networkRequests.push({
      url: data.params.request.url,
      type: data.params.type,
    });
  }
};
function send(method, params = {}) {
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const res = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (res.result?.exceptionDetails) {
    throw new Error(JSON.stringify(res.result.exceptionDetails));
  }
  return res.result?.result?.value;
}

const results = [];
function report(name, ok, detail = "") {
  results.push({ name, ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? ` — ${detail}` : ""}`);
}

// Shared in-page helpers.
await evaluate(`
  window.__e2e = {
    openSearch() {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", ctrlKey: true, bubbles: true }));
    },
    pressEscape() {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    },
    type(text) {
      const input = document.querySelector('[data-testid="search-input"]');
      const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
      setter.call(input, text);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
    // Resolves with ms from now until a node matching selector exists.
    waitFor(selector, timeoutMs = 5000) {
      return new Promise((resolve) => {
        const t0 = performance.now();
        if (document.querySelector(selector)) return resolve(0);
        const obs = new MutationObserver(() => {
          if (document.querySelector(selector)) {
            obs.disconnect();
            resolve(performance.now() - t0);
          }
        });
        obs.observe(document.body, { childList: true, subtree: true, attributes: true });
        setTimeout(() => { obs.disconnect(); resolve(-1); }, timeoutMs);
      });
    },
    snapshotGroups() {
      return [...document.querySelectorAll('section[data-testid^="search-group-"]')].map((g) => ({
        id: g.dataset.testid,
        count: g.querySelector("h3 span")?.textContent ?? null,
        cards: [...g.querySelectorAll('[data-testid$="-card"]')].map((c) => c.dataset.testid),
      }));
    },
  };
  true;
`);

// --- 1. Ctrl+F opens the overlay from multiple sections ---
for (const section of ["/live", "/movies", "/settings"]) {
  await evaluate(`document.querySelector('a[href="${section}"]')?.click(); true;`);
  await wait(400);
  await evaluate(`window.__e2e.openSearch(); true;`);
  const t = await evaluate(`window.__e2e.waitFor('[data-testid="search-overlay"]')`);
  report(`Ctrl+F opens search overlay on ${section}`, t >= 0);
  await evaluate(`window.__e2e.pressEscape(); true;`);
  await wait(150);
}

// --- 2. Network silence: capture all requests while searching ---
await send("Network.enable");
captureNetwork = true;

// --- 3. Timing + grouping on a query that hits all three types ---
await evaluate(`window.__e2e.openSearch(); true;`);
await evaluate(`window.__e2e.waitFor('[data-testid="search-overlay"]')`);
// Warm-up query, then measure a fresh one (includes the 200ms debounce).
await evaluate(`window.__e2e.type("zzwarmupzz"); true;`);
await evaluate(`window.__e2e.waitFor('[data-testid="search-no-results"]')`);
const elapsed = await evaluate(`
  (() => {
    const p = window.__e2e.waitFor('section[data-testid^="search-group-"]');
    window.__e2e.type("the");
    return p;
  })()
`);
report(
  "Results appear within 300ms of the user stopping typing",
  elapsed >= 0 && elapsed <= 300,
  `${Math.round(elapsed)}ms (includes 200ms debounce)`,
);

const groups = await evaluate(`window.__e2e.snapshotGroups()`);
const byId = Object.fromEntries(groups.map((g) => [g.id, g]));
report(
  "Results grouped by content type with homogeneous cards",
  groups.length >= 2 &&
    (byId["search-group-live"]?.cards ?? []).every((c) => c === "channel-card") &&
    (byId["search-group-movies"]?.cards ?? []).every((c) => c === "movie-card") &&
    (byId["search-group-series"]?.cards ?? []).every((c) => c === "series-card"),
  groups.map((g) => `${g.id}:${g.count} (${g.cards.length} inline)`).join(", "),
);
report(
  "Max 5 results inline per group",
  groups.every((g) => g.cards.length <= 5),
);

// --- 4. Content type filter tabs ---
await evaluate(`document.querySelector('[data-testid="search-tab-movies"]').click(); true;`);
await wait(400);
const movieOnly = await evaluate(`window.__e2e.snapshotGroups()`);
report(
  "Content type filter limits results to the selected type",
  movieOnly.length === 1 && movieOnly[0].id === "search-group-movies",
  movieOnly.map((g) => g.id).join(", "),
);
await evaluate(`document.querySelector('[data-testid="search-tab-all"]').click(); true;`);
await wait(400);

// --- 5. "Show all" expander ---
const expanded = await evaluate(`
  (async () => {
    const group = document.querySelector('section[data-testid="search-group-movies"]');
    const before = group.querySelectorAll('[data-testid="movie-card"]').length;
    const label = group.querySelector('[data-testid="search-group-movies-show-all"]')?.textContent;
    group.querySelector('[data-testid="search-group-movies-show-all"]')?.click();
    await new Promise((r) => setTimeout(r, 300));
    const after = group.querySelectorAll('[data-testid="movie-card"]').length;
    return { before, after, label };
  })()
`);
report(
  '"Show all" expander reveals all results for a group',
  expanded.before === 5 && expanded.after > expanded.before,
  `${expanded.before} -> ${expanded.after} ("${expanded.label}")`,
);

// --- 6. No-results state ---
await evaluate(`window.__e2e.type("zzqx zzqx"); true;`);
const noRes = await evaluate(`window.__e2e.waitFor('[data-testid="search-no-results"]')`);
const noResText = await evaluate(
  `document.querySelector('[data-testid="search-no-results"]')?.textContent ?? ""`,
);
report(
  "No-results state displays a friendly message",
  noRes >= 0 && noResText.includes("No results for"),
  noResText.trim().slice(0, 80),
);

// --- 7. Network requests during all searches so far ---
captureNetwork = false;
await send("Network.disable");
// ipc.localhost is Tauri's invoke() transport (WebView2 custom scheme to the
// local Rust process) and 1420 is the Vite dev server — neither leaves the
// machine. Lazy poster/logo image loads are card rendering, not search.
const external = networkRequests.filter(
  (r) =>
    !["Image", "Media"].includes(r.type) &&
    !r.url.startsWith("data:") &&
    !/^https?:\/\/(ipc\.localhost|tauri\.localhost|localhost:1420|127\.0\.0\.1:1420)\//.test(r.url),
);
report(
  "Search performed locally — no network requests",
  external.length === 0,
  external.length
    ? external.slice(0, 5).map((r) => `${r.type} ${r.url}`).join("; ")
    : `0 external requests across ${networkRequests.length} captured (IPC + image loads only)`,
);

// --- 8. Clicking a VOD (movie) result opens the detail view ---
await evaluate(`window.__e2e.type("rocky"); true;`);
await evaluate(`window.__e2e.waitFor('section[data-testid="search-group-movies"]')`);
await evaluate(`document.querySelector('section[data-testid="search-group-movies"] [data-testid="movie-card"]').click(); true;`);
const movieDetail = await evaluate(`window.__e2e.waitFor('[data-testid="movie-detail"]')`);
const moviePath = await evaluate(`location.pathname`);
report(
  "Clicking a movie result navigates to the movie detail view",
  movieDetail >= 0 && moviePath === "/movies",
  `path=${moviePath}`,
);
await evaluate(`document.querySelector('[data-testid="detail-back"]')?.click(); true;`);
await wait(200);

// --- 9. Clicking a VOD (series) result opens the detail view ---
await evaluate(`window.__e2e.openSearch(); true;`);
await evaluate(`window.__e2e.waitFor('[data-testid="search-overlay"]')`);
await evaluate(`window.__e2e.type("the"); true;`);
await evaluate(`window.__e2e.waitFor('section[data-testid="search-group-series"]', 3000)`);
await evaluate(`document.querySelector('section[data-testid="search-group-series"] [data-testid="series-card"]').click(); true;`);
const seriesDetail = await evaluate(`window.__e2e.waitFor('[data-testid="series-detail"]')`);
const seriesPath = await evaluate(`location.pathname`);
report(
  "Clicking a series result navigates to the series detail view",
  seriesDetail >= 0 && seriesPath === "/shows",
  `path=${seriesPath}`,
);
await evaluate(`document.querySelector('[data-testid="detail-back"]')?.click(); true;`);
await wait(200);

// --- 10. Clicking a Live TV result starts playback immediately ---
// Provider streams can individually be dead; try up to 3 search results.
let liveOutcome = { ok: false, detail: "no live results" };
for (let attempt = 0; attempt < 3 && !liveOutcome.ok; attempt++) {
  await evaluate(`window.__e2e.openSearch(); true;`);
  await evaluate(`window.__e2e.waitFor('[data-testid="search-overlay"]')`);
  await evaluate(`window.__e2e.type("24/7"); true;`);
  await evaluate(`window.__e2e.waitFor('section[data-testid="search-group-live"]', 3000)`);
  await evaluate(`document.querySelector('[data-testid="search-group-live-show-all"]')?.click(); true;`);
  const channelName = await evaluate(`
    document.querySelectorAll('section[data-testid="search-group-live"] [data-testid="channel-card"]')[${attempt}]?.textContent?.trim()
  `);
  if (!channelName) break;
  await evaluate(`
    document.querySelectorAll('section[data-testid="search-group-live"] [data-testid="channel-card"]')[${attempt}].click(); true;
  `);
  const overlayUp = await evaluate(`window.__e2e.waitFor('[data-testid="player-overlay"]')`);
  let playing = null;
  for (let i = 0; i < 50; i++) {
    playing = await evaluate(`
      (() => {
        const s = window.__proscenium?.player?.getState();
        return { open: s?.open, playing: s?.mpv?.playing ?? false, buffering: s?.mpv?.buffering ?? false, position: s?.mpv?.position ?? 0, error: s?.fatalError ?? null };
      })()
    `);
    if ((playing.playing && playing.position > 0) || playing.error) break;
    await wait(500);
  }
  liveOutcome = {
    ok: overlayUp >= 0 && playing.playing && playing.position > 0,
    detail: `channel="${channelName}" position=${playing.position?.toFixed?.(1)}s buffering=${playing.buffering} error=${playing.error}`,
  };
  await evaluate(`window.__e2e.pressEscape(); true;`);
  await wait(800);
}
report("Clicking a Live TV result starts playback immediately", liveOutcome.ok, liveOutcome.detail);

// --- Summary ---
const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} checks passed`);
ws.close();
process.exit(failed.length ? 1 : 0);
