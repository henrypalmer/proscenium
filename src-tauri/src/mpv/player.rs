//! libmpv wrapper. The library is loaded at runtime with `libloading`
//! (dynamic linking, per the LGPL compliance note in spec §3) and driven
//! through the C client API. A background thread pumps the mpv event loop,
//! maintains an [`MpvState`] snapshot, and notifies the embedder via a
//! callback (the Tauri layer forwards these as `mpv:state_changed` events).

use crate::models::{MpvState, TrackInfo};
use libloading::Library;
use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// --- C API surface (client.h) ---

const MPV_FORMAT_STRING: c_int = 1;
const MPV_FORMAT_FLAG: c_int = 3;
const MPV_FORMAT_DOUBLE: c_int = 5;

const MPV_EVENT_SHUTDOWN: c_int = 1;
const MPV_EVENT_END_FILE: c_int = 7;
const MPV_EVENT_FILE_LOADED: c_int = 8;
const MPV_EVENT_PROPERTY_CHANGE: c_int = 22;

const MPV_END_FILE_REASON_ERROR: c_int = 4;

// pub(crate): appears in the (pub(crate)) `MpvApi::mpv_wait_event` signature.
#[repr(C)]
pub(crate) struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct MpvEventProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

/// Prefix of mpv_event_end_file; later fields (playlist ids) are not read.
#[repr(C)]
struct MpvEventEndFile {
    reason: c_int,
    error: c_int,
}

pub(crate) type MpvHandle = *mut c_void;

// --- render API surface (render.h / render_gl.h), Milestone 38 + 37 ---
//
// Instead of `--wid` window embedding, mpv renders each frame into a GPU
// surface *we* own (an OpenGL framebuffer). These ABI bindings are shared:
// the macOS player render thread (M38) uses them directly, and the Windows
// `mpv::compositor` (M37) uses them to drive N render contexts into one
// surface — hence `pub(crate)`. `allow(dead_code)` keeps any platform that
// uses only a subset warning-free.
pub(crate) type MpvRenderCtx = *mut c_void;

#[allow(dead_code)]
pub(crate) const MPV_RENDER_PARAM_INVALID: c_int = 0;
#[allow(dead_code)]
pub(crate) const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
#[allow(dead_code)]
pub(crate) const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
#[allow(dead_code)]
pub(crate) const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
#[allow(dead_code)]
pub(crate) const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
/// Bit returned by mpv_render_context_update() meaning "a new frame is ready".
#[allow(dead_code)]
pub(crate) const MPV_RENDER_UPDATE_FRAME: u64 = 1;

#[repr(C)]
#[allow(dead_code)]
pub(crate) struct MpvRenderParam {
    pub(crate) type_: c_int,
    pub(crate) data: *mut c_void,
}

