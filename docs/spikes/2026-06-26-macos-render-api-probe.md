# macOS Render-API Probe — pickup instructions

- **Date:** 2026-06-26
- **Status:** ⏳ **Not started** — to be run on a Mac.
- **Owner action:** this is a **go/no-go probe**, not the migration. Answer one question, record the result, stop.
- **Prereq for:** Milestone 38 (player render-API migration) — specifically its macOS acceptance criterion and risk line (`SPEC.md` §19). Windows is already proven by Spike B (`docs/spikes/2026-06-25-spike-b-render-api-poc.md`).

---

## 0. The one question this probe answers

**Does the Mac's libmpv support `mpv_render_context_create` with `MPV_RENDER_API_TYPE_OPENGL`, and do frames flow into a GL context we own?**

This is the exact macOS analogue of Spike B's "#1 unknown" (which on Windows printed `render context created OK`). Everything else in M38's macOS path (compositing behind the WebView, the render thread, teardown ordering) is already de-risked by the Windows spike and the existing macOS embedding code — the *only* untested thing is whether this particular libmpv build will hand us frames through the GL render API on macOS.

Decision outcomes:

- **PASS** → M38 macOS path is viable as planned; proceed to build it on the render API (unify with Windows).
- **PARTIAL** (GL fails, software `sw` works) → render API is present but not via GL; note it — software blit is too slow to ship, so macOS would need a GL fix or a different libmpv build.
- **FAIL** (`MPV_ERROR_NOT_IMPLEMENTED` for both) → trigger the M38 fallback: ship the render API on **Windows only**, keep the current macOS `--wid`/NSWindow-demote embedding, file a follow-up to source a render-capable libmpv on macOS.

---

## 1. Critical macOS facts (read before writing any code)

These are the things that differ from the Windows spike and will bite if missed:

1. **libmpv's render API has only two types: `opengl` and `sw` (software). There is NO Metal render-API type.** The SPEC's "GL or Metal" phrasing is a hedge — *Metal is not a render-API option in libmpv*. The macOS render path **is OpenGL**. (If GL were unavailable you'd be looking at `--vo=gpu-next`/libplacebo, which is a different, non-render-API mechanism and out of scope for this probe.) So: probe **OpenGL first**, fall back to **`sw`** only to distinguish PARTIAL from FAIL.

2. **macOS OpenGL is deprecated but functional.** Apple froze it at GL 4.1 (10.14+), and on Apple Silicon it's emulated over Metal. So expect `GL_VERSION` to read something like `4.1 Metal - 89.x` — that is **normal and fine** (mpv's GL renderer needs ≥3.2 core). Don't mistake "4.1" for a problem; Windows just happened to show 4.6 on NVIDIA.

