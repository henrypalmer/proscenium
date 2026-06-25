# Spike D — MSE Live-TV Multi-View POC

- **Date:** 2026-06-25
- **Branch:** `poc/mse-live-tv`
- **Status:** POC built + verified in Chromium (preview). Real-app / real-provider validation still required (see §5).
- **Question (from the embedding spike):** can live TV — and especially multi-view — be played with HTML5 `<video>` + MSE (`mpegts.js` / `hls.js`) **inside the WebView**, eliminating the native-window "sandwich" for that path and making the M37 grid trivial?

---

## 1. TL;DR

**Yes — the core hypothesis holds.** With MSE, multi-view is just **N `<video>` elements in a CSS grid**: no native windows, no z-order glue, no per-cell window fitting, and trivially correct audio focus. Verified two real streams decoding **concurrently** side-by-side with audio swapping cleanly between tiles, zero console errors.

The one non-obvious requirement surfaced and was solved: **provider streams need a local proxy** (built here) because IPTV providers send no CORS headers — and the proxy is a bonus, because it also keeps the keychain password server-side.

**What this de-risks for M37:** if the real WebView2/WKWebView codec check (§5) passes for the provider's live channels, the multi-view milestone collapses from "generalize the native-window player to N windows" (the hard, platform-specific path) to "render N `<video>` in a grid" — a fraction of the risk and effort. mpv stays as the VOD / full-fidelity player.

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

## 5. What still must be validated (real app + real provider)

These are the questions only the Tauri build against the real provider can answer — **run `npm run tauri dev`, click 🧪 MSE POC, "Add channel"**:

- [ ] **Do real provider live channels actually play** in WebView2 via the proxy + `mpegts.js`? (The provider's channels are MPEG-TS; this is the core codec-coverage question.)
- [ ] **4 tiles at once** — CPU/GPU/decode and network headroom on the target hardware; respect the provider `max_connections` ceiling (M37 design).
- [ ] **macOS / WKWebView** — same checks (MSE + `mpegts.js` support, codec coverage).
- [ ] **`.m3u8` live channels** — the proxy currently passes bytes straight through, which works for a single continuous TS but **not** for HLS (the playlist's segment URLs would also need proxying/rewriting). Decide whether to handle HLS channels via `hls.js` pointed at a playlist-rewriting proxy, or punt them to mpv.

---

## 6. Recommendation

**Proceed with the MSE path as the primary plan for M37 (live-TV multi-view), pending the §5 real-app codec/perf check.** It removes the riskiest, most platform-specific work from the milestone. Keep the built-in **mpv player as-is** for single-stream VOD and any codec MSE can't handle.

If §5 passes, fold this POC's components into a real `MultiView` feature (replacing the temporary `/poc/mse` route + button), harden the proxy (HLS playlist rewriting, lifecycle, error states), and wire the entry points from the §5.3 Live TV UI per the M37 spec.

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