#[repr(C)]
#[allow(dead_code)]
pub(crate) struct MpvOpenglInitParams {
    pub(crate) get_proc_address:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    pub(crate) get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
#[allow(dead_code)]
pub(crate) struct MpvOpenglFbo {
    pub(crate) fbo: c_int,
    pub(crate) w: c_int,
    pub(crate) h: c_int,
    pub(crate) internal_format: c_int,
}

macro_rules! mpv_api {
    ($($name:ident: fn($($arg:ty),*) -> $ret:ty),+ $(,)?) => {
        // Render-API fn pointers are unused on macOS until its render slice
        // lands (Milestone 38 is Windows-first), so allow dead fields there.
        #[allow(dead_code)]
        pub(crate) struct MpvApi {
            _lib: Library,
            $(pub(crate) $name: unsafe extern "C" fn($($arg),*) -> $ret,)+
        }
        impl MpvApi {
            fn load() -> Result<Self, String> {
                let lib = open_libmpv()?;
                unsafe {
                    Ok(Self {
                        $($name: *lib
                            .get(concat!(stringify!($name), "\0").as_bytes())
                            .map_err(|e| format!("libmpv is missing {}: {e}", stringify!($name)))?,)+
                        _lib: lib,
                    })
                }
            }
        }
    };
}

mpv_api! {
    mpv_create: fn() -> MpvHandle,
    mpv_initialize: fn(MpvHandle) -> c_int,
    mpv_terminate_destroy: fn(MpvHandle) -> c_void,
    mpv_set_option_string: fn(MpvHandle, *const c_char, *const c_char) -> c_int,
    mpv_set_property_string: fn(MpvHandle, *const c_char, *const c_char) -> c_int,
    mpv_command: fn(MpvHandle, *mut *const c_char) -> c_int,
    mpv_observe_property: fn(MpvHandle, u64, *const c_char, c_int) -> c_int,
    mpv_wait_event: fn(MpvHandle, c_double) -> *mut MpvEvent,
    mpv_wakeup: fn(MpvHandle) -> c_void,
    mpv_error_string: fn(c_int) -> *const c_char,
    // Render API (Milestone 38). Present in every libmpv build.
    mpv_render_context_create: fn(*mut MpvRenderCtx, MpvHandle, *mut MpvRenderParam) -> c_int,
    mpv_render_context_render: fn(MpvRenderCtx, *mut MpvRenderParam) -> c_int,
    mpv_render_context_update: fn(MpvRenderCtx) -> u64,
    mpv_render_context_report_swap: fn(MpvRenderCtx) -> c_void,
    mpv_render_context_free: fn(MpvRenderCtx) -> c_void,
}

fn open_libmpv() -> Result<Library, String> {
    let names: &[&str] = if cfg!(target_os = "windows") {
        &["libmpv-2.dll", "mpv-2.dll", "libmpv.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib"]
    } else {
        &["libmpv.so.2", "libmpv.so"]
    };
    // Candidate directories, most specific first: next to the executable
    // (Windows installer layout), and — for a macOS `.app` — the sibling
    // `Contents/Frameworks` where the bundler embeds dylibs declared in
    // `bundle.macOS.frameworks`.
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            #[cfg(target_os = "macos")]
            dirs.push(dir.join("../Frameworks"));
        }
    }
    for name in names {
        for dir in &dirs {
            if let Ok(lib) = unsafe { Library::new(dir.join(name)) } {
                return Ok(lib);
            }
        }
        // Fall back to the loader's default search path (PATH on Windows,
        // rpath/DYLD on macOS, ld.so on Linux).
        if let Ok(lib) = unsafe { Library::new(name) } {
            return Ok(lib);
        }
    }
    Err(
        "Could not load libmpv (libmpv-2.dll). Place it next to the application executable."
            .to_string(),
    )
}

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// The mpv client API is thread-safe; the raw handle may cross threads.
struct Handle(MpvHandle);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

pub struct MpvConfig {
    /// Set (to the host window HWND) when this is a composited video player on
    /// Windows — it flips the player to `vo=libmpv` so the shared
    /// `mpv::compositor` (Milestone 37) can render its handle. It is *not* passed
    /// to mpv as `--wid`. None = headless / tests (macOS uses `gl_host` instead).
    pub wid: Option<isize>,
    /// macOS render-API host (Milestone 38): `(NSOpenGLContext, NSView)` pointers
    /// as `isize`. The render thread makes the context current and draws mpv's
    /// frames into it. None = headless / tests.
    #[cfg(target_os = "macos")]
    pub gl_host: Option<(isize, isize)>,
    pub hwdec: bool,
    /// Null video/audio outputs for tests.
    pub headless: bool,
}

pub type StateCallback = Box<dyn Fn(MpvState) + Send + Sync + 'static>;

