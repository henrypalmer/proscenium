# Spike D — MSE Live-TV Multi-View POC

- **Date:** 2026-06-25
- **Branch:** `poc/mse-live-tv`
- **Status:** **Closed — MSE rejected for live playback.** The architecture works (multi-view = N `<video>` in a grid), but on the real provider the streams **freeze hard after the first buffer or two** and stay unusable even after the stability tuning (§4a). Decision: **M37 will use the libmpv `render` API**, not MSE.
- **Question (from the embedding spike):** can live TV — and especially multi-view — be played with HTML5 `<video>` + MSE (`mpegts.js` / `hls.js`) **inside the WebView**, eliminating the native-window "sandwich" for that path and making the M37 grid trivial?

> ## Decision (2026-06-25)
> **MSE is not viable for live IPTV playback in this product.** Real-provider testing (after the §4a stability fixes) still froze hard on the first buffer or two — far worse than the existing mpv player, which plays the same channels cleanly. The browser MSE pipeline can't match ffmpeg/mpv's tolerance of messy live IPTV (timestamp/PCR discontinuities, buffer underruns). The architectural simplicity isn't worth a visibly worse picture.
> **→ M37 (Live TV Multi-View) will be built on the libmpv `render` API** (Option B from `2026-06-25-player-embedding-architecture.md`): render N mpv instances into N viewports of one composited surface — keeping mpv's robust playback **and** enabling the grid, without today's N-separate-native-windows fragility. **Next step:** Spike B, a single-player render-API proof of concept, to de-risk the GPU-context plumbing before committing the milestone.
> The MSE POC lives on the unmerged `poc/mse-live-tv` branch as a record; it is **not** going into `develop`.

---

## 1. TL;DR

**Yes — the core hypothesis holds.** With MSE, multi-view is just **N `<video>` elements in a CSS grid**: no native windows, no z-order glue, no per-cell window fitting, and trivially correct audio focus. Verified two real streams decoding **concurrently** side-by-side with audio swapping cleanly between tiles, zero console errors.

The one non-obvious requirement surfaced and was solved: **provider streams need a local proxy** (built here) because IPTV providers send no CORS headers — and the proxy is a bonus, because it also keeps the keychain password server-side.

**What this de-risks for M37:** *architecturally*, the multi-view milestone could collapse from "generalize the native-window player to N windows" (the hard, platform-specific path) to "render N `<video>` in a grid" — a fraction of the risk and effort, with mpv retained for VOD.

