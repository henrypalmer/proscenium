//! Render compositor (Milestone 37). One GL context + one render thread own the
//! video-host surface; **N** mpv render contexts (one per tile/player) are each
//! rendered into their own texture-backed FBO and `glBlitFramebuffer`'d into
//! their cell of the window. Single playback is just the N=1 case (one tile that
//! fills the window), so this unifies the single- and multi-view render paths.
//!
//! Windows and macOS share this module. The per-platform GL-surface plumbing
//! (context creation, present, drawable size, the macOS lock / resize-`-update`
//! dance) lives behind `HostSurface` in `mpv::render_win` / `mpv::render_mac`,
//! aliased here as `glsys` so the compositor body is platform-agnostic.
//!
//! The GL context is thread-affine, so *all* render-context and FBO lifecycle
//! happens on the render thread; callers drive it through a command channel.
//! `add`/`remove` block until the render thread acknowledges, which keeps
//! teardown ordered — a tile's render context is freed before its player handle
//! is destroyed (`MpvPlayer`'s drop hook calls `remove`).

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::mpv::player::{
    MpvApi, MpvHandle, MpvOpenglFbo, MpvOpenglInitParams, MpvRenderCtx, MpvRenderParam,
    MPV_RENDER_PARAM_API_TYPE, MPV_RENDER_PARAM_FLIP_Y, MPV_RENDER_PARAM_INVALID,
    MPV_RENDER_PARAM_OPENGL_FBO, MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, MPV_RENDER_UPDATE_FRAME,
};

// The per-platform GL surface (`HostSurface`) and GL proc loader
// (`get_proc_address`) behind one alias, so the compositor body is identical on
// both platforms.
#[cfg(target_os = "windows")]
use crate::mpv::render_win as glsys;
#[cfg(target_os = "macos")]
use crate::mpv::render_mac as glsys;
use glsys::HostSurface;

/// A tile's destination rectangle in window *client* coordinates (CSS top-left
/// origin, +y down) as reported by the frontend grid.
// Constructed by the multi-view layout path (Stage 3); the single-player tile
// uses `rect: None` (fill window), so this is dead until then.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub(crate) type TileId = u64;

/// Backdrop shown in gaps / before frames flow: zinc-900 (#18181b).
const BACKDROP: (f32, f32, f32, f32) = (0x18 as f32 / 255.0, 0x18 as f32 / 255.0, 0x1b as f32 / 255.0, 1.0);

enum Cmd {
    Add {
        handle: isize,
        rect: Option<Rect>,
        reply: Sender<Result<TileId, String>>,
    },
    SetRect {
        id: TileId,
        rect: Rect,
    },
    SetFill {
        id: TileId,
    },
    Remove {
        id: TileId,
        reply: Sender<()>,
    },
    Shutdown {
        reply: Sender<()>,
    },
}