pub struct MpvPlayer {
    api: Arc<MpvApi>,
    handle: Arc<Handle>,
    state: Arc<Mutex<MpvState>>,
    shutdown: Arc<AtomicBool>,
    /// Position (seconds) to seek to once the next file finishes loading, set
    /// by `load_url` for resume playback (spec §5.9). Applied on FILE_LOADED so
    /// playback starts at the resume point with no visible jump from 0.
    pending_seek: Arc<Mutex<Option<f64>>>,
    event_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// macOS render-API host (Milestone 38): the dedicated render thread plus its
    /// stop flag. Its render context must be freed before the player handle is
    /// destroyed, which `Drop` enforces by joining this thread first. On Windows
    /// rendering is owned by `mpv::compositor` (Milestone 37), not the player.
    #[cfg(target_os = "macos")]
    render: Mutex<Option<RenderHost>>,
    /// Windows ordered-teardown hook (Milestone 37): runs in `Drop` *before* the
    /// mpv handle is terminated. `commands/playback.rs` sets it to remove this
    /// player's compositor tile, freeing its render context in the right order.
    #[cfg(target_os = "windows")]
    pre_terminate: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

/// Owns the macOS render thread (and the signal to stop it). The thread itself
/// owns the GL + mpv render context.
#[cfg(target_os = "macos")]
struct RenderHost {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MpvPlayer {
    pub fn new(config: MpvConfig, on_state: StateCallback) -> Result<Arc<Self>, String> {
        let api = Arc::new(MpvApi::load()?);
        let handle = unsafe { (api.mpv_create)() };
        if handle.is_null() {
            return Err("mpv_create failed".into());
        }

        let set = |name: &str, value: &str| -> Result<(), String> {
            let rc = unsafe {
                (api.mpv_set_option_string)(handle, cstr(name).as_ptr(), cstr(value).as_ptr())
            };
            if rc < 0 {
                Err(format!("failed to set mpv option {name}={value} ({rc})"))
            } else {
                Ok(())
            }
        };

        // Diagnostic log in the temp dir; invaluable for vo/ao issues.
        let log_path = std::env::temp_dir().join("proscenium-mpv.log");
        set("log-file", &log_path.to_string_lossy())?;
        set("msg-level", "all=warn")?;

        // We render our own controls; mpv must not handle input or draw UI.
        set("input-default-bindings", "no")?;
        set("input-vo-keyboard", "no")?;
        set("osc", "no")?;
        set("osd-level", "0")?;
        set("terminal", "no")?;
        set("keep-open", "yes")?; // VOD end: hold last frame instead of going idle
        set("cache", "yes")?;
        // Subtitles default OFF (spec §5.6, Milestone 22): don't auto-select an
        // embedded or sidecar subtitle track on load — the user opts in from the
        // track menu. `sub-visibility=yes` ensures a track they *do* select is
        // actually rendered. Runtime `sid` changes (set_subtitle_track) override.
        set("sub-auto", "no")?;
        set("sid", "no")?;
        set("sub-visibility", "yes")?;
        // Hardware decode by default (spec §11); silent software fallback —
        // including the Dolby Vision chain — is handled inside libmpv.
        set("hwdec", if config.hwdec { "auto-safe" } else { "no" })?;
        // Windows (Milestone 38/37): render via the libmpv render API into a GPU
        // surface we own, not `--wid`. `vo=libmpv` makes mpv hand frames to the
        // render context the shared `mpv::compositor` creates for this player.
        #[cfg(target_os = "windows")]
        if !config.headless && config.wid.is_some() {
            set("vo", "libmpv")?;
        }
        // macOS (Milestone 38): render via the libmpv render API into the
        // NSOpenGLContext we own (created on the main thread and glued behind the
        // app window — see `mpv::render_mac`), driven by the render thread spawned
        // below. `vo=libmpv` makes mpv hand frames to our render context instead
        // of opening its own window.
        #[cfg(target_os = "macos")]
        if !config.headless && config.gl_host.is_some() {
            set("vo", "libmpv")?;
        }
        if config.headless {
            set("vo", "null")?;
            set("ao", "null")?;
            set("force-window", "no")?;
            // No vo interop in headless mode; copy-back keeps hwdec testable.
            if config.hwdec {
                set("hwdec", "auto-copy")?;
            }
        }

        let rc = unsafe { (api.mpv_initialize)(handle) };
        if rc < 0 {
            unsafe { (api.mpv_terminate_destroy)(handle) };
            return Err(format!("mpv_initialize failed ({rc})"));
        }

        for (name, format) in [
            ("pause", MPV_FORMAT_FLAG),
            ("idle-active", MPV_FORMAT_FLAG),
            ("mute", MPV_FORMAT_FLAG),
            ("paused-for-cache", MPV_FORMAT_FLAG),
            ("eof-reached", MPV_FORMAT_FLAG),
            ("time-pos", MPV_FORMAT_DOUBLE),
            ("duration", MPV_FORMAT_DOUBLE),
            ("volume", MPV_FORMAT_DOUBLE),
            ("track-list", MPV_FORMAT_STRING),
            ("aid", MPV_FORMAT_STRING),
            ("sid", MPV_FORMAT_STRING),
            ("hwdec-current", MPV_FORMAT_STRING),
        ] {
            unsafe {
                (api.mpv_observe_property)(handle, 0, cstr(name).as_ptr(), format);
            }
        }

        let player = Arc::new(Self {
            api: api.clone(),
            handle: Arc::new(Handle(handle)),
            state: Arc::new(Mutex::new(MpvState::default())),
            shutdown: Arc::new(AtomicBool::new(false)),
            pending_seek: Arc::new(Mutex::new(None)),
            event_thread: Mutex::new(None),
            #[cfg(target_os = "macos")]
            render: Mutex::new(None),
            #[cfg(target_os = "windows")]
            pre_terminate: Mutex::new(None),
        });

        let thread = {
            let api = api.clone();
            let handle = player.handle.clone();
            let state = player.state.clone();
            let shutdown = player.shutdown.clone();
            let pending_seek = player.pending_seek.clone();
            std::thread::Builder::new()
                .name("mpv-events".into())
                .spawn(move || event_loop(api, handle, state, shutdown, pending_seek, on_state))
                .map_err(|e| format!("failed to spawn mpv event thread: {e}"))?
        };
        *player.event_thread.lock().unwrap() = Some(thread);

        // Windows (Milestone 37): the player does NOT own a render thread — the
        // shared `mpv::compositor` creates a render context for this player's
        // handle and composites it (and any other tiles) into the host window.
        // `commands/playback.rs` registers the player with the compositor after
        // construction (and unregisters it before drop, for ordered teardown).

        // macOS render-API (Milestone 38): same dedicated-render-thread model, but
        // the GL context is the NSOpenGLContext created on the main thread and
        // handed in via `gl_host`. Made current on this thread (proven OK by the
        // probe), it renders into the context's drawable.
        #[cfg(target_os = "macos")]
        if !config.headless {
            if let Some((gl_context, gl_view)) = config.gl_host {
                let stop = Arc::new(AtomicBool::new(false));
                let api = api.clone();
                let handle = player.handle.clone();
                let stop_rt = stop.clone();
                let thread = std::thread::Builder::new()
                    .name("mpv-render".into())
                    .spawn(move || render_thread_mac(api, handle, gl_context, gl_view, stop_rt))
                    .map_err(|e| format!("failed to spawn mpv render thread: {e}"))?;
                *player.render.lock().unwrap() = Some(RenderHost {
                    stop,
                    thread: Some(thread),
                });
            }
        }

        Ok(player)
    }

