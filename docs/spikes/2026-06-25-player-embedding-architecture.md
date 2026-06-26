# Spike: Player Embedding Architecture

- **Date:** 2026-06-25
- **Author:** Engineering (Claude Code session)
- **Status:** Complete — findings + recommendation below
- **Outcome (2026-06-25):** Option D (MSE) was prototyped in **Spike D** and **rejected** — it freezes on real live IPTV (`2026-06-25-spike-d-mse-multiview-poc.md`). **Option B (libmpv `render` API) is the chosen direction** for both the player and M37 multi-view; next step is **Spike B**, a single-player render-API PoC.
- **Trigger:** Before generalizing the player to multi-view (Milestone 37), validate whether the way we embed the player is the most seamless / maintainable / architecturally sound option, with **Windows + macOS** as first-class targets. Specifically: is there an SDK or plugin we can adopt instead of hand-rolling window glue and bundling DLLs?

---

## TL;DR

- **The current approach (a separate native video window glued behind a transparent WebView) works, but it is the *fragile* embedding strategy** — and we maintain **two entirely different per-platform implementations** of it.
- **It is not fragile by accident.** It's forced by hard constraints: our format goals require a real media engine (mpv); LGPL requires dynamic linking; and neither WebView2 nor WKWebView lets you composite native GPU content *into* the page. Given those, a behind-the-webview surface is the standard pattern.
- **No plugin or SDK meaningfully improves on what we already have.** The most popular option, `tauri-plugin-libmpv`, uses the *same* transparent-composite trick, is **less** cross-platform-proven than our current code (macOS "not tested", Linux "embedding not working"), and **still bundles the same DLLs**.
- **Bundling the media-engine binary is inherent and unavoidable** if we want mpv-class format support. There is no SDK that ships it for us, and the LGPL choice to dynamically link is deliberate and correct. The only way to drop the binary is to drop the format support (use the browser's `<video>`/MSE or OS-native players).
- **There is one genuinely more sound direction: migrate from window-glue embedding to libmpv's `render` API.** This is **mpv's own recommendation** over window embedding, it **unifies Windows + macOS on a single mechanism**, it removes the z-order glue / self-heal / re-fit fragility, and it makes multi-view far more tractable. It's a real, comparable-precedent path (IPTVnator), but it's a **bounded rewrite with its own GPU-plumbing risks** and should be prototyped first.
- **For Milestone 37 (live-TV multi-view) specifically, also evaluate an MSE path** (`<video>` + `mpegts.js`/`hls.js`). Live TV is overwhelmingly H.264/AAC, which the WebView can play directly — eliminating the native-window problem for the grid entirely. Keep mpv for VOD/full-fidelity.

**Recommendation:** keep mpv (the engine choice is correct), but treat the **render-API migration** as the strategic fix for maintainability + cross-platform + multi-view, gated behind a small proof-of-concept. In parallel, spike the **MSE live-TV path** as a possibly-simpler foundation for M37 specifically.

---

## 1. What we do today

mpv does **not** render into the WebView. We run libmpv (loaded at runtime via `libloading` for LGPL compliance) and put its video into a **separate native window glued directly behind the transparent main window** in the desktop z-order. The HTML page only turns transparent over the player region once frames arrive, so the video shows through exactly there.

This is implemented **twice, differently**:

| | Windows | macOS |
|---|---|---|
| Embedding mechanism | `--wid` into a window **we** create | mpv renders into **its own** window (no `--wid` support in this build) |
| The native surface | a top-level `WS_POPUP` + `WS_EX_TOOLWINDOW \| WS_EX_NOACTIVATE` window (`ProsceniumVideoHost`) | mpv's `NSWindow`, demoted to a borderless child |
| Why not a child window | a child under the full-size WebView is clipped out of DWM composition (swapchain never visible) | n/a — mpv owns the window |
| "Glue" | `SetWindowPos(host, parent, …, SWP_NOACTIVATE)` to keep it immediately below the app window; re-fit on move/resize/focus + self-heal from the player state callback | objc2 `msg_send!`: `setStyleMask:borderless`, match level, `setIgnoresMouseEvents:`, `addChildWindow:ordered:below`, `setFrame:contentRect` |
| Transparency requirement | tao DWM blur-behind + `transparent: true` | `macOSPrivateApi: true` + `macos-private-api` cargo feature |

Relevant code: [`mpv/mod.rs`](../../src-tauri/src/mpv/mod.rs) (`video_host` — both platforms), [`mpv/player.rs`](../../src-tauri/src/mpv/player.rs) (`wid` / `force-window` config), [`commands/playback.rs`](../../src-tauri/src/commands/playback.rs) (`PlayerHandle`, `VideoHost`, `glue_video_window`), [`lib.rs`](../../src-tauri/src/lib.rs) (`on_window_event` re-fit). Distribution: `libmpv-2.dll` + `WebView2Loader.dll` next to the Windows exe; **47 bundled dylibs (with no `@rpath` LC_RPATH)** on macOS (see `RELEASE.md`, `DEVELOPMENT.md`, `CLAUDE.md`).

### Why this is the maintenance/risk hot-spot

- **Two divergent implementations** of the hardest, most platform-specific part of the app.
- **Manual z-order management** that must self-heal (any window slipping between the two breaks the illusion) and **re-fit on every window event**.
- **Transparency coupling** — the whole UI relies on `macOSPrivateApi` (a private API) and DWM behavior.
- **Multi-view multiplies all of it** — Milestone 37 would need **N** of these windows, each glued and re-fitted, which is why this spike exists.

---

## 2. The constraints that force this shape

Any proposal has to respect these, or it's not really a candidate:

1. **Format breadth is a core product goal.** SPEC §2/§7 require "the widest possible range of video and audio formats, including HDR and lossless audio" — Dolby Vision, Dolby Atmos, TrueHD, DTS-HD MA, VC-1, MKV, raw MPEG-TS, etc. **Only a full media engine (mpv / VLC / GStreamer) covers this.** Browser `<video>`/MSE and OS-native players (AVPlayer, Media Foundation) do **not**.
2. **LGPL → dynamic linking.** libmpv is LGPL; as a proprietary app we must dynamically link and offer the libmpv source on request (SPEC §3). This **rules out static linking** the engine into the exe — i.e., we *will* ship a separate binary. (Same for libVLC/GStreamer.)
3. **WebViews don't let you composite native GPU content into the page.** WebView2 (Chromium) and WKWebView don't expose their compositor/GPU surface; you can't hand mpv "the page's framebuffer." So native video lives **behind** (transparent webview) or **beside** (separate window) the page — not *in* it — unless you copy frames into the page yourself (see Option C).
4. **Child windows get clipped from DWM composition on Windows** — hence the top-level-window-glued-behind trick rather than a simple child HWND.

---

## 3. Options evaluated

| # | Option | Engine binary still bundled? | Format breadth | Cross-platform unification | Compositing fragility | Effort to adopt | Verdict |
|---|---|---|---|---|---|---|---|
| A | **Status quo** — separate window glued behind transparent webview | Yes | Full (mpv) | ✗ two impls | High (z-order glue, self-heal) | — (current) | Works; fragile |
| B | **libmpv `render` API → composited child GPU surface** | Yes | Full (mpv) | ✓ one mechanism | Low–Med (real child surface) | **High (rewrite)** | **Recommended direction** |
| C | Render API → upload frames into the webview (wgpu/WebGL/canvas) | Yes | Full (mpv) | ✓ | Med (immature; "flickers on resize") | Very high | Not yet |
| D | **MSE `<video>` (`mpegts.js`/`hls.js`)** — no native engine | **No** | **Limited** (H.264/AAC) | ✓ (it's the webview) | **None** | Med | **Adopt for live-TV/multi-view only** |
| E | `tauri-plugin-libmpv` / `tauri-plugin-mpv` | Yes | Full (mpv) | ✗ (worse than ours) | High (same trick) | Low | No gain |
| F | GStreamer (`gstreamer-rs`) | Yes (+ plugin tree) | Full | ~ | High ("renders above the page") | High | Worse bundling |
| G | libVLC (`vlc-rs`) | Yes (+ plugins folder) | Full | ~ | High (same window-embed problem) | High | No gain |
| H | OS-native (AVPlayer / Media Foundation) | No | **Limited** & **divergent** | ✗ | Low | High | Fails format goal |

### A — Status quo (current)
Honest assessment: it's a legitimate, shipped, *working* approach (Windows fully verified; macOS verified on real hardware per the milestones). Its weakness is exactly what this spike flags — fragility and double-maintenance — which **multi-view amplifies**.

### B — libmpv `render` API (recommended direction)
Instead of mpv owning a window, you create an `mpv_render_context` and **render mpv's video into a GPU surface you own** (an OpenGL FBO / texture; Metal on macOS). mpv's own client-API guidance is explicit:

> "Due to the various platform-specific behaviour and problems (in particular on macOS), using the render API is currently recommended over window embedding." — mpv `client-api-changes` / examples

A directly comparable app, **IPTVnator**, migrated to this exact approach: its native addon creates an `mpv_handle`, sets `vo=libmpv`, and renders through the render API into a macOS `NSOpenGLView`, **because the `--wid` window-embedding approach "was not reliable enough on macOS."**

Why this is more sound for us:
- **One mechanism for both platforms** (render into a surface) instead of two window-glue hacks.
- The surface is a **real child** of the window → no separate top-level window, no `SetWindowPos` z-order glue, no self-heal loop, no "window slipped in between."
- **Multi-view becomes tractable:** N render contexts drawn into N sub-rects of one surface (or N child surfaces) — no N separate native windows to individually glue and re-fit.
- Still works with the transparent-webview model (surface behind the page), but the layering is structural, not a per-frame fight.

Costs / risks (do not under-sell):
- **It's a real rewrite** of `mpv::video_host` + the player init (`vo=libmpv`, render-context lifecycle on a GL/Metal context).
- GPU-context plumbing: a shared GL context, correct **render-context teardown ordering** (must drop before the player — the Rust binding can't enforce it at compile time), and the documented **"flickers when resizing"** class of issues seen in the wry experiments.
- macOS needs a libmpv built with the right context (cocoa-cb / GL), which is a build-pipeline change.

### C — Render API → into the webview page
The "most seamless" end state (video as actual page content, no transparency tricks, trivial CSS compositing). But the Tauri/wry community has chased this since 2021 with **"no official solution"**; reports include GStreamer "rendering above the surface covering all html elements" and OpenGL paths that "flicker when resizing." Multi-webview + wgpu is emerging but immature. **Too risky to bet on now.**

### D — MSE `<video>` (`mpegts.js` / `hls.js`) — the pragmatic multi-view play
Play streams directly in HTML5 `<video>` by transmuxing in JS: `mpegts.js` demuxes MPEG-TS → fMP4 via Media Source Extensions; `hls.js` does the same for HLS. **No native window, no DLL, no transparency sandwich** — it's just DOM, so a multi-view grid is *N `<video>` elements in CSS grid*.
- **Limitation:** MSE in WebView2/WKWebView supports roughly **H.264 + AAC/MP3** (HEVC/AC3 are spotty/platform-dependent). It will **not** do TrueHD/DTS-HD/Dolby Atmos/VC-1/MKV.
- **But live TV is overwhelmingly H.264/AAC MPEG-TS or HLS** — exactly MSE's sweet spot. So this is a strong, much-simpler foundation **for live TV and especially multi-view**, with mpv retained for VOD and full-fidelity playback. This directly de-risks Milestone 37.

### E — `tauri-plugin-libmpv` / `tauri-plugin-mpv`
- `tauri-plugin-libmpv` (nini22P): embeds libmpv, **requires `transparent: true`** (i.e., the *same* composite trick), bundles `libmpv-2.dll` + a `libmpv-wrapper.dll`. Self-reported support: **Windows "fully tested", macOS "not tested", Linux "window embedding is not working."** That's **less** proven cross-platform than our current code.
- `tauri-plugin-mpv` (nini22P): drives an **external mpv.exe** via JSON IPC. Avoids libmpv linking but means shipping/locating a full mpv binary and managing a child process — different trade-offs, not obviously better, and weaker control than libmpv.
- **Conclusion:** adopting a plugin would *regress* our cross-platform maturity and still bundle DLLs. The useful idea to borrow is the plugin's *render path*, not the plugin itself — which points back to Option B.

### F — GStreamer
Full-featured and cross-platform, with Rust bindings (`gstreamer-rs`) and platform sinks. But: a **heavier distribution footprint** (a plugin tree, not one dll), and the same webview-compositing problem (community reports it "rendering above the surface covering all html elements"). No bundling win, more moving parts.

### G — libVLC
Same shape as mpv (native engine, window-handle embedding via `libvlc_media_player_set_hwnd` / `set_nsobject`, or memory callbacks), **same LGPL dynamic-link requirement**, and a **larger** distribution payload (VLC's `plugins/` folder). No improvement over mpv; we'd re-solve the same embedding problem with a bigger binary.

### H — OS-native players (AVPlayer / Media Foundation)
No bundled engine, but **fails the format goal** (no MKV/TS-non-HLS/lossless audio) and forces **two divergent codebases**. Non-starter as the primary engine. (AVPlayer's native HLS could be a niche macOS fast-path, but not a foundation.)

---

## 4. "Can we stop packaging DLLs?" — direct answer

**Largely no, and that's expected.** Shipping the media-engine binary is intrinsic to having mpv-class playback:

- There is **no SDK/package that ships libmpv's binary for us** in a way that avoids bundling. The Rust crates (`libmpv2`, `libmpv-sys`) are *bindings*; they link against a libmpv you provide. `tauri-plugin-libmpv` still downloads + bundles `libmpv-2.dll`.
- **LGPL deliberately requires** dynamic linking (SPEC §3), so the engine is a separate file by design — that compliance posture is correct and worth keeping.
- `WebView2Loader.dll` on Windows is a **separate, unrelated** GNU-toolchain artifact (CLAUDE.md) and is independent of the player choice.

The **only** way to ship *no* media binary is Option D/H — i.e., accept the browser/OS codec set. That's viable **for live TV** (Option D) but not as the whole player.

What we *can* improve about distribution regardless of engine approach:
- The macOS **47-dylib / `@rpath`-scrubbing** dance (RELEASE.md) is the real bundling pain. Migrating to the render API doesn't remove the libmpv dependency, but standardizing on an **official/pinned libmpv build** (vs. Homebrew's transitive dylib forest) could shrink and simplify the macOS payload.

---

## 5. Recommendation

1. **Keep mpv.** The engine choice is correct and non-negotiable given the format goals + LGPL posture. No SDK/plugin/alternative engine improves on it.
2. **Adopt the libmpv `render` API as the strategic embedding target** (Option B), replacing the dual window-glue implementations with a single render-into-a-surface mechanism. This is mpv's own recommendation, has a comparable-IPTV-app precedent, unifies Windows + macOS, removes the z-order/self-heal fragility, and is the thing that makes **multi-view sane**. Treat it as a **rewrite of `mpv::video_host` + player init**, gated behind step 4.
3. **For Milestone 37 (live-TV multi-view), evaluate the MSE path (Option D) in parallel.** If WebView2 + WKWebView play our providers' live streams acceptably via `mpegts.js`/`hls.js`, multi-view becomes "N `<video>` in a CSS grid" with **zero** native-window work — a dramatically smaller, lower-risk M37 — while mpv remains the VOD/full-fidelity player. This may even reorder M37's plan.
4. **De-risk before committing** with two small spikes (below). Do **not** start the render-API rewrite or M37 until these report back.
5. **Don't adopt a third-party plugin** (`tauri-plugin-libmpv`/`-mpv`) — it regresses cross-platform maturity and changes nothing about bundling.

### Proposed next spikes
- **Spike B (render API PoC):** single mpv player via `mpv_render_context` into an OpenGL FBO (Windows) and a GL/Metal layer (macOS), composited behind the webview. Success = plays a stream, resizes without flicker, clean teardown. Decision gate for the rewrite.
- **Spike D (MSE live-TV PoC):** play 1 and then 4 real provider live streams in `<video>` via `mpegts.js`/`hls.js` inside WebView2 **and** WKWebView. Measure codec coverage against real provider channels, CPU/GPU, and audio-focus switching. Decision gate for how M37 is built.

---

## Sources

- [tauri-apps/tauri Discussion #6343 — Embedding an mpv/vlc video player inside a window](https://github.com/tauri-apps/tauri/discussions/6343)
- [tauri-apps/wry Discussion #284 — Rendering canvas/video on the native side](https://github.com/tauri-apps/wry/discussions/284)
- [nini22P/tauri-plugin-libmpv (GitHub)](https://github.com/nini22P/tauri-plugin-libmpv) · [crates.io](https://crates.io/crates/tauri-plugin-libmpv) · [docs.rs](https://docs.rs/crate/tauri-plugin-libmpv/latest)
- [nini22P/tauri-plugin-mpv (GitHub)](https://github.com/nini22P/tauri-plugin-mpv) · [crates.io](https://crates.io/crates/tauri-plugin-mpv/0.2.5)
- [mpv-player/mpv-examples — libmpv (render API examples, incl. `cocoa-rendergl`)](https://github.com/mpv-player/mpv-examples/tree/master/libmpv)
- [mpv `DOCS/client-api-changes.rst` (render API recommended over window embedding)](https://github.com/mpv-player/mpv/blob/master/DOCS/client-api-changes.rst)
- [mpv Issue #12518 — "No render context set." (macOS render-context requirement)](https://github.com/mpv-player/mpv/issues/12518)
- [IPTVnator — Experimental embedded MPV support on macOS (render API into NSOpenGLView)](https://4gray.github.io/iptvnator/blog/embedded-mpv-macos-experiment/)
- [libmpv-sys — `mpv_opengl_fbo` / `mpv_opengl_init_params`](https://docs.rs/libmpv-sys/latest/libmpv_sys/struct.mpv_opengl_fbo.html)
- [ParadoxSpiral/libmpv-rs PR #22 — Renderer API wrapper](https://github.com/ParadoxSpiral/libmpv-rs/pull/22)
- [xqq/mpegts.js — HTML5 MPEG2-TS / FLV player (MSE)](https://github.com/xqq/mpegts.js/)
- [HLS.js — adaptive streaming via MSE transmuxing](https://www.videosdk.live/developer-hub/hls/hls-js)
- [MDN — Media Source Extensions API](https://developer.mozilla.org/en-US/docs/Web/API/Media_Source_Extensions_API)
