//! macOS render-API probe — go/no-go for Milestone 38's macOS path.
//!
//! Answers ONE question (the exact macOS analogue of Spike B's #1 unknown):
//!   Does THIS Mac's libmpv.2.dylib support `mpv_render_context_create` with
//!   MPV_RENDER_API_TYPE_OPENGL, and do frames flow into a GL context we own?
//!
//! Tier 1 (this file): a headless CGL context (no window, no main-thread
//! constraints) → create the render context → that return code is the answer.
//! If it succeeds we load a real stream, render into a texture-backed FBO, and
//! glReadPixels one pixel to prove frames are non-black. Tier 2 (onscreen
//! NSOpenGL + resize) is only worth writing if Tier 1 PASSES.
//!
//! Deliberately standalone: does NOT link proscenium_lib (the Tauri/objc2/
//! private-api surface doesn't belong in a bare example). DB + keychain are
//! read with sqlx + keyring directly, exactly like the Windows spike.
//!
//! Run (from src-tauri/):
//!   cargo run --example render_api_probe_macos                 # default test HLS
//!   cargo run --example render_api_probe_macos -- --channel ESPN
//!   SPIKE_SECS=8 cargo run --example render_api_probe_macos    # headless auto-quit

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("render_api_probe_macos is macOS-only.");
}