    fn command(&self, args: &[&str]) -> Result<(), String> {
        let cstrings: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
        let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        let rc = unsafe { (self.api.mpv_command)(self.handle.0, ptrs.as_mut_ptr()) };
        if rc < 0 {
            let msg = unsafe { CStr::from_ptr((self.api.mpv_error_string)(rc)) };
            Err(format!(
                "mpv command {:?} failed: {}",
                args[0],
                msg.to_string_lossy()
            ))
        } else {
            Ok(())
        }
    }

    fn set_property(&self, name: &str, value: &str) -> Result<(), String> {
        let rc = unsafe {
            (self.api.mpv_set_property_string)(
                self.handle.0,
                cstr(name).as_ptr(),
                cstr(value).as_ptr(),
            )
        };
        if rc < 0 {
            let msg = unsafe { CStr::from_ptr((self.api.mpv_error_string)(rc)) };
            Err(format!("failed to set {name}: {}", msg.to_string_lossy()))
        } else {
            Ok(())
        }
    }

    /// Load `url`. When `start` is given (spec §5.9 resume), playback seeks to
    /// that position once the file finishes loading.
    pub fn load_url(&self, url: &str, start: Option<f64>) -> Result<(), String> {
        {
            let mut state = self.state.lock().unwrap();
            state.error = None;
            state.buffering = true;
        }
        *self.pending_seek.lock().unwrap() = start.filter(|s| *s > 0.0);
        self.command(&["loadfile", url])?;
        self.set_property("pause", "no")
    }

    pub fn play(&self) -> Result<(), String> {
        self.set_property("pause", "no")
    }

    pub fn pause(&self) -> Result<(), String> {
        self.set_property("pause", "yes")
    }

