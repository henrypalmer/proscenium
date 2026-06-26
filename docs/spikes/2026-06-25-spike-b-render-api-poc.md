# Spike B — libmpv `render` API PoC

- **Date:** 2026-06-25
- **Branch:** `poc/render-api`
- **Status:** **Phase 1 PASS** (core GPU plumbing, headless-verified). Two things remain: a human visual resize-flicker check, and the WebView-compositing integration (Phase 2).
- **Question:** can we drive libmpv's **render API** (render mpv into a GPU surface *we* own) instead of the current `--wid` window-glue — on Windows, with our shipped libmpv build? This is the foundation the embedding spike recommended and that M37 (multi-view) needs.

---

## 1. TL;DR

**Yes — the render API works with our libmpv, cleanly.** A standalone example (`src-tauri/examples/render_api_spike.rs`) creates its own WGL/OpenGL context, hands mpv a `get_proc_address`, and mpv renders each frame into our default framebuffer via `mpv_render_context_render` → `SwapBuffers`. Headless run (8 s auto-quit):

```
[spike] loaded …\src-tauri\lib\libmpv-2.dll
[spike] GL context created
[spike] GL_VERSION  = 4.6.0 NVIDIA 610.62
[spike] GL_RENDERER = NVIDIA GeForce RTX 4080 SUPER/PCIe/SSE2
[spike] render context created OK          ← the #1 unknown: this build supports the GL render API
[spike] resize -> 1264x681
[spike] 120 frames (39 fps avg), 1264x681
[spike] 240 frames (47 fps avg), 1264x681
[spike] 360 frames (51 fps avg), 1264x681
[spike] auto-quit after 8s
[spike] tearing down…
[spike] done — 414 frames rendered.        ← clean teardown, no hang/crash
```

This de-risks the GPU plumbing that the embedding spike flagged as the riskiest part of moving off window-glue. The render context creates, frames flow, and teardown (free render context **before** the player) is clean.

Bonus vs Spike D: mpv played the **HLS test stream directly — no proxy, no transmux** — i.e. the robust path that browser MSE couldn't deliver.

---

## 2. What was built

`src-tauri/examples/render_api_spike.rs` — a **standalone** Windows example (its own window + WGL context + message loop), deliberately **not** wired into the Tauri app, so the render-API mechanism is proven in isolation before tackling WebView compositing.

- Loads `libmpv-2.dll` (from `src-tauri/lib/` or PATH) + the render-API symbols (`mpv_render_context_create/render/update/report_swap/free`).
- Creates a legacy WGL context (gives a 4.6 compatibility context on NVIDIA); `get_proc_address` = `wglGetProcAddress` with an `opengl32.dll` fallback for GL 1.1 entry points.
- `vo=libmpv` + `hwdec=auto-safe`; `mpv_render_context_create` with `MPV_RENDER_PARAM_API_TYPE=opengl` + init params.
- Render loop: render only on `MPV_RENDER_UPDATE_FRAME`, into FBO 0 at the live client size, `SwapBuffers`, `report_swap`.
- `SPIKE_SECS=N` auto-quit for headless lifecycle/teardown testing.

`Cargo.toml`: added `Win32_Graphics_OpenGL` + `Win32_System_LibraryLoader` to the existing `windows-sys` (binding availability only; no runtime cost).

**Run:**
```sh
cd src-tauri
cargo run --example render_api_spike                 # public HLS test stream; close the window to exit
cargo run --example render_api_spike -- "<url>"      # any mpv-playable URL, incl. a real provider stream
SPIKE_SECS=8 cargo run --example render_api_spike     # headless: auto-quit + teardown after 8s
```

---

## 3. What's de-risked vs what remains

| | Status |
|---|---|
| This libmpv build supports the OpenGL render API | ✅ verified |
| GL context creation (WGL) on the target GPU | ✅ verified (GL 4.6) |
| mpv renders frames into a surface we own | ✅ verified (frames flowing) |
| Clean teardown (free render ctx → terminate player) | ✅ verified (no hang/crash) |
| Plays real live IPTV robustly | ✅ inherent (it's mpv/ffmpeg — the existing player's strength; re-run with a provider URL to confirm) |
| **Resize without flicker / black flashes** | ⏳ **human visual check** — run it and drag-resize the window |
| **Compositing behind the Tauri WebView** (the "sandwich", but our surface) | ⏳ **Phase 2** — not attempted yet |
| Multiple render contexts → N viewports (multi-view) | ⏳ Phase 3 / M37 |

The headless logs can't show flicker, so **please run it and resize the window** — that's the one remaining known-risk from the wry experiments ("OpenGL paths flicker when resizing").

---

## 4. Recommendation / next steps

Phase 1 is a clear **go**: the render API is viable on Windows with our libmpv, with mpv's full robustness intact. Proceed in order:

1. **Visual resize check** (you): confirm no flicker on drag-resize. If it flickers, that's the one thing to solve before Phase 2 (usually fixed by rendering on the update callback + not resizing the swapchain mid-frame, or a WGL swap-control/`glFinish` tweak).
2. **Phase 2 — composite behind the WebView:** replace the example's own window with a child/host surface glued behind the transparent Tauri WebView (reuse the existing video-host window, but now *we* render into it via the render context instead of mpv owning it via `--wid`). This proves the real integration and is the basis for the **player render-API migration** (which also cleans up the single-player path on both OSes).
3. **macOS Phase:** the same render API with a GL or Metal context (mpv recommends the render API precisely because macOS window-embedding is unreliable). Untestable here (no Mac), but it's the same mechanism.
4. **M37:** N render contexts → N viewports of one composited surface.

If the resize check is clean, M37-on-the-render-API is low-risk, and the player migration is the natural first implementation slice.