3. **The dylib is `libmpv.2.dylib`, not a `.dll`.** Search order for the probe:
   - `src-tauri/lib/libmpv.2.dylib` (repo's bundled copy, gitignored — same place RELEASE.md stages it)
   - Homebrew: `/opt/homebrew/opt/mpv/lib/libmpv.2.dylib` (Apple Silicon) or `/usr/local/opt/mpv/lib/libmpv.2.dylib` (Intel)
   - Bare `libmpv.2.dylib` via the loader's default search
   Load by **absolute path** with `libloading::Library::new(...)` to sidestep `@rpath` resolution.

4. **`get_proc_address` is NOT `wglGetProcAddress` on macOS.** Resolve GL symbols with `dlsym`. Robust form: `dlopen("/System/Library/Frameworks/OpenGL.framework/Versions/Current/OpenGL", RTLD_LAZY|RTLD_LOCAL)` once, then `dlsym(handle, name)` per symbol. (`dlsym(RTLD_DEFAULT, name)` also works once the OpenGL framework is in-process, i.e. after you create an NSOpenGL/CGL context.)

5. **App DB + keychain paths differ.** For the `--channel` resolver:
   - DB: `~/Library/Application Support/proscenium/proscenium.db` (Windows used `%APPDATA%\proscenium`).
   - Keychain: **unchanged** — service `Proscenium`, account `provider:{provider_id}` (keyring's `apple-native` backend, already enabled in `Cargo.toml`). Compose `{base}/live/{user}/{password}/{id}.{ext}`, never log the password.

6. **Cocoa threading:** an `NSWindow` must be created on the **main thread**. A `CGL` (headless) context has no such requirement. This is why Tier 1 below is headless — it skips the window/main-thread dance entirely for the fastest go/no-go.

---

## 2. What to reuse vs. what to write

**Reuse verbatim from `src-tauri/examples/render_api_spike.rs`** (all OS-independent — they're just libmpv ABI bindings):

- The render-API constants & structs: `MPV_RENDER_PARAM_*`, `MPV_RENDER_UPDATE_FRAME`, `MpvRenderParam`, `MpvOpenglInitParams`, `MpvOpenglFbo`.
- The `Mpv` struct, its `load()` symbol-binding, `unsafe impl Send/Sync`, and `err()`.
- `acquire_url()` and `resolve_channel()` — **change only the DB path** (`%APPDATA%` → `~/Library/Application Support`, e.g. via `dirs`/`std::env::var("HOME")`). The keychain call is identical.

**Write fresh (the macOS-specific 20%):**

- `open_libmpv()` → search the `.dylib` paths from §1.3.
- A GL context (CGL for Tier 1, NSOpenGL for Tier 2) + a `get_proc_address` backed by `dlsym` (§1.4).
- The render loop (same shape as Windows: render on `MPV_RENDER_UPDATE_FRAME` into FBO 0, present, `report_swap`).

**Recommended file:** make a **new** example `src-tauri/examples/render_api_probe_macos.rs` (leave the Windows spike pristine). Gate it `#[cfg(target_os = "macos")]` with a stub `main` otherwise. `objc2` is already a macOS dep (`Cargo.toml`), so NSOpenGL is available without adding crates; CGL/`dlopen`/`dlsym` come from `libc`/`std` FFI.

---

## 3. Tier 1 — headless CGL go/no-go (do this first)

Fastest path to the answer; no window, no main-thread constraints.

1. Create a CGL context:
   - `CGLChoosePixelFormat` with attributes `[kCGLPFAAccelerated, kCGLPFADoubleBuffer, kCGLPFAOpenGLProfile, kCGLOGLPVersion_3_2_Core, 0]`.
   - `CGLCreateContext` → `CGLSetCurrentContext`.
   - (FFI: link `OpenGL.framework`; the `CGL*` symbols live there.)
2. Build the `get_proc_address` closure → `dlsym` (§1.4).
3. `mpv_create` → set options: `vo=libmpv`, `hwdec=auto-safe`, `terminal=yes`, `msg-level=all=v` (verbose, like the Windows spike).
4. `mpv_render_context_create` with `MPV_RENDER_PARAM_API_TYPE = "opengl"` + `MPV_RENDER_PARAM_OPENGL_INIT_PARAMS` (your `get_proc_address`). **This return code is the answer:**
   - `0` → the build supports the GL render API → **PASS path**, continue.
   - `MPV_ERROR_NOT_IMPLEMENTED` (-12) → retry once with type `"sw"`; if that succeeds it's **PARTIAL**, if it also fails it's **FAIL**. Either way, stop and record.
5. If PASS: `mpv_initialize`, load the stream (`loadfile <url>`), then loop:
   - `mpv_render_context_update`; when it returns `MPV_RENDER_UPDATE_FRAME`, render into FBO 0 (an offscreen FBO or the default — for headless you can render into a texture-backed FBO and `glReadPixels` one pixel to prove it's non-black), `report_swap`, count frames.
   - Print `GL_VERSION`/`GL_RENDERER`, a rising frame count, and exit after ~8s (reuse the Windows `SPIKE_SECS` auto-quit idea).
6. Teardown **in order**: `mpv_render_context_free` → `mpv_terminate_destroy` → `CGLSetCurrentContext(null)` → `CGLDestroyContext`. Confirm no hang/crash.

**Tier 1 PASS signal** (mirrors Spike B):
```
[probe] loaded …/libmpv.2.dylib
[probe] GL_VERSION  = 4.1 Metal - <build>
[probe] render context created OK        ← the answer
[probe] 120 frames … / 240 frames …
[probe] done — N frames rendered.        ← clean teardown
```

## 4. Tier 2 — NSOpenGL window (only if Tier 1 PASSES)

Confirms the *representative* path (onscreen GL + resize), closest to the real M38 integration.

- Create an `NSWindow` + `NSOpenGLContext` on the **main thread** (`NSOpenGLPFADoubleBuffer`, `NSOpenGLPFAOpenGLProfile = NSOpenGLProfileVersion3_2Core`), via `objc2`.
- Present with `[ctx flushBuffer]` (the NSOpenGL analogue of `SwapBuffers`); default FBO is `0`.
- **Render on a dedicated thread** (make the context current there; never touch it from main) — same architecture Spike B proved is required for smooth drag-resize. Main thread runs only the Cocoa event loop.
- Manually **resize the window in several directions** and confirm: video keeps playing, no freeze, no "stuck" resize, no flicker. (This is the macOS equivalent of the Spike B §3a finding.)

If Tier 2 is smooth, the macOS half of M38 is fully de-risked.

---

## 5. Build & run

```sh
cd src-tauri
# default public HLS test stream:
cargo run --example render_api_probe_macos
# a real provider channel (DB + keychain), name-filtered:
cargo run --example render_api_probe_macos -- --channel ESPN
# headless auto-quit for lifecycle/teardown:
SPIKE_SECS=8 cargo run --example render_api_probe_macos
```

- If the runtime can't find libmpv, either put it at `src-tauri/lib/libmpv.2.dylib` or `export DYLD_LIBRARY_PATH=/opt/homebrew/opt/mpv/lib` (Homebrew). Loading by absolute path in `open_libmpv` avoids needing this.
- Toolchain: `rust-toolchain.toml` pins `stable` (host-default target), so on macOS it resolves to the Apple-Silicon/Intel triple automatically — no GNU-vs-MSVC dance like Windows.

---

## 6. When done — record the result

1. Set this doc's **Status** to PASS / PARTIAL / FAIL with the `GL_VERSION`, the `mpv_render_context_create` return code, and the frame count (paste the log, like Spike B does).
2. Update **`SPEC.md` §19 M38**: flip the macOS acceptance checkbox / risk line to reflect the finding (viable as planned, or fallback-to-Windows-only).
3. If PASS: the new `render_api_probe_macos.rs` becomes the seed for M38's macOS render layer — keep it.
4. Commit on a `poc/` or milestone branch, consistent with how Spike B was banked to `develop`.

---

## 7. Scope guardrails

- **Do not** build the WebView compositing / transparent-sandwich integration in this probe — that's M38 proper. The probe's job ends at "frames flow into a GL context we own."
- **Do not** link `proscenium_lib` into the example (same reason as Windows — and on macOS the Tauri stack pulls in the private-API/objc2 surface you don't want in a bare example). Use `sqlx` + `keyring` directly, exactly as the Windows spike's `resolve_channel` does.
- Time-box it: Tier 1 is a couple hours at most; if `mpv_render_context_create` returns 0, you basically have your answer.