    pub fn stop(&self) -> Result<(), String> {
        self.command(&["stop"])
    }

    /// Absolute seek in seconds.
    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        self.command(&["seek", &format!("{seconds:.3}"), "absolute"])
    }

    /// 0–100.
    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.set_property("volume", &format!("{:.0}", volume.clamp(0.0, 100.0)))
    }

    pub fn set_mute(&self, muted: bool) -> Result<(), String> {
        self.set_property("mute", if muted { "yes" } else { "no" })
    }

    /// Set the hardware-decode mode (spec §11 / Settings "Hardware decode"). mpv
    /// applies `hwdec` when the next file's decoder initializes, so setting it
    /// just before `load_url` makes the Settings toggle take effect on the next
    /// stream even though the player instance is reused across streams.
    pub fn set_hwdec(&self, enabled: bool) -> Result<(), String> {
        self.set_property("hwdec", if enabled { "auto-safe" } else { "no" })
    }

    /// Track ids come from the track list; a negative id disables the track.
    pub fn set_audio_track(&self, track_id: i64) -> Result<(), String> {
        let value = if track_id < 0 { "no".into() } else { track_id.to_string() };
        self.set_property("aid", &value)
    }

    pub fn set_subtitle_track(&self, track_id: i64) -> Result<(), String> {
        let value = if track_id < 0 { "no".into() } else { track_id.to_string() };
        self.set_property("sid", &value)
    }

    pub fn get_state(&self) -> MpvState {
        self.state.lock().unwrap().clone()
    }

    /// The raw mpv handle as an `isize`, for the Windows compositor to create a
    /// render context against (Milestone 37). The handle is thread-safe; the
    /// compositor only ever calls render-API functions with it, never frees it.
    #[cfg(target_os = "windows")]
    pub(crate) fn raw_handle(&self) -> isize {
        self.handle.0 as isize
    }

    /// A clone of the loaded libmpv API, so the compositor can call the
    /// render-context functions (they are library-global — valid for any handle).
    #[cfg(target_os = "windows")]
    pub(crate) fn api(&self) -> Arc<MpvApi> {
        self.api.clone()
    }

    /// Register a hook to run in `Drop` *before* the mpv handle is terminated
    /// (Milestone 37): used to free this player's compositor tile in order.
    #[cfg(target_os = "windows")]
    pub(crate) fn set_pre_terminate(&self, f: Box<dyn FnOnce() + Send>) {
        *self.pre_terminate.lock().unwrap() = Some(f);
    }

    /// Capture the current video frame to `path` as a PNG. `subtitles` keeps
    /// the rendered subtitle overlay; `false` grabs the clean video. Used by
    /// the macOS playback verification harness to prove the embedded video
    /// output actually rendered frames.
    pub fn screenshot_to_file(&self, path: &str, subtitles: bool) -> Result<(), String> {
        let mode = if subtitles { "subtitles" } else { "video" };
        self.command(&["screenshot-to-file", path, mode])
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        // macOS: stop the render thread first — it must free its mpv render
        // context *before* the player handle is terminated below (mpv requires
        // this order; the binding can't enforce it). On Windows the compositor
        // owns the render context and `commands/playback.rs` removes this
        // player's tile (freeing it) before dropping the player.
        #[cfg(target_os = "macos")]
        if let Some(mut host) = self.render.lock().unwrap().take() {
            host.stop.store(true, Ordering::SeqCst);
            if let Some(thread) = host.thread.take() {
                let _ = thread.join();
            }
        }
        // Windows: free this player's compositor tile (its render context) before
        // terminating the handle — the ordered-teardown rule the compositor needs.
        #[cfg(target_os = "windows")]
        if let Some(f) = self.pre_terminate.lock().unwrap().take() {
            f();
        }
        self.shutdown.store(true, Ordering::SeqCst);
        unsafe { (self.api.mpv_wakeup)(self.handle.0) };
        if let Some(thread) = self.event_thread.lock().unwrap().take() {
            let _ = thread.join();
        }
        // The event thread has exited; nobody else touches the handle now.
        unsafe { (self.api.mpv_terminate_destroy)(self.handle.0) };
    }
}

