//! Spike B — libmpv `render` API proof of concept (Windows).
//!
//! Goal: prove we can drive libmpv's *render* API (instead of `--wid` window
//! embedding) — create an OpenGL context we own, hand mpv a `get_proc_address`,
//! and have mpv render each frame into our default framebuffer, which we then
//! `SwapBuffers`. This is the mechanism the embedding spike recommends and that
//! Milestone 37 (multi-view) would build on (N render contexts → N viewports).
//!
//! It is a STANDALONE example window (its own WGL context + message loop), NOT
//! wired into the Tauri app — so the GPU plumbing is de-risked in isolation.
//!
//! What to watch for when you run it (the deciding questions):
//!   1. Does it play? (logs: GL version, "file loaded", rising frame count)
//!   2. Does resizing the window stay smooth — no flicker / no black flashes?
//!   3. Does it close cleanly (no hang / crash on teardown)?
//!
//! Run:  cargo run --example render_api_spike -- "<stream url>"
//! (defaults to a public HLS test stream; pass a provider stream URL to test
//! real IPTV — mpv plays TS/HLS directly, so no proxy is needed here.)

#[cfg(not(windows))]
fn main() {
    eprintln!("render_api_spike is Windows-only.");
}

#[cfg(windows)]
fn main() {
    if let Err(e) = win::run() {
        eprintln!("[spike] ERROR: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod win {
    use libloading::Library;
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{GetDC, HDC};
    use windows_sys::Win32::Graphics::OpenGL::{
        wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent, ChoosePixelFormat,
        SetPixelFormat, SwapBuffers, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
        PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
    };
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
        PostMessageW, PostQuitMessage, RegisterClassW, TranslateMessage, CS_OWNDC, CW_USEDEFAULT,
        MSG, WM_CLOSE, WM_DESTROY, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    // --- mpv client + render API surface (client.h / render.h / render_gl.h) ---

    type MpvHandle = *mut c_void;
    type MpvRenderCtx = *mut c_void;

    const MPV_RENDER_PARAM_INVALID: c_int = 0;
    const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
    const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
    const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
    const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
    /// Bit returned by mpv_render_context_update() meaning "a new frame is ready".
    const MPV_RENDER_UPDATE_FRAME: u64 = 1;

    #[repr(C)]
    struct MpvRenderParam {
        type_: c_int,
        data: *mut c_void,
    }

    #[repr(C)]
    struct MpvOpenglInitParams {
        get_proc_address:
            Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
        get_proc_address_ctx: *mut c_void,
    }

    #[repr(C)]
    struct MpvOpenglFbo {
        fbo: c_int,
        w: c_int,
        h: c_int,
        internal_format: c_int,
    }

    struct Mpv {
        _lib: Library,
        create: unsafe extern "C" fn() -> MpvHandle,
        initialize: unsafe extern "C" fn(MpvHandle) -> c_int,
        terminate_destroy: unsafe extern "C" fn(MpvHandle),
        set_option_string:
            unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int,
        command: unsafe extern "C" fn(MpvHandle, *mut *const c_char) -> c_int,
        error_string: unsafe extern "C" fn(c_int) -> *const c_char,
        render_context_create:
            unsafe extern "C" fn(*mut MpvRenderCtx, MpvHandle, *mut MpvRenderParam) -> c_int,
        render_context_render: unsafe extern "C" fn(MpvRenderCtx, *mut MpvRenderParam) -> c_int,
        render_context_update: unsafe extern "C" fn(MpvRenderCtx) -> u64,
        render_context_report_swap: unsafe extern "C" fn(MpvRenderCtx),
        render_context_free: unsafe extern "C" fn(MpvRenderCtx),
    }

    // The mpv client API is thread-safe and the function pointers/library handle
    // are address-stable; sharing `Arc<Mpv>` across the main + render threads is
    // sound (the app shares `Arc<MpvApi>` the same way).
    unsafe impl Send for Mpv {}
    unsafe impl Sync for Mpv {}

    impl Mpv {
        fn load() -> Result<Self, String> {
            let lib = open_libmpv()?;
            unsafe {
                macro_rules! sym {
                    ($n:literal) => {
                        *lib.get(concat!($n, "\0").as_bytes())
                            .map_err(|e| format!("libmpv missing {}: {e}", $n))?
                    };
                }
                Ok(Self {
                    create: sym!("mpv_create"),
                    initialize: sym!("mpv_initialize"),
                    terminate_destroy: sym!("mpv_terminate_destroy"),
                    set_option_string: sym!("mpv_set_option_string"),
                    command: sym!("mpv_command"),
                    error_string: sym!("mpv_error_string"),
                    render_context_create: sym!("mpv_render_context_create"),
                    render_context_render: sym!("mpv_render_context_render"),
                    render_context_update: sym!("mpv_render_context_update"),
                    render_context_report_swap: sym!("mpv_render_context_report_swap"),
                    render_context_free: sym!("mpv_render_context_free"),
                    _lib: lib,
                })
            }
        }

        fn err(&self, code: c_int) -> String {
            unsafe { CStr::from_ptr((self.error_string)(code)) }
                .to_string_lossy()
                .into_owned()
        }
    }

    fn open_libmpv() -> Result<Library, String> {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                dirs.push(d.to_path_buf());
            }
        }
        // The repo keeps libmpv-2.dll in src-tauri/lib (gitignored) and copies it
        // next to the app exe; search both so the example finds it too.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        dirs.push(manifest.join("lib"));
        dirs.push(manifest.join("target/debug"));
        for name in ["libmpv-2.dll", "mpv-2.dll", "libmpv.dll"] {
            for dir in &dirs {
                if let Ok(lib) = unsafe { Library::new(dir.join(name)) } {
                    eprintln!("[spike] loaded {}", dir.join(name).display());
                    return Ok(lib);
                }
            }
            if let Ok(lib) = unsafe { Library::new(name) } {
                eprintln!("[spike] loaded {name} from PATH");
                return Ok(lib);
            }
        }
        Err("could not load libmpv-2.dll (put it in src-tauri/lib/ or on PATH)".into())
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    /// Decide what to play:
    ///   (no args)                  → a public HLS test stream
    ///   "<url>"                    → that URL verbatim (mpv plays TS/HLS/etc.)
    ///   --channel ["name filter"]  → resolve a REAL channel from the app's DB +
    ///                                keychain, exactly like the app does at play
    ///                                time (so you don't hand-compose URLs).
    fn acquire_url() -> Result<String, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(|s| s.as_str()) {
            Some("--channel") => resolve_channel(args.get(1).cloned()),
            Some(url) => {
                eprintln!("[spike] stream: {url}");
                Ok(url.to_string())
            }
            None => {
                let def = "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8".to_string();
                eprintln!("[spike] stream (default test): {def}");
                Ok(def)
            }
        }
    }

    /// Resolve a live channel's real stream URL straight from the app's SQLite DB
    /// + the OS keychain, mirroring the app's compose (`commands/playback.rs`).
    /// Uses sqlx + keyring directly — NOT the proscenium lib — because linking the
    /// Tauri-bound lib into an *example* won't load on Windows (no Common-Controls
    /// manifest; see CLAUDE.md). The composed URL (with the password) is never logged.
    fn resolve_channel(query: Option<String>) -> Result<String, String> {
        use sqlx::Row;
        let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
        let db_path = std::path::Path::new(&appdata)
            .join("proscenium")
            .join("proscenium.db");
        if !db_path.exists() {
            return Err(format!("app DB not found at {}", db_path.display()));
        }
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(async move {
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .read_only(true);
            let pool = sqlx::SqlitePool::connect_with(opts)
                .await
                .map_err(|e| format!("open db (is the app installed/initialized?): {e}"))?;

            let provider_id: String =
                sqlx::query("SELECT value FROM settings WHERE key='active_provider_id'")
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|r| r.get::<String, _>("value"))
                    .ok_or("no active provider — open the app and select one first")?;

            let prow = sqlx::query("SELECT name, type, server_url, username FROM providers WHERE id = ?")
                .bind(&provider_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("active provider row not found")?;
            let pname: String = prow.get("name");
            let ptype: String = prow.get("type");
            let server_url: Option<String> = prow.get("server_url");
            let username: Option<String> = prow.get("username");

            let crow = sqlx::query(
                "SELECT id, name, stream_ext, stream_url FROM live_channels
                 WHERE provider_id = ?1 AND (?2 = '' OR name LIKE ?3)
                 ORDER BY name COLLATE NOCASE LIMIT 1",
            )
            .bind(&provider_id)
            .bind(query.clone().unwrap_or_default())
            .bind(format!("%{}%", query.clone().unwrap_or_default()))
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| match &query {
                    Some(qq) => format!("no live channel matches \"{qq}\""),
                    None => "no live channels in the catalog".to_string(),
                })?;
            let chid: String = crow.get("id");
            let chname: String = crow.get("name");
            let ext: String = crow.get("stream_ext");
            let stored_url: String = crow.get("stream_url");
            eprintln!("[spike] provider: {pname} ({ptype}) | channel: {chname} ({chid})");

            let url = if ptype == "xtream" {
                let base = server_url
                    .as_deref()
                    .map(|s| s.trim_end_matches('/'))
                    .filter(|s| !s.is_empty())
                    .ok_or("provider has no server URL")?;
                let user = username
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or("provider has no username")?;
                let entry = keyring::Entry::new("Proscenium", &format!("provider:{provider_id}"))
                    .map_err(|e| format!("keychain: {e}"))?;
                let password = entry
                    .get_password()
                    .map_err(|e| format!("keychain read failed: {e}"))?;
                format!("{base}/live/{user}/{password}/{chid}.{ext}")
            } else {
                if stored_url.is_empty() {
                    return Err("M3U channel has no stored URL".into());
                }
                stored_url
            };
            eprintln!("[spike] resolved real stream URL from keychain (secret redacted)");
            Ok(url)
        })
    }

    // GL function loader handed to mpv: try wgl first (extensions), then the
    // opengl32 module (GL 1.1 core entry points wgl won't return).
    static OPENGL32: std::sync::OnceLock<isize> = std::sync::OnceLock::new();
    unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
        let p = wglGetProcAddress(name as *const u8);
        if let Some(f) = p {
            return f as *mut c_void;
        }
        let module = *OPENGL32.get_or_init(|| LoadLibraryA(b"opengl32.dll\0".as_ptr()) as isize);
        if module == 0 {
            return std::ptr::null_mut();
        }
        match GetProcAddress(module as _, name as *const u8) {
            Some(f) => f as *mut c_void,
            None => std::ptr::null_mut(),
        }
    }

    /// Set when the window receives WM_CLOSE/DESTROY so the render loop exits.
    static QUIT: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CLOSE => {
                QUIT.store(true, Ordering::SeqCst);
                PostQuitMessage(0);
                0
            }
            WM_DESTROY => {
                QUIT.store(true, Ordering::SeqCst);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn client_size(hwnd: HWND) -> (c_int, c_int) {
        unsafe {
            let mut r = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            ((r.right - r.left).max(1), (r.bottom - r.top).max(1))
        }
    }

    pub fn run() -> Result<(), String> {
        let url = acquire_url()?;
        let mpv = std::sync::Arc::new(Mpv::load()?);

        // Window + message pump live on THIS (main) thread; rendering lives on a
        // dedicated thread. That separation is the whole point of the fix: a
        // Win32 drag-resize runs a *modal* loop on the window's thread, so if we
        // rendered here the video would freeze mid-resize. With rendering off on
        // its own thread, resize stays smooth and the main thread always services
        // window messages (so resizing never gets "stuck").
        let hwnd = unsafe { create_window()? };

        // mpv player (the handle is thread-safe; the render *context* is created
        // and used only on the render thread, where the GL context is current).
        let handle = unsafe { (mpv.create)() };
        if handle.is_null() {
            return Err("mpv_create returned null".into());
        }
        unsafe {
            let set = |k: &str, v: &str| {
                (mpv.set_option_string)(handle, cstr(k).as_ptr(), cstr(v).as_ptr())
            };
            set("terminal", "no");
            set("msg-level", "all=warn");
            set("vo", "libmpv");
            set("hwdec", "auto-safe");
            let rc = (mpv.initialize)(handle);
            if rc < 0 {
                return Err(format!("mpv_initialize failed: {}", mpv.err(rc)));
            }
        }

        // Spawn the render thread. Pass the raw handles as isize (Send) and cast
        // back inside — avoids wrapping, since a Copy `*mut c_void` field would
        // still be captured bare under 2021 disjoint closure captures.
        let hwnd_n = hwnd as isize;
        let handle_n = handle as isize;
        let mpv_rt = mpv.clone();
        let url_rt = url.clone();
        let render = std::thread::spawn(move || {
            let hwnd = hwnd_n as *mut c_void;
            let handle = handle_n as *mut c_void;
            if let Err(e) = render_thread(&mpv_rt, handle, hwnd, &url_rt) {
                eprintln!("[spike] render thread ERROR: {e}");
                // Wake the main thread so it can exit.
                unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            }
        });

        // Message pump: block on messages (no busy-spin — rendering is elsewhere).
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        QUIT.store(true, Ordering::SeqCst);
        let _ = render.join();

        // The render thread freed its render context before exiting; now destroy
        // the player (order matters).
        unsafe { (mpv.terminate_destroy)(handle) };
        eprintln!("[spike] done.");
        Ok(())
    }

    /// Runs on the render thread: owns the GL context, renders mpv into it.
    fn render_thread(
        mpv: &Mpv,
        handle: MpvHandle,
        hwnd: *mut c_void,
        url: &str,
    ) -> Result<(), String> {
        let (hdc, hglrc) = unsafe { init_gl(hwnd)? };
        eprintln!("[spike] GL context current on render thread");

        // GL version/renderer (context is current now).
        unsafe {
            let gl_get_string: Option<unsafe extern "C" fn(u32) -> *const u8> = std::mem::transmute(
                get_proc_address(std::ptr::null_mut(), b"glGetString\0".as_ptr() as _),
            );
            if let Some(f) = gl_get_string {
                let ver = f(0x1F02);
                let rend = f(0x1F01);
                if !ver.is_null() {
                    eprintln!("[spike] GL_VERSION  = {}", CStr::from_ptr(ver as _).to_string_lossy());
                }
                if !rend.is_null() {
                    eprintln!("[spike] GL_RENDERER = {}", CStr::from_ptr(rend as _).to_string_lossy());
                }
            }
        }

        // Render context.
        let mut ctx: MpvRenderCtx = std::ptr::null_mut();
        unsafe {
            let api = cstr("opengl");
            let mut gl_init = MpvOpenglInitParams {
                get_proc_address: Some(get_proc_address),
                get_proc_address_ctx: std::ptr::null_mut(),
            };
            let mut params = [
                MpvRenderParam { type_: MPV_RENDER_PARAM_API_TYPE, data: api.as_ptr() as *mut c_void },
                MpvRenderParam {
                    type_: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: &mut gl_init as *mut _ as *mut c_void,
                },
                MpvRenderParam { type_: MPV_RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
            ];
            let rc = (mpv.render_context_create)(&mut ctx, handle, params.as_mut_ptr());
            if rc < 0 {
                return Err(format!(
                    "mpv_render_context_create failed: {} — this libmpv may not support the GL render API",
                    mpv.err(rc)
                ));
            }
        }
        eprintln!("[spike] render context created OK");

        // loadfile.
        unsafe {
            let load = cstr("loadfile");
            let u = cstr(url);
            let mut args = [load.as_ptr(), u.as_ptr(), std::ptr::null()];
            let rc = (mpv.command)(handle, args.as_mut_ptr());
            if rc < 0 {
                return Err(format!("loadfile failed: {}", mpv.err(rc)));
            }
        }

        // Render loop (vsync-paced via SwapBuffers), unaffected by the main
        // thread's modal resize loop.
        let max_secs = std::env::var("SPIKE_SECS").ok().and_then(|s| s.parse::<u64>().ok());
        let start = std::time::Instant::now();
        let mut frames: u64 = 0;
        let mut last_size = (0, 0);
        loop {
            if QUIT.load(Ordering::SeqCst) {
                break;
            }
            if let Some(m) = max_secs {
                if start.elapsed().as_secs() >= m {
                    eprintln!("[spike] auto-quit after {m}s");
                    unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                    break;
                }
            }
            let flags = unsafe { (mpv.render_context_update)(ctx) };
            let (w, h) = client_size(hwnd);
            if (w, h) != last_size {
                last_size = (w, h);
                eprintln!("[spike] resize -> {w}x{h}");
            }
            if flags & MPV_RENDER_UPDATE_FRAME != 0 {
                unsafe {
                    let mut fbo = MpvOpenglFbo { fbo: 0, w, h, internal_format: 0 };
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
                    (mpv.render_context_render)(ctx, params.as_mut_ptr());
                    SwapBuffers(hdc);
                    (mpv.render_context_report_swap)(ctx);
                }
                frames += 1;
                if frames % 120 == 0 {
                    let fps = frames as f64 / start.elapsed().as_secs_f64();
                    eprintln!("[spike] {frames} frames ({fps:.0} fps avg), {w}x{h}");
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }

        eprintln!("[spike] render thread tearing down… ({frames} frames)");
        unsafe {
            (mpv.render_context_free)(ctx);
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            wglDeleteContext(hglrc as *mut c_void);
        }
        Ok(())
    }

    /// Register the class + create the (visible, resizable) window on the calling
    /// (main) thread. No GL here — that's set up on the render thread.
    unsafe fn create_window() -> Result<*mut c_void, String> {
        let class_name = wide("ProsceniumRenderSpike");
        let wc = WNDCLASSW {
            style: CS_OWNDC,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: std::ptr::null_mut(),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);
        let title = wide("Proscenium render-API spike (resize me!)");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            720,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowExW failed".into());
        }
        Ok(hwnd)
    }

    /// Set up the WGL/GL context on the window's private DC. Called on the render
    /// thread so the context is current there.
    unsafe fn init_gl(hwnd: *mut c_void) -> Result<(HDC, isize), String> {
        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return Err("GetDC failed".into());
        }
        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
        pfd.nVersion = 1;
        pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
        pfd.iPixelType = PFD_TYPE_RGBA as u8;
        pfd.cColorBits = 32;
        pfd.cDepthBits = 24;
        pfd.iLayerType = PFD_MAIN_PLANE as u8;
        let pf = ChoosePixelFormat(hdc, &pfd);
        if pf == 0 {
            return Err("ChoosePixelFormat failed".into());
        }
        if SetPixelFormat(hdc, pf, &pfd) == 0 {
            return Err("SetPixelFormat failed".into());
        }
        let hglrc = wglCreateContext(hdc);
        if hglrc.is_null() {
            return Err("wglCreateContext failed".into());
        }
        if wglMakeCurrent(hdc, hglrc) == 0 {
            return Err("wglMakeCurrent failed".into());
        }
        Ok((hdc, hglrc as isize))
    }
}
