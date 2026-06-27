//! Render compositor (Milestone 37, Windows). One GL context + one render thread
//! own the video-host window; **N** mpv render contexts (one per tile/player) are
//! each rendered into their own texture-backed FBO and `glBlitFramebuffer`'d into
//! their cell of the window. Single playback is just the N=1 case (one tile that
//! fills the window), so this unifies the single- and multi-view render paths.
//!
//! The GL context is thread-affine, so *all* render-context and FBO lifecycle
//! happens on the render thread; callers drive it through a command channel.
//! `add`/`remove` block until the render thread acknowledges, which keeps
//! teardown ordered — a tile's render context is freed before its player handle
//! is destroyed (`MpvPlayer`'s drop hook calls `remove`).

use std::ffi::{c_int, c_void, CStr, CString};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::mpv::player::{
    MpvApi, MpvHandle, MpvOpenglFbo, MpvOpenglInitParams, MpvRenderCtx, MpvRenderParam,
    MPV_RENDER_PARAM_API_TYPE, MPV_RENDER_PARAM_FLIP_Y, MPV_RENDER_PARAM_INVALID,
    MPV_RENDER_PARAM_OPENGL_FBO, MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, MPV_RENDER_UPDATE_FRAME,
};
use crate::mpv::render_win::{self, GlFns};

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
    /// Create the compositor on `host_hwnd` (the video-host window). `api` is a
    /// loaded libmpv whose render-context functions are library-global (valid for
    /// any handle). Spawns the render thread and blocks until GL is initialized,
    /// so an init failure surfaces here.
    pub(crate) fn new(host_hwnd: isize, api: Arc<MpvApi>) -> Result<Self, String> {
        let (tx, rx) = channel::<Cmd>();
        let (ready_tx, ready_rx) = channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("mpv-compositor".into())
            .spawn(move || render_thread(host_hwnd, api, rx, ready_tx))
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
    host_hwnd: isize,
    api: Arc<MpvApi>,
    rx: Receiver<Cmd>,
    ready: Sender<Result<(), String>>,
) {
    let (hdc, hglrc) = match unsafe { render_win::init_gl(host_hwnd) } {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let gl = match unsafe { GlFns::load() } {
        Ok(g) => g,
        Err(e) => {
            unsafe { render_win::destroy_gl(hdc, hglrc) };
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    let mut tiles: Vec<Tile> = Vec::new();
    let mut next_id: TileId = 1;

    loop {
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
                    let result = unsafe { add_tile(&api, &gl, host_hwnd, id, handle, rect) };
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

        // --- render pass ---
        let (cw, ch) = render_win::client_size(host_hwnd);
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
            unsafe { render_win::swap_buffers(hdc) };
            for t in tiles.iter() {
                unsafe { (api.mpv_render_context_report_swap)(t.render_ctx) };
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    unsafe { render_win::destroy_gl(hdc, hglrc) };
}

/// Create a tile: an mpv render context for `handle` plus its initial FBO.
unsafe fn add_tile(
    api: &MpvApi,
    gl: &GlFns,
    host_hwnd: isize,
    id: TileId,
    handle: isize,
    rect: Option<Rect>,
) -> Result<Tile, String> {
    let render_ctx = create_render_ctx(api, handle as MpvHandle)?;
    let (iw, ih) = match rect {
        Some(r) => (r.w, r.h),
        None => render_win::client_size(host_hwnd),
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
        get_proc_address: Some(render_win::get_proc_address),
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