/// Handle to the compositor render thread. Cheap to clone via `Arc`.
pub(crate) struct Compositor {
    tx: Sender<Cmd>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Compositor {
    /// Windows: create the compositor on `host_hwnd` (the video-host window).
    #[cfg(target_os = "windows")]
    pub(crate) fn new(host_hwnd: isize, api: Arc<MpvApi>) -> Result<Self, String> {
        Self::start(api, move || unsafe { HostSurface::create(host_hwnd) })
    }

    /// macOS: create the compositor on the `(NSOpenGLContext, NSView)` built on
    /// the main thread (`render_mac::create_gl_host`) and glued behind the app
    /// window. The render thread makes the context current.
    #[cfg(target_os = "macos")]
    pub(crate) fn new(gl_context: isize, gl_view: isize, api: Arc<MpvApi>) -> Result<Self, String> {
        Self::start(api, move || unsafe {
            HostSurface::create(gl_context, gl_view)
        })
    }

    /// Spawn the render thread with a platform `HostSurface` factory and block
    /// until GL is initialized, so an init failure surfaces here. `api` is a
    /// loaded libmpv whose render-context functions are library-global (valid for
    /// any handle).
    fn start(
        api: Arc<MpvApi>,
        make_surface: impl FnOnce() -> Result<HostSurface, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (tx, rx) = channel::<Cmd>();
        let (ready_tx, ready_rx) = channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("mpv-compositor".into())
            .spawn(move || render_thread(make_surface, api, rx, ready_tx))
            .map_err(|e| format!("failed to spawn compositor thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                tx,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err("compositor thread exited during init".into()),
        }
    }

    /// Add a tile that renders mpv `handle`. `rect = None` fills the whole window
    /// (single-player). Blocks until the render context is created (so the caller
    /// knows GL state is ready and, for teardown, that `remove` will be ordered).
    pub(crate) fn add(&self, handle: isize, rect: Option<Rect>) -> Result<TileId, String> {
        let (reply, ack) = channel();
        self.tx
            .send(Cmd::Add { handle, rect, reply })
            .map_err(|_| "compositor thread is gone".to_string())?;
        ack.recv()
            .map_err(|_| "compositor thread is gone".to_string())?
    }

    /// Update a tile's destination rectangle (frontend layout / resize).
    /// Unused until the multi-view layout stage.
    #[allow(dead_code)]
    pub(crate) fn set_rect(&self, id: TileId, rect: Rect) {
        let _ = self.tx.send(Cmd::SetRect { id, rect });
    }

    /// Revert a tile to filling the whole window (auto-tracks window resize).
    /// Used when leaving multi-view to restore the single-player tile.
    #[allow(dead_code)]
    pub(crate) fn set_fill(&self, id: TileId) {
        let _ = self.tx.send(Cmd::SetFill { id });
    }

    /// Remove a tile, freeing its render context + FBO on the render thread.
    /// Blocks until done, so the caller may then safely terminate the player
    /// handle (ordered teardown).
    pub(crate) fn remove(&self, id: TileId) {
        let (reply, ack) = channel();
        if self.tx.send(Cmd::Remove { id, reply }).is_ok() {
            let _ = ack.recv();
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let (reply, ack) = channel();
        if self.tx.send(Cmd::Shutdown { reply }).is_ok() {
            let _ = ack.recv();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// One composited tile: an mpv render context drawn into a (grow-only)
/// texture-backed FBO, blitted into `rect` (or the whole window if `None`).
struct Tile {
    id: TileId,
    render_ctx: MpvRenderCtx,
    fbo: u32,
    tex: u32,
    /// Allocated texture capacity (grown, never shrunk, to avoid per-frame churn
    /// during a drag-resize). mpv renders into the (0,0)-(draw_w,draw_h) corner.
    cap_w: i32,
    cap_h: i32,
    draw_w: i32,
    draw_h: i32,
    rect: Option<Rect>,
    has_content: bool,
}

/// Round a dimension up to the next multiple of 256, so a drag-resize regrows the
/// FBO only occasionally rather than every frame.
fn round_up(v: i32) -> i32 {
    ((v.max(1) + 255) / 256) * 256
}

fn render_thread(
    make_surface: impl FnOnce() -> Result<HostSurface, String>,
    api: Arc<MpvApi>,
    rx: Receiver<Cmd>,
    ready: Sender<Result<(), String>>,
) {
    let mut surface = match make_surface() {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let gl = match unsafe { GlFns::load(glsys::get_proc_address) } {
        Ok(g) => g,
        Err(e) => {
            unsafe { surface.destroy() };
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    let mut tiles: Vec<Tile> = Vec::new();
    let mut next_id: TileId = 1;

    loop {
        // Drawable size for this frame; on macOS this also fires the resize
        // `-update` when the host view changed size.
        let (cw, ch) = surface.begin_frame();

        // --- process pending commands (GL-thread-affine work) ---
        let mut shutdown = false;
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Cmd::Add {
                    handle,
                    rect,
                    reply,
                } => {
                    let id = next_id;
                    let result = unsafe { add_tile(&api, &gl, cw, ch, id, handle, rect) };
                    match result {
                        Ok(tile) => {
                            tiles.push(tile);
                            next_id += 1;
                            let _ = reply.send(Ok(id));
                        }
                        Err(e) => {
                            let _ = reply.send(Err(e));
                        }
                    }
                }
                Cmd::SetRect { id, rect } => {
                    if let Some(t) = tiles.iter_mut().find(|t| t.id == id) {
                        t.rect = Some(rect);
                    }
                }
                Cmd::SetFill { id } => {
                    if let Some(t) = tiles.iter_mut().find(|t| t.id == id) {
                        t.rect = None;
                    }
                }
                Cmd::Remove { id, reply } => {
                    if let Some(pos) = tiles.iter().position(|t| t.id == id) {
                        let t = tiles.remove(pos);
                        unsafe {
                            (api.mpv_render_context_free)(t.render_ctx);
                            gl.delete_fbo(t.fbo, t.tex);
                        }
                    }
                    let _ = reply.send(());
                }
                Cmd::Shutdown { reply } => {
                    for t in tiles.drain(..) {
                        unsafe {
                            (api.mpv_render_context_free)(t.render_ctx);
                            gl.delete_fbo(t.fbo, t.tex);
                        }
                    }
                    shutdown = true;
                    let _ = reply.send(());
                }
            }
        }
        if shutdown {
            break;
        }

        // --- render pass (offscreen: each tile into its own FBO) ---
        let mut any_new = false;

        for t in tiles.iter_mut() {
            let (rw, rh) = match t.rect {
                Some(r) => (r.w.max(1), r.h.max(1)),
                None => (cw, ch),
            };
            let mut force = false;
            // Grow the FBO if the tile got bigger than its texture capacity.
            if rw > t.cap_w || rh > t.cap_h {
                let nw = round_up(rw.max(t.cap_w));
                let nh = round_up(rh.max(t.cap_h));
                match unsafe { gl.create_fbo(nw, nh) } {
                    Ok((fbo, tex)) => {
                        unsafe { gl.delete_fbo(t.fbo, t.tex) };
                        t.fbo = fbo;
                        t.tex = tex;
                        t.cap_w = nw;
                        t.cap_h = nh;
                        force = true; // new texture is empty — render into it now
                    }
                    Err(e) => eprintln!("[compositor] FBO grow failed: {e}"),
                }
            }
            t.draw_w = rw;
            t.draw_h = rh;

            let flags = unsafe { (api.mpv_render_context_update)(t.render_ctx) };
            if force || flags & MPV_RENDER_UPDATE_FRAME != 0 {
                unsafe { render_into_fbo(&api, t.render_ctx, t.fbo, rw, rh) };
                t.has_content = true;
                any_new = true;
            }
        }

        if any_new {
            // The composite + present touch the default framebuffer (the
            // drawable); lock so a concurrent main-thread `-update` can't
            // reconfigure it mid-frame (macOS; no-op on Windows).
            unsafe { surface.lock() };
            unsafe { gl.begin_window_frame(cw, ch, BACKDROP) };
            for t in tiles.iter() {
                if !t.has_content {
                    continue;
                }
                let (dx, dy, dw, dh) = match t.rect {
                    Some(r) => (r.x, r.y, r.w.max(1), r.h.max(1)),
                    None => (0, 0, cw, ch),
                };
                // CSS top-left rect -> GL bottom-left destination.
                let dx0 = dx;
                let dx1 = dx + dw;
                let dy0 = ch - (dy + dh);
                let dy1 = ch - dy;
                unsafe { gl.blit_to_window(t.fbo, t.draw_w, t.draw_h, dx0, dy0, dx1, dy1) };
            }
            unsafe { surface.present() };
            unsafe { surface.unlock() };
            for t in tiles.iter() {
                unsafe { (api.mpv_render_context_report_swap)(t.render_ctx) };
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    unsafe { surface.destroy() };
}

/// Create a tile: an mpv render context for `handle` plus its initial FBO.
/// `(fill_w, fill_h)` is the current drawable size, used to size a fill tile
/// (`rect == None`).
unsafe fn add_tile(
    api: &MpvApi,
    gl: &GlFns,
    fill_w: i32,
    fill_h: i32,
    id: TileId,
    handle: isize,
    rect: Option<Rect>,
) -> Result<Tile, String> {
    let render_ctx = create_render_ctx(api, handle as MpvHandle)?;
    let (iw, ih) = match rect {
        Some(r) => (r.w, r.h),
        None => (fill_w, fill_h),
    };
    let cap_w = round_up(iw);
    let cap_h = round_up(ih);
    let (fbo, tex) = match gl.create_fbo(cap_w, cap_h) {
        Ok(v) => v,
        Err(e) => {
            (api.mpv_render_context_free)(render_ctx);
            return Err(e);
        }
    };
    Ok(Tile {
        id,
        render_ctx,
        fbo,
        tex,
        cap_w,
        cap_h,
        draw_w: iw.max(1),
        draw_h: ih.max(1),
        rect,
        has_content: false,
    })
}

unsafe fn create_render_ctx(api: &MpvApi, handle: MpvHandle) -> Result<MpvRenderCtx, String> {
    let mut ctx: MpvRenderCtx = std::ptr::null_mut();
    let api_type = CString::new("opengl").unwrap();
    let mut gl_init = MpvOpenglInitParams {
        get_proc_address: Some(glsys::get_proc_address),
        get_proc_address_ctx: std::ptr::null_mut(),
    };
    let mut params = [
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_API_TYPE,
            data: api_type.as_ptr() as *mut c_void,
        },
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
            data: &mut gl_init as *mut _ as *mut c_void,
        },
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        },
    ];
    let rc = (api.mpv_render_context_create)(&mut ctx, handle, params.as_mut_ptr());
    if rc < 0 {
        let msg = CStr::from_ptr((api.mpv_error_string)(rc)).to_string_lossy();
        return Err(format!("mpv_render_context_create failed: {msg}"));
    }
    Ok(ctx)
}

/// Render the current mpv frame into the `(0,0)-(w,h)` corner of `fbo`.
unsafe fn render_into_fbo(api: &MpvApi, ctx: MpvRenderCtx, fbo: u32, w: i32, h: i32) {
    let mut mfbo = MpvOpenglFbo {
        fbo: fbo as c_int,
        w,
        h,
        internal_format: 0,
    };
    let mut flip: c_int = 1;
    let mut params = [
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_OPENGL_FBO,
            data: &mut mfbo as *mut _ as *mut c_void,
        },
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_FLIP_Y,
            data: &mut flip as *mut _ as *mut c_void,
        },
        MpvRenderParam {
            type_: MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        },
    ];
    (api.mpv_render_context_render)(ctx, params.as_mut_ptr());
}

// --- OpenGL function table for the compositor (Milestone 37) ---
//
// Each mpv tile renders into its own texture-backed FBO; the compositor then
// `glBlitFramebuffer`s each into its sub-rectangle of the window's default
// framebuffer. These are the GL functions that needs. They are resolved through
// the platform `get_proc_address` (WGL + the opengl32 fallback on Windows;
// `dlsym` from the OpenGL framework on macOS). GL uses the `system` calling
// convention (APIENTRY/stdcall on Windows; identical to C on macOS).

const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_RGBA: u32 = 0x1908;
const GL_RGBA8: u32 = 0x8058;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_READ_FRAMEBUFFER: u32 = 0x8CA8;
const GL_DRAW_FRAMEBUFFER: u32 = 0x8CA9;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_LINEAR: i32 = 0x2601;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_SCISSOR_TEST: u32 = 0x0C11;

type GenFn = unsafe extern "system" fn(i32, *mut u32);
type DelFn = unsafe extern "system" fn(i32, *const u32);
type BindFn = unsafe extern "system" fn(u32, u32);

/// The compositor's OpenGL function table (resolved once per GL context).
#[allow(dead_code)]
struct GlFns {
    gen_framebuffers: GenFn,
    delete_framebuffers: DelFn,
    bind_framebuffer: BindFn,
    gen_textures: GenFn,
    delete_textures: DelFn,
    bind_texture: BindFn,
    tex_image_2d: unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void),
    tex_parameteri: unsafe extern "system" fn(u32, u32, i32),
    framebuffer_texture_2d: unsafe extern "system" fn(u32, u32, u32, u32, i32),
    check_framebuffer_status: unsafe extern "system" fn(u32) -> u32,
    blit_framebuffer: unsafe extern "system" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32),
    clear: unsafe extern "system" fn(u32),
    clear_color: unsafe extern "system" fn(f32, f32, f32, f32),
    viewport: unsafe extern "system" fn(i32, i32, i32, i32),
    disable: unsafe extern "system" fn(u32),
}

impl GlFns {
    /// Resolve the entry points via the platform `get_proc` loader. A GL context
    /// must be current on this thread.
    unsafe fn load(
        get_proc: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    ) -> Result<Self, String> {
        macro_rules! sym {
            ($n:literal) => {{
                let p = get_proc(
                    std::ptr::null_mut(),
                    concat!($n, "\0").as_ptr() as *const c_char,
                );
                if p.is_null() {
                    return Err(format!("GL entry point not found: {}", $n));
                }
                std::mem::transmute(p)
            }};
        }
        Ok(Self {
            gen_framebuffers: sym!("glGenFramebuffers"),
            delete_framebuffers: sym!("glDeleteFramebuffers"),
            bind_framebuffer: sym!("glBindFramebuffer"),
            gen_textures: sym!("glGenTextures"),
            delete_textures: sym!("glDeleteTextures"),
            bind_texture: sym!("glBindTexture"),
            tex_image_2d: sym!("glTexImage2D"),
            tex_parameteri: sym!("glTexParameteri"),
            framebuffer_texture_2d: sym!("glFramebufferTexture2D"),
            check_framebuffer_status: sym!("glCheckFramebufferStatus"),
            blit_framebuffer: sym!("glBlitFramebuffer"),
            clear: sym!("glClear"),
            clear_color: sym!("glClearColor"),
            viewport: sym!("glViewport"),
            disable: sym!("glDisable"),
        })
    }

    /// Create a texture-backed FBO of size `w`×`h`. Returns `(fbo, texture)`.
    unsafe fn create_fbo(&self, w: i32, h: i32) -> Result<(u32, u32), String> {
        let (w, h) = (w.max(1), h.max(1));
        let mut tex: u32 = 0;
        (self.gen_textures)(1, &mut tex);
        (self.bind_texture)(GL_TEXTURE_2D, tex);
        (self.tex_image_2d)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as i32,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        (self.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (self.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        let mut fbo: u32 = 0;
        (self.gen_framebuffers)(1, &mut fbo);
        (self.bind_framebuffer)(GL_FRAMEBUFFER, fbo);
        (self.framebuffer_texture_2d)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
        let status = (self.check_framebuffer_status)(GL_FRAMEBUFFER);
        (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
        if status != GL_FRAMEBUFFER_COMPLETE {
            (self.delete_framebuffers)(1, &fbo);
            (self.delete_textures)(1, &tex);
            return Err(format!("framebuffer incomplete: 0x{status:X}"));
        }
        Ok((fbo, tex))
    }

    unsafe fn delete_fbo(&self, fbo: u32, tex: u32) {
        (self.delete_framebuffers)(1, &fbo);
        (self.delete_textures)(1, &tex);
    }

    /// Bind the default framebuffer, reset viewport/scissor, and clear it to
    /// `rgba` (the backdrop shown in gaps between tiles).
    unsafe fn begin_window_frame(&self, cw: i32, ch: i32, rgba: (f32, f32, f32, f32)) {
        (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
        (self.disable)(GL_SCISSOR_TEST);
        (self.viewport)(0, 0, cw.max(1), ch.max(1));
        (self.clear_color)(rgba.0, rgba.1, rgba.2, rgba.3);
        (self.clear)(GL_COLOR_BUFFER_BIT);
    }

    /// Blit tile FBO `src_fbo` (its full `sw`×`sh`) into the default
    /// framebuffer's destination rect (GL bottom-left coordinates).
    unsafe fn blit_to_window(
        &self,
        src_fbo: u32,
        sw: i32,
        sh: i32,
        dx0: i32,
        dy0: i32,
        dx1: i32,
        dy1: i32,
    ) {
        (self.bind_framebuffer)(GL_READ_FRAMEBUFFER, src_fbo);
        (self.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, 0);
        (self.blit_framebuffer)(
            0,
            0,
            sw,
            sh,
            dx0,
            dy0,
            dx1,
            dy1,
            GL_COLOR_BUFFER_BIT,
            GL_LINEAR as u32,
        );
    }
}