**But — important caveat (§4a):** the real-provider test shows MSE **buffers/freezes noticeably more than mpv** on the same live channels. Part of that was a POC misconfiguration (now fixed — mpegts.js was tuned for latency, not stability; proxy lacked `TCP_NODELAY`), and part is likely fundamental (mpv/ffmpeg tolerates messy IPTV streams far better than browser MSE). **A re-test with the fixes is the deciding factor:** if stability matches mpv, go MSE; if not, M37 should instead use the **libmpv render API** (which keeps mpv's robustness *and* enables multi-view). See §6.

---

## 2. What was built

All under `src/poc/mse/` + `src-tauri/src/commands/poc.rs`, reachable at **`/poc/mse`** via a temporary floating **🧪 MSE POC** button (POC branch only).

- **`MseTile.tsx`** — one `<video>` driven by `mpegts.js` (MPEG-TS, `type: 'mse', isLive: true`) or `hls.js` (HLS), with native-HLS fallback. Per-tile diagnostics (state, engine, resolution, error). Muted unless it's the active (audio) tile.
- **`MseMultiViewPoc.tsx`** — the page: a WebView-capability probe; up to **4** tiles in an adaptive CSS grid (1 → full, 2 → side-by-side, 3/4 → 2×2); **audio focus** (click a tile → it gets audio, others mute, emerald ring); add via a **live-channel picker** (real provider channels) or a **manual URL** (any CORS-enabled test stream); a diagnostics footer.
- **`commands/poc.rs`** — a **localhost stream proxy** (`http://127.0.0.1:<port>`): `GET /live?provider&channel` resolves the real keychain-composed URL **server-side** and pipes it back with permissive CORS (so the WebView fetch is same-origin and the password never reaches the page); `GET /proxy?url=` passes through a raw URL. Bound at startup in `lib.rs`; the port is exposed via `poc_proxy_base`.

Dependencies added: `mpegts.js`, `hls.js`.

---

## 3. What was verified (Chromium / preview)

| Check | Result |
|---|---|
| WebView capability probe | ✓ MSE · ✓ mpegts.js live · ✓ hls.js · ✓ native HLS |
| Single tile plays a real HLS stream | ✓ `readyState 4`, **1920×1080**, `currentTime` advancing |
| Two tiles, side-by-side grid | ✓ both decoding **concurrently** (`playingCount: 2`) |
| Audio focus | ✓ muted state moved `[false,true] → [true,false]` on click; exactly one tile has audio |
| Console | ✓ no errors |
| `npm run build` / `cargo check` | ✓ both clean |

> Note: the preview is Chromium (same engine family as WebView2), so MSE/`mpegts.js`/`hls.js` support is representative — but **not identical** to shipping WebView2/WKWebView. Codec coverage especially must be re-checked in the real app (§5).

The screenshot in the session shows two tiles playing the Mux test stream, the right tile ringed emerald (audio), the left muted — the whole architectural point in one frame.

---

## 4. Findings

1. **Multi-view with MSE is trivial and correct.** N `<video>` in a CSS grid; concurrent decode; audio focus is a one-line `muted` toggle. None of the native-window machinery (separate top-level windows, `SetWindowPos`/objc2 glue, z-order self-heal, per-cell fit on every window event) is needed.
2. **CORS is the catch — and the proxy solves it well.** Direct `fetch` of a provider stream from the WebView is CORS-blocked. The localhost proxy fixes that **and** improves on the current design: the provider password is composed and used **server-side**, never reaching the page (today's mpv path composes it in Rust too, but a naive `<video src=…>` would have leaked it — the proxy avoids that trap).
3. **Codec breadth is the real boundary** (expected, per the embedding spike). MSE covers ~**H.264 + AAC/MP3**; HEVC/AC3 are platform-dependent; TrueHD/DTS-HD/VC-1/MKV are out. Live TV is overwhelmingly H.264/AAC, so this is fine **for live** — but it's why **mpv stays** for VOD and full-fidelity playback.
4. **Engine selection differs by WebView.** The preview used native HLS; WebView2 will likely fall to `hls.js`; `.ts` channels use `mpegts.js`. The tile already handles all three paths.

---

## 4a. Real-provider test — buffering / freezing (the deciding finding)

Testing against the real provider in the Tauri app: **channels play, but buffer much more and sometimes freeze, while the *same* channel in the established mpv player does not buffer at all.** This is the make-or-break result for the MSE approach, and there are two distinct causes:

**(i) A POC misconfiguration — fixed.** The first cut configured mpegts.js for *low latency*, which is exactly wrong for jittery IPTV:
- `liveBufferLatencyChasing: true` — re-seeks to the live edge whenever the buffer dips, so every network hiccup becomes a stall. (Now `false`.)
- `enableStashBuffer: false` — removed the IO cushion. (Now `true`, with a larger `stashInitialSize` + `autoCleanupSourceBuffer`.)
- The localhost proxy lacked **`TCP_NODELAY`**, so Nagle coalesced the small chunked writes and added jitter the MSE buffer had to absorb. (Now set.)

→ **Re-test needed in the real app** to see how much of the gap these close.

**(ii) A likely-fundamental gap — may not close.** mpv/ffmpeg has a deep, mature, jitter-tolerant demuxer cache and handles IPTV's messy reality (timestamp/PCR discontinuities, PAT/PMT changes, variable bitrate, mid-stream hiccups) gracefully. Browser MSE + mpegts.js does its transmuxing in JS with a shallower buffer and is **inherently more sensitive** to exactly those conditions — which is what produces the extra buffering and the freezes. Tuning narrows this; it may not erase it.

**Why this matters:** the whole appeal of the MSE path was a *simpler* multi-view. If the trade is "trivial grid, but a visibly worse/less-stable picture than the player we already ship," that's a bad trade for a product whose §2 goal is smooth playback. So this gap — after the §4a(i) fixes — is the single fact that should decide M37's direction (see §6).

---

## 5. What still must be validated (real app + real provider)

These are the questions only the Tauri build against the real provider can answer — **run `npm run tauri dev`, click 🧪 MSE POC, "Add channel"**:

- [ ] **Do real provider live channels actually play** in WebView2 via the proxy + `mpegts.js`? (The provider's channels are MPEG-TS; this is the core codec-coverage question.)
- [ ] **4 tiles at once** — CPU/GPU/decode and network headroom on the target hardware; respect the provider `max_connections` ceiling (M37 design).
- [ ] **macOS / WKWebView** — same checks (MSE + `mpegts.js` support, codec coverage).
- [ ] **`.m3u8` live channels** — the proxy currently passes bytes straight through, which works for a single continuous TS but **not** for HLS (the playlist's segment URLs would also need proxying/rewriting). Decide whether to handle HLS channels via `hls.js` pointed at a playlist-rewriting proxy, or punt them to mpv.

---

## 6. Recommendation (revised after §4a)

The MSE path is **architecturally** a clear win for multi-view (proven in §3). But §4a shows it has a **playback-quality cost** for live IPTV that the established mpv player doesn't. So the decision is now gated on **one re-test**, not on the architecture:

**Decision gate — re-test live channels in the real app with the §4a(i) fixes applied:**

- **If stability now matches mpv** (no meaningful extra buffering/freezing across a range of the provider's channels): **proceed with MSE for M37.** Fold this POC into a real `MultiView` feature (replace the temp `/poc/mse` route + button), harden the proxy (HLS playlist rewriting, lifecycle, per-tile error states, the `max_connections` budget), and wire the §5.3 entry points. Keep mpv as the VOD / full-fidelity player.

- **If it still buffers/freezes** noticeably worse than mpv: **do not ship MSE for the main viewing path. Pivot M37 to the libmpv `render` API** (Option B in the embedding spike, `2026-06-25-player-embedding-architecture.md`). That keeps mpv's robust, jitter-tolerant playback **and** enables multi-view by rendering N mpv instances into N viewports of one composited surface — more implementation work than MSE, but **far** less than today's N-separate-native-windows approach, and with no quality regression. This is the safer bet for a product whose core promise is smooth playback.

A reasonable **hybrid** also exists if MSE is close-but-not-perfect: use MSE only for the *secondary* multi-view tiles (where a little buffering is acceptable while you monitor several games) and keep the *focused/audio* tile on mpv. More complex; only worth it if the simple gate is ambiguous.

**Bottom line:** the POC did its job — it turned "MSE makes multi-view trivial" into a concrete, measured trade-off. The architecture is sound; the open question is purely playback stability, and the §4a(i) re-test answers it.

**Production hardening notes (when promoting out of POC):**
- The proxy is intentionally minimal: no Range support, no playlist rewriting, no connection accounting. Production needs per-tile error surfacing (reuse the §12/M22 classifier), the `max_connections` budget, and clean teardown.
- Code-split `mpegts.js`/`hls.js` (they add ~1 MB to the bundle) behind a dynamic import so they only load for multi-view.
- Decide the engine/codec fallback policy explicitly (native HLS → `hls.js` → `mpegts.js`), and define the behavior when a channel's codec is unsupported (fall back to opening that channel in the mpv player?).

---

## How to run

```sh
npm install            # picks up mpegts.js + hls.js
npm run tauri dev      # real app — required for the channel proxy + WebView2 codec check
# click the floating "🧪 MSE POC" button (bottom-right) → /poc/mse
#   • "Add channel"  → real provider live streams (via the localhost proxy)
#   • paste a URL    → any CORS-enabled test stream (e.g. an HLS .m3u8)
# click a tile to give it audio.
```