/// Fire-and-forget mpv command from the event thread (no error reporting path).
fn raw_command(api: &MpvApi, handle: &Handle, args: &[&str]) {
    let cstrings: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
    let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe {
        (api.mpv_command)(handle.0, ptrs.as_mut_ptr());
    }
}

fn event_loop(
    api: Arc<MpvApi>,
    handle: Arc<Handle>,
    state: Arc<Mutex<MpvState>>,
    shutdown: Arc<AtomicBool>,
    pending_seek: Arc<Mutex<Option<f64>>>,
    on_state: StateCallback,
) {
    let mut last_emit = std::time::Instant::now();
    let mut pending_emit = false;
    let mut significant = false;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let event = unsafe { &*(api.mpv_wait_event)(handle.0, 0.25) };
        match event.event_id {
            0 => {} // MPV_EVENT_NONE: timeout
            MPV_EVENT_SHUTDOWN => break,
            MPV_EVENT_FILE_LOADED => {
                {
                    let mut s = state.lock().unwrap();
                    s.error = None;
                }
                // Resume seek (spec §5.9): apply now, before frames flow.
                if let Some(pos) = pending_seek.lock().unwrap().take() {
                    raw_command(&api, &handle, &["seek", &format!("{pos:.3}"), "absolute"]);
                }
                pending_emit = true;
                significant = true;
            }
            MPV_EVENT_END_FILE => {
                let end = unsafe { &*(event.data as *const MpvEventEndFile) };
                if end.reason == MPV_END_FILE_REASON_ERROR {
                    let msg = unsafe { CStr::from_ptr((api.mpv_error_string)(end.error)) };
                    let mut s = state.lock().unwrap();
                    s.error = Some(msg.to_string_lossy().into_owned());
                    s.buffering = false;
                    s.playing = false;
                    pending_emit = true;
                    significant = true;
                }
            }
            MPV_EVENT_PROPERTY_CHANGE => {
                let prop = unsafe { &*(event.data as *const MpvEventProperty) };
                let name = unsafe { CStr::from_ptr(prop.name) }.to_string_lossy();
                let mut s = state.lock().unwrap();
                let is_significant = apply_property(&mut s, &name, prop);
                pending_emit = true;
                significant = significant || is_significant;
            }
            _ => {}
        }

        // Coalesce bursts; time-pos alone is throttled to ~2 Hz.
        if pending_emit && (significant || last_emit.elapsed().as_millis() >= 500) {
            let snapshot = state.lock().unwrap().clone();
            on_state(snapshot);
            last_emit = std::time::Instant::now();
            pending_emit = false;
            significant = false;
        }
    }
}

/// Returns whether the change warrants an immediate event (vs. throttled).
fn apply_property(s: &mut MpvState, name: &str, prop: &MpvEventProperty) -> bool {
    let as_flag = || -> Option<bool> {
        (prop.format == MPV_FORMAT_FLAG && !prop.data.is_null())
            .then(|| unsafe { *(prop.data as *const c_int) } != 0)
    };
    let as_double = || -> Option<f64> {
        (prop.format == MPV_FORMAT_DOUBLE && !prop.data.is_null())
            .then(|| unsafe { *(prop.data as *const c_double) })
    };
    let as_string = || -> Option<String> {
        if prop.format == MPV_FORMAT_STRING && !prop.data.is_null() {
            let ptr = unsafe { *(prop.data as *const *const c_char) };
            (!ptr.is_null())
                .then(|| unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
        } else {
            None
        }
    };

    match name {
        "pause" => {
            if let Some(v) = as_flag() {
                s.paused = v;
            }
            true
        }
        "idle-active" => {
            if let Some(v) = as_flag() {
                s.playing = !v;
            }
            true
        }
        "mute" => {
            if let Some(v) = as_flag() {
                s.muted = v;
            }
            true
        }
        "paused-for-cache" => {
            if let Some(v) = as_flag() {
                s.buffering = v;
            }
            true
        }
        "eof-reached" => true,
        "time-pos" => {
            s.position = as_double().unwrap_or(0.0);
            // First time-pos after a load means frames are flowing.
            if s.position > 0.0 && s.buffering && !s.paused {
                s.buffering = false;
            }
            false
        }
        "duration" => {
            s.duration = as_double().filter(|d| *d > 0.0);
            true
        }
        "volume" => {
            if let Some(v) = as_double() {
                s.volume = v;
            }
            true
        }
        "aid" => {
            s.active_audio_track = as_string().and_then(|v| v.parse().ok());
            true
        }
        "sid" => {
            s.active_subtitle_track = as_string().and_then(|v| v.parse().ok());
            true
        }
        "hwdec-current" => {
            s.hwdec_current = as_string().filter(|v| !v.is_empty() && v != "no");
            true
        }
        "track-list" => {
            if let Some(json) = as_string() {
                apply_track_list(s, &json);
            }
            true
        }
        _ => false,
    }
}

/// mpv serializes list properties as JSON when read in string format.
fn apply_track_list(s: &mut MpvState, json: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return;
    };
    let Some(tracks) = value.as_array() else {
        return;
    };
    s.audio_tracks.clear();
    s.subtitle_tracks.clear();
    for t in tracks {
        let info = TrackInfo {
            id: t["id"].as_i64().unwrap_or(0),
            title: t["title"].as_str().map(String::from),
            lang: t["lang"].as_str().map(String::from),
            codec: t["codec"].as_str().map(String::from),
        };
        match t["type"].as_str() {
            Some("audio") => s.audio_tracks.push(info),
            Some("sub") => s.subtitle_tracks.push(info),
            _ => {}
        }
    }
}