#[cfg(target_os = "macos")]
fn main() {
    if let Err(e) = mac::run() {
        eprintln!("[probe] ERROR: {e}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use libloading::Library;
    use std::ffi::{c_char, c_int, c_void, CStr, CString};

    // ---- mpv client + render API surface (reused verbatim from the Windows
    // spike — these are OS-independent libmpv ABI bindings) ----

    type MpvHandle = *mut c_void;
    type MpvRenderCtx = *mut c_void;

    const MPV_RENDER_PARAM_INVALID: c_int = 0;
    const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
    const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
    const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
    const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
    /// Bit returned by mpv_render_context_update() meaning "a new frame is ready".
    const MPV_RENDER_UPDATE_FRAME: u64 = 1;
    /// mpv error code for an unsupported render-API type.
    const MPV_ERROR_NOT_IMPLEMENTED: c_int = -12;

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

    /// Search the .dylib paths from the probe doc §1.3; load by ABSOLUTE path to
    /// sidestep @rpath resolution.
    fn open_libmpv() -> Result<Library, String> {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut candidates: Vec<std::path::PathBuf> = vec![
            // Repo's bundled copy (gitignored — same place RELEASE.md stages it).
            manifest.join("lib/libmpv.2.dylib"),
            // Homebrew (Apple Silicon, then Intel).
            "/opt/homebrew/opt/mpv/lib/libmpv.2.dylib".into(),
            "/usr/local/opt/mpv/lib/libmpv.2.dylib".into(),
        ];
        // Also next to the example exe, if staged there.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                candidates.push(d.join("libmpv.2.dylib"));
            }
        }
        for p in &candidates {
            if let Ok(lib) = unsafe { Library::new(p) } {
                eprintln!("[probe] loaded {}", p.display());
                return Ok(lib);
            }
        }
        // Last resort: the loader's default search.
        if let Ok(lib) = unsafe { Library::new("libmpv.2.dylib") } {
            eprintln!("[probe] loaded libmpv.2.dylib from default search path");
            return Ok(lib);
        }
        Err("could not load libmpv.2.dylib (put it in src-tauri/lib/ or install \
             via Homebrew, or set DYLD_LIBRARY_PATH)"
            .into())
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    // ---- stream acquisition (reused; only the DB path differs from Windows) ----

    fn acquire_url() -> Result<String, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(|s| s.as_str()) {
            Some("--channel") => resolve_channel(args.get(1).cloned()),
            Some(url) => {
                eprintln!("[probe] stream: {url}");
                Ok(url.to_string())
            }
            None => {
                let def = "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8".to_string();
                eprintln!("[probe] stream (default test): {def}");
                Ok(def)
            }
        }
    }

    /// Resolve a live channel's real stream URL from the app's SQLite DB + the OS
    /// keychain, mirroring commands/playback.rs. macOS DB lives under
    /// ~/Library/Application Support/proscenium; keychain is identical to Windows
    /// (service "Proscenium", account "provider:{id}", apple-native backend). The
    /// composed URL (with the password) is never logged.
    fn resolve_channel(query: Option<String>) -> Result<String, String> {
        use sqlx::Row;
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let db_path = std::path::Path::new(&home)
            .join("Library/Application Support/proscenium/proscenium.db");
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

            let prow =
                sqlx::query("SELECT name, type, server_url, username FROM providers WHERE id = ?")
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
            eprintln!("[probe] provider: {pname} ({ptype}) | channel: {chname} ({chid})");

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
            eprintln!("[probe] resolved real stream URL from keychain (secret redacted)");
            Ok(url)
        })
    }

    // ---- CGL (headless GL context) + GL symbol loading ----

    type CglPixelFormatObj = *mut c_void;
    type CglContextObj = *mut c_void;
    type CglError = c_int;
    type CglPixelFormatAttribute = u32;

    // CGLTypes.h attribute selectors.
    const KCGL_PFA_ACCELERATED: CglPixelFormatAttribute = 73;
    const KCGL_PFA_DOUBLE_BUFFER: CglPixelFormatAttribute = 5;
    const KCGL_PFA_OPENGL_PROFILE: CglPixelFormatAttribute = 99;
    const KCGL_PFA_COLOR_SIZE: CglPixelFormatAttribute = 8;
    // CGLOpenGLProfile.h
    const KCGL_OGLP_VERSION_3_2_CORE: CglPixelFormatAttribute = 0x3200;

    #[link(name = "OpenGL", kind = "framework")]
    extern "C" {
        fn CGLChoosePixelFormat(
            attribs: *const CglPixelFormatAttribute,
            pix: *mut CglPixelFormatObj,
            npix: *mut c_int,
        ) -> CglError;
        fn CGLCreateContext(
            pix: CglPixelFormatObj,
            share: CglContextObj,
            ctx: *mut CglContextObj,
        ) -> CglError;
        fn CGLSetCurrentContext(ctx: CglContextObj) -> CglError;
        fn CGLDestroyContext(ctx: CglContextObj) -> CglError;
        fn CGLDestroyPixelFormat(pix: CglPixelFormatObj) -> CglError;
        fn CGLErrorString(error: CglError) -> *const c_char;
        // Serialize render-thread drawing against a main-thread -update (Tier 2).
        fn CGLLockContext(ctx: CglContextObj) -> CglError;
        fn CGLUnlockContext(ctx: CglContextObj) -> CglError;
    }

    /// dlopen the OpenGL framework once; mpv's get_proc_address dlsym's into it.
    static OPENGL_FW: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
        let handle = *OPENGL_FW.get_or_init(|| {
            let path = cstr(
                "/System/Library/Frameworks/OpenGL.framework/Versions/Current/OpenGL",
            );
            libc::dlopen(path.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) as usize
        });
        if handle == 0 {
            return std::ptr::null_mut();
        }
        libc::dlsym(handle as *mut c_void, name)
    }

    /// Resolve a GL function pointer by name (context must be current).
    unsafe fn gl<T>(name: &str) -> T {
        let p = get_proc_address(std::ptr::null_mut(), cstr(name).as_ptr());
        assert!(!p.is_null(), "GL symbol not found: {name}");
        std::mem::transmute_copy::<*mut c_void, T>(&p)
    }

    // GL enum constants used by the offscreen FBO + readback.
    const GL_VENDOR: u32 = 0x1F00;
    const GL_RENDERER: u32 = 0x1F01;
    const GL_VERSION: u32 = 0x1F02;
    const GL_TEXTURE_2D: u32 = 0x0DE1;
    const GL_RGBA: u32 = 0x1908;
    const GL_RGBA8: u32 = 0x8058;
    const GL_UNSIGNED_BYTE: u32 = 0x1401;
    const GL_FRAMEBUFFER: u32 = 0x8D40;
    const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
    const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;

    pub fn run() -> Result<(), String> {
        let url = acquire_url()?;
        let mpv = Mpv::load()?;

        // PROBE_TIER2=1 → run the onscreen NSOpenGL window (resize verification)
        // instead of the headless Tier 1 go/no-go. Only meaningful once Tier 1
        // has PASSED (it has) — this confirms the representative onscreen path.
        if std::env::var("PROBE_TIER2").as_deref() == Ok("1") {
            return tier2::run_windowed(mpv, url);
        }

        // --- Tier 1 step 1: headless CGL context (3.2 core) ---
        let (pix, ctx) = unsafe { create_cgl_context()? };
        eprintln!("[probe] CGL context current (headless, 3.2 core)");

        // GL version/renderer (context is current).
        unsafe {
            let get_string: unsafe extern "C" fn(u32) -> *const u8 = gl("glGetString");
            for (label, e) in [
                ("GL_VERSION ", GL_VERSION),
                ("GL_RENDERER", GL_RENDERER),
                ("GL_VENDOR  ", GL_VENDOR),
            ] {
                let s = get_string(e);
                if !s.is_null() {
                    eprintln!(
                        "[probe] {label} = {}",
                        CStr::from_ptr(s as _).to_string_lossy()
                    );
                }
            }
        }

        // --- mpv player ---
        let handle = unsafe { (mpv.create)() };
        if handle.is_null() {
            return Err("mpv_create returned null".into());
        }
        unsafe {
            let set = |k: &str, v: &str| {
                (mpv.set_option_string)(handle, cstr(k).as_ptr(), cstr(v).as_ptr())
            };
            // Keep mpv's terminal logging at warn (matches the Windows spike):
            // 'v' echoes the resolved stream URL — which contains the keychain
            // password — so warn keeps errors visible without leaking the secret.
            set("terminal", "yes");
            set("msg-level", "all=warn");
            set("vo", "libmpv");
            set("hwdec", "auto-safe");
            let rc = (mpv.initialize)(handle);
            if rc < 0 {
                return Err(format!("mpv_initialize failed: {}", mpv.err(rc)));
            }
        }

        // --- Tier 1 step 4: THE ANSWER — create the render context ---
        let mut rctx: MpvRenderCtx = std::ptr::null_mut();
        let answer = unsafe { try_create_render_ctx(&mpv, handle, &mut rctx, "opengl") };
        match answer {
            0 => eprintln!("[probe] render context created OK   <-- PASS (OpenGL render API)"),
            MPV_ERROR_NOT_IMPLEMENTED => {
                eprintln!(
                    "[probe] OpenGL render API NOT IMPLEMENTED ({}) — retrying with \"sw\"…",
                    mpv.err(MPV_ERROR_NOT_IMPLEMENTED)
                );
                let sw = unsafe { try_create_render_ctx(&mpv, handle, &mut rctx, "sw") };
                if sw == 0 {
                    eprintln!("[probe] software render API works   <-- PARTIAL (GL unavailable, sw only — not shippable)");
                } else {
                    eprintln!("[probe] software render API also failed: {}   <-- FAIL", mpv.err(sw));
                }
                // Either way the GL go/no-go is answered; stop here.
                unsafe { teardown(&mpv, handle, rctx, pix, ctx) };
                return Ok(());
            }
            other => {
                eprintln!(
                    "[probe] mpv_render_context_create failed: {} (code {other})   <-- FAIL",
                    mpv.err(other)
                );
                unsafe { teardown(&mpv, handle, rctx, pix, ctx) };
                return Ok(());
            }
        }

        // --- PASS path: prove frames actually flow. Render offscreen into a
        // texture-backed FBO and glReadPixels a center pixel to show non-black. ---
        const W: c_int = 1280;
        const H: c_int = 720;
        let fbo = unsafe { create_offscreen_fbo(W, H)? };
        eprintln!("[probe] offscreen FBO {fbo} ({W}x{H}) ready");

        unsafe {
            let load = cstr("loadfile");
            let u = cstr(&url);
            let mut args = [load.as_ptr(), u.as_ptr(), std::ptr::null()];
            let rc = (mpv.command)(handle, args.as_mut_ptr());
            if rc < 0 {
                return Err(format!("loadfile failed: {}", mpv.err(rc)));
            }
        }

        let max_secs = std::env::var("SPIKE_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(15);
        let start = std::time::Instant::now();
        let mut frames: u64 = 0;
        let mut non_black_seen = false;

        let read_pixels: unsafe extern "C" fn(c_int, c_int, c_int, c_int, u32, u32, *mut c_void) =
            unsafe { gl("glReadPixels") };
        // mpv leaves framebuffer 0 bound after rendering; on a headless CGL
        // context that default FB has no drawable, so we must re-bind our
        // offscreen FBO before reading back the pixel mpv rendered into it.
        let bind_framebuffer: unsafe extern "C" fn(u32, u32) = unsafe { gl("glBindFramebuffer") };

        loop {
            if start.elapsed().as_secs() >= max_secs {
                eprintln!("[probe] auto-quit after {max_secs}s");
                break;
            }
            let flags = unsafe { (mpv.render_context_update)(rctx) };
            if flags & MPV_RENDER_UPDATE_FRAME != 0 {
                unsafe {
                    let mut gl_fbo = MpvOpenglFbo { fbo, w: W, h: H, internal_format: 0 };
                    let mut flip: c_int = 1;
                    let mut params = [
                        MpvRenderParam {
                            type_: MPV_RENDER_PARAM_OPENGL_FBO,
                            data: &mut gl_fbo as *mut _ as *mut c_void,
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
                    (mpv.render_context_render)(rctx, params.as_mut_ptr());
                    (mpv.render_context_report_swap)(rctx);

                    // Read one center pixel as proof the FBO got real content.
                    bind_framebuffer(GL_FRAMEBUFFER, fbo as u32);
                    let mut px = [0u8; 4];
                    read_pixels(
                        W / 2,
                        H / 2,
                        1,
                        1,
                        GL_RGBA,
                        GL_UNSIGNED_BYTE,
                        px.as_mut_ptr() as *mut c_void,
                    );
                    if !non_black_seen && (px[0] | px[1] | px[2]) != 0 {
                        non_black_seen = true;
                        eprintln!(
                            "[probe] first non-black frame: center pixel rgba = {:?}",
                            px
                        );
                    }
                }
                frames += 1;
                if frames % 60 == 0 {
                    let fps = frames as f64 / start.elapsed().as_secs_f64();
                    eprintln!("[probe] {frames} frames ({fps:.0} fps avg)");
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }

        eprintln!("[probe] tearing down… ({frames} frames, non_black_seen={non_black_seen})");
        unsafe { teardown(&mpv, handle, rctx, pix, ctx) };
        eprintln!("[probe] done — {frames} frames rendered.");
        if frames == 0 {
            eprintln!("[probe] WARNING: render context created but zero frames — investigate.");
        }
        Ok(())
    }

    /// Build the params and call mpv_render_context_create with the given API type.
    unsafe fn try_create_render_ctx(
        mpv: &Mpv,
        handle: MpvHandle,
        out: &mut MpvRenderCtx,
        api_type: &str,
    ) -> c_int {
        let api = cstr(api_type);
        let mut gl_init = MpvOpenglInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        // For "sw" the GL init params are ignored, but passing them is harmless.
        let mut params = [
            MpvRenderParam {
                type_: MPV_RENDER_PARAM_API_TYPE,
                data: api.as_ptr() as *mut c_void,
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
        (mpv.render_context_create)(out, handle, params.as_mut_ptr())
    }

    /// Create + make-current a headless CGL context (OpenGL 3.2 core).
    unsafe fn create_cgl_context() -> Result<(CglPixelFormatObj, CglContextObj), String> {
        let attribs: [CglPixelFormatAttribute; 8] = [
            KCGL_PFA_ACCELERATED,
            KCGL_PFA_DOUBLE_BUFFER,
            KCGL_PFA_COLOR_SIZE,
            32,
            KCGL_PFA_OPENGL_PROFILE,
            KCGL_OGLP_VERSION_3_2_CORE,
            0,
            0,
        ];
        let cgl_err = |e: CglError| -> String {
            let s = CGLErrorString(e);
            if s.is_null() {
                format!("CGL error {e}")
            } else {
                CStr::from_ptr(s).to_string_lossy().into_owned()
            }
        };
        let mut pix: CglPixelFormatObj = std::ptr::null_mut();
        let mut npix: c_int = 0;
        let e = CGLChoosePixelFormat(attribs.as_ptr(), &mut pix, &mut npix);
        if e != 0 || pix.is_null() {
            return Err(format!("CGLChoosePixelFormat failed: {}", cgl_err(e)));
        }
        let mut ctx: CglContextObj = std::ptr::null_mut();
        let e = CGLCreateContext(pix, std::ptr::null_mut(), &mut ctx);
        if e != 0 || ctx.is_null() {
            return Err(format!("CGLCreateContext failed: {}", cgl_err(e)));
        }
        let e = CGLSetCurrentContext(ctx);
        if e != 0 {
            return Err(format!("CGLSetCurrentContext failed: {}", cgl_err(e)));
        }
        Ok((pix, ctx))
    }

    /// Create a texture-backed framebuffer to render into (headless target).
    unsafe fn create_offscreen_fbo(w: c_int, h: c_int) -> Result<c_int, String> {
        let gen_textures: unsafe extern "C" fn(c_int, *mut u32) = gl("glGenTextures");
        let bind_texture: unsafe extern "C" fn(u32, u32) = gl("glBindTexture");
        let tex_image2d: unsafe extern "C" fn(
            u32, c_int, c_int, c_int, c_int, c_int, u32, u32, *const c_void,
        ) = gl("glTexImage2D");
        let gen_framebuffers: unsafe extern "C" fn(c_int, *mut u32) = gl("glGenFramebuffers");
        let bind_framebuffer: unsafe extern "C" fn(u32, u32) = gl("glBindFramebuffer");
        let framebuffer_texture2d: unsafe extern "C" fn(u32, u32, u32, u32, c_int) =
            gl("glFramebufferTexture2D");
        let check_framebuffer_status: unsafe extern "C" fn(u32) -> u32 =
            gl("glCheckFramebufferStatus");

        let mut tex: u32 = 0;
        gen_textures(1, &mut tex);
        bind_texture(GL_TEXTURE_2D, tex);
        tex_image2d(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as c_int,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        let mut fbo: u32 = 0;
        gen_framebuffers(1, &mut fbo);
        bind_framebuffer(GL_FRAMEBUFFER, fbo);
        framebuffer_texture2d(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
        let status = check_framebuffer_status(GL_FRAMEBUFFER);
        if status != GL_FRAMEBUFFER_COMPLETE {
            return Err(format!("framebuffer incomplete: 0x{status:X}"));
        }
        Ok(fbo as c_int)
    }

    /// Teardown in the order the probe doc mandates:
    /// render_context_free → terminate_destroy → CGLSetCurrentContext(null) →
    /// CGLDestroyContext (+ pixel format).
    unsafe fn teardown(
        mpv: &Mpv,
        handle: MpvHandle,
        rctx: MpvRenderCtx,
        pix: CglPixelFormatObj,
        ctx: CglContextObj,
    ) {
        if !rctx.is_null() {
            (mpv.render_context_free)(rctx);
        }
        (mpv.terminate_destroy)(handle);
        CGLSetCurrentContext(std::ptr::null_mut());
        if !ctx.is_null() {
            CGLDestroyContext(ctx);
        }
        if !pix.is_null() {
            CGLDestroyPixelFormat(pix);
        }
    }

    /// Tier 2 — onscreen NSOpenGL window with rendering on a DEDICATED thread.
    /// This is the macOS analogue of Spike B §3a: the main thread runs only the
    /// Cocoa event loop, so a drag-resize (which spins a modal loop) can never
    /// starve rendering. Manually resize the window in several directions and
    /// confirm: video keeps playing, no freeze, no flicker, no "stuck" resize.
    ///
    /// Run:  PROBE_TIER2=1 cargo run --example render_api_probe_macos -- --channel ESPN
    /// Close the window (red button) to exit; or set SPIKE_SECS=N to auto-quit.
    mod tier2 {
        use super::*;
        use objc2::encode::{Encode, Encoding};
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send, sel};
        use std::sync::atomic::{AtomicBool, Ordering};

        // CoreGraphics geometry mirrors (same pattern as src/mpv/mod.rs).
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CGPoint {
            x: f64,
            y: f64,
        }
        unsafe impl Encode for CGPoint {
            const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CGSize {
            width: f64,
            height: f64,
        }
        unsafe impl Encode for CGSize {
            const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CGRect {
            origin: CGPoint,
            size: CGSize,
        }
        unsafe impl Encode for CGRect {
            const ENCODING: Encoding =
                Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
        }

        // NSWindowStyleMask: Titled|Closable|Miniaturizable|Resizable = 1|2|4|8.
        const NS_WINDOW_STYLE: u64 = 1 | 2 | 4 | 8;
        const NS_BACKING_STORE_BUFFERED: u64 = 2;
        const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: i64 = 0;

        // NSOpenGLPixelFormatAttribute values.
        const NSOPENGL_PFA_ACCELERATED: u32 = 73;
        const NSOPENGL_PFA_DOUBLE_BUFFER: u32 = 5;
        const NSOPENGL_PFA_COLOR_SIZE: u32 = 8;
        const NSOPENGL_PFA_OPENGL_PROFILE: u32 = 99;
        const NSOPENGL_PROFILE_VERSION_3_2_CORE: u32 = 0x3200;

        // Set to stop the render loop early (currently only the in-loop checks
        // — window close / SPIKE_SECS — end it; kept for symmetry with Spike B).
        static QUIT: AtomicBool = AtomicBool::new(false);

        pub fn run_windowed(mpv: Mpv, url: String) -> Result<(), String> {
            unsafe {
                let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
                let _: () =
                    msg_send![app, setActivationPolicy: NS_APPLICATION_ACTIVATION_POLICY_REGULAR];

                // Window on the main thread (Cocoa requires it).
                let rect = CGRect {
                    origin: CGPoint { x: 100.0, y: 100.0 },
                    size: CGSize { width: 1280.0, height: 720.0 },
                };
                let win: *mut AnyObject = msg_send![class!(NSWindow), alloc];
                let win: *mut AnyObject = msg_send![
                    win,
                    initWithContentRect: rect,
                    styleMask: NS_WINDOW_STYLE,
                    backing: NS_BACKING_STORE_BUFFERED,
                    defer: false
                ];
                // Don't dealloc the window object when the red button closes it —
                // the render thread still reads isVisible to detect the close.
                let _: () = msg_send![win, setReleasedWhenClosed: false];
                let title: *mut AnyObject = msg_send![
                    class!(NSString),
                    stringWithUTF8String: cstr("Proscenium render-API probe — resize me!").as_ptr()
                ];
                let _: () = msg_send![win, setTitle: title];

                let view: *mut AnyObject = msg_send![win, contentView];
                // 1:1 backing (no retina doubling) keeps the FBO size = point size,
                // so the render thread can use the view bounds directly.
                let _: () = msg_send![view, setWantsBestResolutionOpenGLSurface: false];

                // NSOpenGL pixel format + context (3.2 core, double-buffered).
                let attrs: [u32; 8] = [
                    NSOPENGL_PFA_ACCELERATED,
                    NSOPENGL_PFA_DOUBLE_BUFFER,
                    NSOPENGL_PFA_COLOR_SIZE,
                    24,
                    NSOPENGL_PFA_OPENGL_PROFILE,
                    NSOPENGL_PROFILE_VERSION_3_2_CORE,
                    0,
                    0,
                ];
                let pf: *mut AnyObject = msg_send![class!(NSOpenGLPixelFormat), alloc];
                let pf: *mut AnyObject = msg_send![pf, initWithAttributes: attrs.as_ptr()];
                if pf.is_null() {
                    return Err("NSOpenGLPixelFormat init failed (no 3.2 core pixel format)".into());
                }
                let gctx: *mut AnyObject = msg_send![class!(NSOpenGLContext), alloc];
                let nil: *mut AnyObject = std::ptr::null_mut();
                let gctx: *mut AnyObject = msg_send![gctx, initWithFormat: pf, shareContext: nil];
                if gctx.is_null() {
                    return Err("NSOpenGLContext init failed".into());
                }
                // Associate the context with the view (main thread).
                let _: () = msg_send![gctx, setView: view];

                let _: () = msg_send![win, makeKeyAndOrderFront: nil];
                let _: () = msg_send![app, activateIgnoringOtherApps: true];

                // Render on a dedicated thread (the §3a fix). Pass pointers as
                // isize (Send) and rebuild inside.
                let gctx_n = gctx as isize;
                let view_n = view as isize;
                let win_n = win as isize;
                std::thread::spawn(move || {
                    if let Err(e) = render_thread(mpv, url, gctx_n, view_n, win_n) {
                        eprintln!("[probe] render thread ERROR: {e}");
                    }
                    // Ordered teardown happened inside; leave the app.
                    std::process::exit(0);
                });

                // Main thread: Cocoa event loop only. Never returns (the render
                // thread process::exit's on close/timeout).
                let _: () = msg_send![app, run];
            }
            Ok(())
        }

        fn render_thread(
            mpv: Mpv,
            url: String,
            gctx_n: isize,
            view_n: isize,
            win_n: isize,
        ) -> Result<(), String> {
            let gctx = gctx_n as *mut AnyObject;
            let view = view_n as *mut AnyObject;
            let win = win_n as *mut AnyObject;

            unsafe {
                let _: () = msg_send![gctx, makeCurrentContext];
            }

            // GL strings (context current on this thread now).
            unsafe {
                let get_string: unsafe extern "C" fn(u32) -> *const u8 = gl("glGetString");
                for (label, e) in [("GL_VERSION ", GL_VERSION), ("GL_RENDERER", GL_RENDERER)] {
                    let s = get_string(e);
                    if !s.is_null() {
                        eprintln!("[probe] {label} = {}", CStr::from_ptr(s as _).to_string_lossy());
                    }
                }
            }

            // mpv player + render context.
            let handle = unsafe { (mpv.create)() };
            if handle.is_null() {
                return Err("mpv_create returned null".into());
            }
            unsafe {
                let set = |k: &str, v: &str| {
                    (mpv.set_option_string)(handle, cstr(k).as_ptr(), cstr(v).as_ptr())
                };
                set("terminal", "yes");
                set("msg-level", "all=warn"); // 'v' would leak the stream URL/password
                set("vo", "libmpv");
                set("hwdec", "auto-safe");
                let rc = (mpv.initialize)(handle);
                if rc < 0 {
                    return Err(format!("mpv_initialize failed: {}", mpv.err(rc)));
                }
            }

            let mut rctx: MpvRenderCtx = std::ptr::null_mut();
            let rc = unsafe { try_create_render_ctx(&mpv, handle, &mut rctx, "opengl") };
            if rc < 0 {
                return Err(format!("mpv_render_context_create failed: {}", mpv.err(rc)));
            }
            eprintln!("[probe] render context created OK (onscreen NSOpenGL)");

            unsafe {
                let load = cstr("loadfile");
                let u = cstr(&url);
                let mut args = [load.as_ptr(), u.as_ptr(), std::ptr::null()];
                let rc = (mpv.command)(handle, args.as_mut_ptr());
                if rc < 0 {
                    return Err(format!("loadfile failed: {}", mpv.err(rc)));
                }
            }

            // The CGL context underlying the NSOpenGLContext — used to lock the
            // render thread's drawing against a main-thread -update during resize.
            let cgl: CglContextObj = unsafe { msg_send![gctx, CGLContextObj] };

            // -update must run on the MAIN thread (it touches AppKit/the view —
            // calling it on the render thread SIGTRAPs). Dispatch it without
            // waiting so a modal drag-resize on main can't deadlock us.
            let update_ctx = |ctx: *mut AnyObject| unsafe {
                let nil: *mut AnyObject = std::ptr::null_mut();
                let _: () = msg_send![
                    ctx,
                    performSelectorOnMainThread: sel!(update),
                    withObject: nil,
                    waitUntilDone: false
                ];
            };

            let max_secs = std::env::var("SPIKE_SECS").ok().and_then(|s| s.parse::<u64>().ok());
            let start = std::time::Instant::now();
            let mut frames: u64 = 0;
            let mut last_size = (0i32, 0i32);

            loop {
                if QUIT.load(Ordering::SeqCst) {
                    break;
                }
                // Window closed (red button)? Stop. (isVisible read off-main is
                // benign here and the canonical lightweight close-poll.)
                let visible: bool = unsafe { msg_send![win, isVisible] };
                if !visible {
                    eprintln!("[probe] window closed");
                    break;
                }
                if let Some(m) = max_secs {
                    if start.elapsed().as_secs() >= m {
                        eprintln!("[probe] auto-quit after {m}s");
                        break;
                    }
                }

                // Current backing size from the view bounds.
                let bounds: CGRect = unsafe { msg_send![view, bounds] };
                let w = (bounds.size.width as i32).max(1);
                let h = (bounds.size.height as i32).max(1);
                if (w, h) != last_size {
                    last_size = (w, h);
                    update_ctx(gctx); // tell NSOpenGL the drawable resized
                    eprintln!("[probe] resize -> {w}x{h}");
                }

                let flags = unsafe { (mpv.render_context_update)(rctx) };
                if flags & MPV_RENDER_UPDATE_FRAME != 0 {
                    unsafe {
                        // Lock the CGL context so a concurrent main-thread -update
                        // (resize) can't reconfigure the drawable mid-render.
                        CGLLockContext(cgl);
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
                        (mpv.render_context_render)(rctx, params.as_mut_ptr());
                        let _: () = msg_send![gctx, flushBuffer]; // present (≈ SwapBuffers)
                        CGLUnlockContext(cgl);
                    }
                    unsafe { (mpv.render_context_report_swap)(rctx) };
                    frames += 1;
                    if frames % 120 == 0 {
                        let fps = frames as f64 / start.elapsed().as_secs_f64();
                        eprintln!("[probe] {frames} frames ({fps:.0} fps avg), {w}x{h}");
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }

            eprintln!("[probe] tearing down… ({frames} frames)");
            unsafe {
                (mpv.render_context_free)(rctx);
                (mpv.terminate_destroy)(handle);
                let _: () = msg_send![class!(NSOpenGLContext), clearCurrentContext];
            }
            eprintln!("[probe] done — {frames} frames rendered.");
            Ok(())
        }
    }
}
