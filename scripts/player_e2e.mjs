// Drives the real Proscenium app through WebView2 remote debugging (CDP):
// clicks a channel, verifies actual libmpv playback (position advancing,
// hardware decode active), exercises transport controls and fullscreen,
// and confirms the browser survives closing the player.
// Usage: node player_e2e.mjs   (app must be running with
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
ws.onmessage = (msg) => {
  const data = JSON.parse(msg.data);
  if (data.id && pending.has(data.id)) {
    pending.get(data.id)(data);
    pending.delete(data.id);
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
async function pressKey(key, code, keyCode) {
  for (const type of ["keyDown", "keyUp"]) {
    await send("Input.dispatchKeyEvent", {
      type,
      key,
      code,
      windowsVirtualKeyCode: keyCode,
    });
  }
}

const results = {};

// 1. Wait for the channel list, click the first channel.
results.channelClicked = await evaluate(`(async () => {
  const until = async (pred, ms = 15000) => {
    const start = Date.now();
    while (Date.now() - start < ms) {
      const v = pred(); if (v) return v;
      await new Promise(r => setTimeout(r, 200));
    }
    return null;
  };
  const card = await until(() => document.querySelector('[data-testid="channel-card"]'));
  if (!card) return false;
  card.click();
  return true;
})()`);

// 2. Wait for real playback: position advancing, then sample twice.
results.playback = await evaluate(`(async () => {
  const store = window.__proscenium.player;
  const until = async (pred, ms = 20000) => {
    const start = Date.now();
    while (Date.now() - start < ms) {
      const v = pred(); if (v) return v;
      await new Promise(r => setTimeout(r, 200));
    }
    return null;
  };
  await until(() => store.getState().mpv && store.getState().mpv.playing && store.getState().mpv.position > 0.3);
  const p1 = store.getState().mpv?.position ?? -1;
  await new Promise(r => setTimeout(r, 1500));
  const s = store.getState().mpv;
  return s ? {
    overlay: !!document.querySelector('[data-testid="player-overlay"]'),
    playing: s.playing, positionAdvanced: s.position > p1, p1, p2: s.position,
    duration: s.duration, hwdec: s.hwdecCurrent, error: s.error,
    audioTracks: s.audioTracks.length, volume: s.volume,
  } : null;
})()`);

// 3. Keyboard: Space pause / resume.
await pressKey(" ", "Space", 32);
await wait(800);
results.pausedBySpace = await evaluate(
  `window.__proscenium.player.getState().mpv?.paused`,
);
await pressKey(" ", "Space", 32);
await wait(800);
results.resumedBySpace = await evaluate(
  `window.__proscenium.player.getState().mpv?.paused === false`,
);

// 4. Volume / mute via keyboard (same command path as the UI buttons).
await pressKey("ArrowDown", "ArrowDown", 40);
await wait(800);
results.volAfterArrowDown = await evaluate(
  `window.__proscenium.player.getState().mpv?.volume`,
);
await pressKey("m", "KeyM", 77);
await wait(800);
results.mutedByM = await evaluate(
  `window.__proscenium.player.getState().mpv?.muted`,
);
await pressKey("m", "KeyM", 77);

// 5. Seek (VOD content: duration is set since the source is an mp4 file).
results.seek = await evaluate(`(async () => {
  const s = window.__proscenium.player.getState().mpv;
  if (!s || s.duration === null) return { skipped: 'live stream' };
  const before = s.position;
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await new Promise(r => setTimeout(r, 1500));
  const after = window.__proscenium.player.getState().mpv.position;
  return { before, after, jumped: after - before > 5 };
})()`);

// 6. Fullscreen toggle via F key (verified by viewport vs. screen size).
await pressKey("f", "KeyF", 70);
await wait(1200);
results.windowSizeFullscreen = await evaluate(
  `({ w: window.innerWidth, h: window.innerHeight, sw: screen.width, sh: screen.height })`,
);
await pressKey("f", "KeyF", 70);
await wait(1200);
results.windowSizeRestored = await evaluate(
  `({ w: window.innerWidth, h: window.innerHeight })`,
);

// 7. Esc closes the player; the browser returns with state intact.
await pressKey("Escape", "Escape", 27);
await wait(1000);
results.closed = await evaluate(`(() => {
  const ps = window.__proscenium.player.getState();
  return {
    open: ps.open,
    browserVisible: !document.querySelector('.invisible'),
    channelsStillListed: document.querySelectorAll('[data-testid="channel-card"]').length,
  };
})()`);

console.log(JSON.stringify(results, null, 2));
ws.close();