/// The macOS render thread (Milestone 38). Makes the (main-thread-created)
/// `NSOpenGLContext` current here, creates mpv's render context, and renders
/// each frame into the context's drawable (FBO 0). The drawing is guarded by
/// `CGLLockContext` so a main-thread `-update` (resize) can't reconfigure the
/// drawable mid-render; on a size change it asks NSOpenGL to `-update` on the
/// main thread. Mirrors the Windows render thread and the validated probe.
#[cfg(target_os = "macos")]
fn render_thread_mac(
    api: Arc<MpvApi>,
    handle: Arc<Handle>,
    gl_context: isize,
    gl_view: isize,
    stop: Arc<AtomicBool>,
) {
    use crate::mpv::render_mac;

    render_mac::make_current(gl_context);

    // Create the mpv render context on this thread (the GL context is current).
    let mut ctx: MpvRenderCtx = std::ptr::null_mut();
    let api_type = cstr("opengl");
    let mut gl_init = MpvOpenglInitParams {
        get_proc_address: Some(render_mac::get_proc_address),
        get_proc_address_ctx: std::ptr::null_mut(),
    };
    let rc = unsafe {
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
        (api.mpv_render_context_create)(&mut ctx, handle.0, params.as_mut_ptr())
    };
    if rc < 0 {
        let msg = unsafe { CStr::from_ptr((api.mpv_error_string)(rc)) }.to_string_lossy();
        eprintln!("[render] mpv_render_context_create failed: {msg}; video will not display");
        render_mac::clear_current();
        return;
    }

    let cgl = render_mac::cgl_context(gl_context);
    let mut last_size = (0i32, 0i32);

    while !stop.load(Ordering::SeqCst) {
        let (w, h) = render_mac::view_size(gl_view);
        if (w, h) != last_size {
            last_size = (w, h);
            render_mac::update_on_main(gl_context); // tell NSOpenGL the drawable resized
        }
        let flags = unsafe { (api.mpv_render_context_update)(ctx) };
        if flags & MPV_RENDER_UPDATE_FRAME != 0 {
            // Lock so a concurrent main-thread -update can't reconfigure the
            // drawable while we render + present.
            render_mac::lock(cgl);
            unsafe {
                let mut fbo = MpvOpenglFbo {
                    fbo: 0,
                    w,
                    h,
                    internal_format: 0,
                };
                let mut flip: c_int = 1;
                let mut params = [
                    MpvRenderParam {
                        type_: MPV_RENDER_PARAM_OPENGL_FBO,
                        data: &mut fbo as *mut _ as *mut c_void,
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
            render_mac::flush_buffer(gl_context); // present
            render_mac::unlock(cgl);
            unsafe { (api.mpv_render_context_report_swap)(ctx) };
        } else {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    // Ordered teardown: free the render context, then release the GL context.
    unsafe { (api.mpv_render_context_free)(ctx) };
    render_mac::clear_current();
}
