pub mod player;

/// Multi-instance render compositor (Milestone 37). Windows-only for now —
/// macOS keeps the M38 per-player render thread until its multi-view follow-up.
#[cfg(target_os = "windows")]
pub mod compositor;

/// Native window hosting mpv's video output.
///
/// Why a separate *top-level* window instead of a child of the app window:
/// a child window underneath the (full-size) WebView gets clipped out of
/// DWM composition entirely — its swapchain is never visible. Top-level
/// windows compose independently, so the video window is glued directly
/// *behind* the main window in the desktop z-order. The main window is
/// transparent (tao's DWM blur-behind), and the HTML page only goes
/// transparent over the player area once the stream is actually delivering
/// frames, so the video shows through exactly there and nothing else does.
#[cfg(target_os = "windows")]
pub mod video_host {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, CreateSolidBrush};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetClientRect, IsIconic, RegisterClassW, SetWindowPos,
        ShowWindow, CS_OWNDC, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Window class with a soft dark background (zinc-900, #18181b) so the
    /// surface is easy on the eyes whenever mpv has no frame to show.
    fn class_name() -> &'static Vec<u16> {
        static CLASS: OnceLock<Vec<u16>> = OnceLock::new();
        CLASS.get_or_init(|| {
            let name = wide("ProsceniumVideoHost");
            unsafe {
                let class = WNDCLASSW {
                    // CS_OWNDC: a private DC the render thread can bind a GL
                    // context to once (set pixel format + WGL context) and keep
                    // for the window's lifetime (Milestone 38).
                    style: CS_OWNDC,
                    lpfnWndProc: Some(DefWindowProcW),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: std::ptr::null_mut(),
                    hIcon: std::ptr::null_mut(),
                    hCursor: std::ptr::null_mut(),
                    // COLORREF is 0x00BBGGRR: #18181b -> blue 0x1b, green/red 0x18.
                    hbrBackground: CreateSolidBrush(0x001B1818),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: name.as_ptr(),
                };
                RegisterClassW(&class);
            }
            name
        })
    }

    /// The app window's client area in screen coordinates.
    fn client_rect_on_screen(parent: isize) -> (i32, i32, i32, i32) {
        unsafe {
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(parent as HWND, &mut rect);
            let mut origin = POINT { x: 0, y: 0 };
            ClientToScreen(parent as HWND, &mut origin);
            (
                origin.x,
                origin.y,
                rect.right - rect.left,
                rect.bottom - rect.top,
            )
        }
    }

    /// Create the video window over the app window's client area and slot
    /// it directly below the app window in the desktop z-order. Must be
    /// called on the thread that owns `parent` (the main thread).
    pub fn create(parent: isize) -> Result<isize, String> {
        let (x, y, width, height) = client_rect_on_screen(parent);
        let hwnd = unsafe {
            CreateWindowExW(
                // Tool window: no taskbar entry; never steals activation.
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name().as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                x,
                y,
                width,
                height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err("failed to create the video host window".into());
        }
        fit_to_parent(hwnd as isize, parent);
        Ok(hwnd as isize)
    }

    /// Glue the video window to the app window: match the client area and
    /// keep it immediately below the app window in the z-order. Called on
    /// move/resize/focus and periodically from the player's state callback
    /// (self-healing if another window slips in between).
    pub fn fit_to_parent(host: isize, parent: isize) {
        unsafe {
            if IsIconic(parent as HWND) != 0 {
                ShowWindow(host as HWND, SW_HIDE);
                return;
            }
            ShowWindow(host as HWND, SW_SHOWNOACTIVATE);
            let (x, y, width, height) = client_rect_on_screen(parent);
            // hWndInsertAfter = parent → host is placed directly below it.
            SetWindowPos(
                host as HWND,
                parent as HWND,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
    }

}

/// Windows OpenGL plumbing for the libmpv render API (Milestone 38). The
/// `mpv::player` render thread owns a WGL context on the video-host window and
/// drives `mpv_render_context_render` into it. Handles are passed as `isize`
/// so they cross from the spawning thread cleanly. Ported from the Spike B
/// example (`examples/render_api_spike.rs`), which validated this end-to-end.
#[cfg(target_os = "windows")]
pub mod render_win {
    use std::ffi::{c_char, c_void};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{GetDC, HDC};
    use windows_sys::Win32::Graphics::OpenGL::{
        wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent, ChoosePixelFormat,
        SetPixelFormat, SwapBuffers, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
        PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
    };
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    static OPENGL32: OnceLock<isize> = OnceLock::new();

    /// GL proc loader handed to mpv: `wglGetProcAddress` first (extensions),
    /// then the `opengl32.dll` module for the GL 1.1 core entry points wgl
    /// won't return. Must run with a GL context current (it is, on the render
    /// thread). Matches the `MpvOpenglInitParams::get_proc_address` ABI.
    pub unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
        if let Some(f) = wglGetProcAddress(name as *const u8) {
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

    /// Create and make-current a legacy WGL context on the host window's private
    /// DC (the window class must have `CS_OWNDC`). Runs on the render thread so
    /// the context is current there. Returns `(HDC, HGLRC)` as `isize`.
    pub unsafe fn init_gl(hwnd: isize) -> Result<(isize, isize), String> {
        let hdc = GetDC(hwnd as HWND);
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
        Ok((hdc as isize, hglrc as isize))
    }

    /// Present the rendered frame (vsync-paced).
    pub unsafe fn swap_buffers(hdc: isize) {
        SwapBuffers(hdc as HDC);
    }

    /// Current client size of the host window (clamped to ≥ 1×1). Queried each
    /// frame so resizes (driven by `SetWindowPos` on the UI thread) are picked
    /// up with no cross-thread signaling.
    pub fn client_size(hwnd: isize) -> (i32, i32) {
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd as HWND, &mut r);
            ((r.right - r.left).max(1), (r.bottom - r.top).max(1))
        }
    }

    /// Release the GL context. The DC is owned by the window (`CS_OWNDC`), so it
    /// isn't released here.
    pub unsafe fn destroy_gl(_hdc: isize, hglrc: isize) {
        wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
        wglDeleteContext(hglrc as *mut c_void);
    }

    // --- OpenGL entry points for the compositor (Milestone 37) ---
    //
    // Multi-view renders each mpv tile into its own texture-backed FBO, then
    // `glBlitFramebuffer`s each into its sub-rectangle of the window. These are
    // the GL functions that needs — loaded via `get_proc_address` (wgl returns
    // 3.0+ entry points; the opengl32 fallback covers the 1.1 ones). GL uses the
    // `system` (APIENTRY/stdcall) calling convention on Windows.

    pub const GL_TEXTURE_2D: u32 = 0x0DE1;
    pub const GL_RGBA: u32 = 0x1908;
    pub const GL_RGBA8: u32 = 0x8058;
    pub const GL_UNSIGNED_BYTE: u32 = 0x1401;
    pub const GL_FRAMEBUFFER: u32 = 0x8D40;
    pub const GL_READ_FRAMEBUFFER: u32 = 0x8CA8;
    pub const GL_DRAW_FRAMEBUFFER: u32 = 0x8CA9;
    pub const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
    pub const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
    pub const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
    pub const GL_LINEAR: i32 = 0x2601;
    pub const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
    pub const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
    pub const GL_SCISSOR_TEST: u32 = 0x0C11;

    type GenFn = unsafe extern "system" fn(i32, *mut u32);
    type DelFn = unsafe extern "system" fn(i32, *const u32);
    type BindFn = unsafe extern "system" fn(u32, u32);

    /// The compositor's OpenGL function table (resolved once per GL context).
    #[allow(dead_code)]
    pub struct GlFns {
        gen_framebuffers: GenFn,
        delete_framebuffers: DelFn,
        bind_framebuffer: BindFn,
        gen_textures: GenFn,
        delete_textures: DelFn,
        bind_texture: BindFn,
        tex_image_2d:
            unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void),
        tex_parameteri: unsafe extern "system" fn(u32, u32, i32),
        framebuffer_texture_2d: unsafe extern "system" fn(u32, u32, u32, u32, i32),
        check_framebuffer_status: unsafe extern "system" fn(u32) -> u32,
        blit_framebuffer:
            unsafe extern "system" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32),
        clear: unsafe extern "system" fn(u32),
        clear_color: unsafe extern "system" fn(f32, f32, f32, f32),
        viewport: unsafe extern "system" fn(i32, i32, i32, i32),
        disable: unsafe extern "system" fn(u32),
    }

    impl GlFns {
        /// Resolve the entry points. A GL context must be current on this thread.
        pub unsafe fn load() -> Result<Self, String> {
            macro_rules! sym {
                ($n:literal) => {{
                    let p = get_proc_address(
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
        pub unsafe fn create_fbo(&self, w: i32, h: i32) -> Result<(u32, u32), String> {
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
            (self.framebuffer_texture_2d)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                tex,
                0,
            );
            let status = (self.check_framebuffer_status)(GL_FRAMEBUFFER);
            (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
            if status != GL_FRAMEBUFFER_COMPLETE {
                (self.delete_framebuffers)(1, &fbo);
                (self.delete_textures)(1, &tex);
                return Err(format!("framebuffer incomplete: 0x{status:X}"));
            }
            Ok((fbo, tex))
        }

        pub unsafe fn delete_fbo(&self, fbo: u32, tex: u32) {
            (self.delete_framebuffers)(1, &fbo);
            (self.delete_textures)(1, &tex);
        }

        /// Bind the default framebuffer, reset viewport/scissor, and clear it to
        /// `rgba` (the backdrop shown in gaps between tiles).
        pub unsafe fn begin_window_frame(&self, cw: i32, ch: i32, rgba: (f32, f32, f32, f32)) {
            (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
            (self.disable)(GL_SCISSOR_TEST);
            (self.viewport)(0, 0, cw.max(1), ch.max(1));
            (self.clear_color)(rgba.0, rgba.1, rgba.2, rgba.3);
            (self.clear)(GL_COLOR_BUFFER_BIT);
        }

        /// Blit tile FBO `src_fbo` (its full `sw`×`sh`) into the default
        /// framebuffer's destination rect (GL bottom-left coordinates).
        pub unsafe fn blit_to_window(
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
}

/// Native window hosting mpv's video output on macOS.
///
/// This libmpv build (Homebrew, Vulkan/Metal GPU contexts only) has no
/// Cocoa-GL context, so `--wid` embedding into one of our own `NSView`s is not
/// supported — mpv always renders into a window it creates itself. So, exactly
/// like the Windows path conceptually, we keep that as a *separate* native
/// window and glue it directly behind the (transparent) app window in the
/// z-order: mpv's window is made borderless, demoted to a child window ordered
/// *below* the main window, and sized to the main window's content area. The
/// HTML page only paints transparent over the player area once frames flow
/// (the `macOSPrivateApi` transparent-background API, enabled in
/// tauri.conf.json), so the video shows through exactly there.
#[cfg(target_os = "macos")]
pub mod video_host {
    use objc2::encode::{Encode, Encoding};
    use objc2::runtime::AnyObject;
    use objc2::msg_send;

    // Minimal CoreGraphics geometry mirrors. `Encode` lets objc2's `msg_send!`
    // pass/return them by value with the right ABI (CGFloat is f64 on 64-bit).
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

    // NSWindowStyleMaskBorderless = 0; NSWindowOrderingMode::Below = -1.
    const NS_WINDOW_STYLE_BORDERLESS: u64 = 0;
    const NS_WINDOW_BELOW: i64 = -1;

    /// Glue our video window (`mpv`) behind the app window (`main`): strip its
    /// border, match the app window's level, ignore mouse events (the main
    /// window handles all input), and attach it as a child ordered below the
    /// main window so it tracks moves automatically. Must run on the main
    /// thread.
    pub fn glue(main: isize, mpv: isize) {
        unsafe {
            let main_w = main as *mut AnyObject;
            let mpv_w = mpv as *mut AnyObject;
            let _: () = msg_send![mpv_w, setStyleMask: NS_WINDOW_STYLE_BORDERLESS];
            let level: i64 = msg_send![main_w, level];
            let _: () = msg_send![mpv_w, setLevel: level];
            let _: () = msg_send![mpv_w, setIgnoresMouseEvents: true];
            let _: () = msg_send![mpv_w, setMovable: false];
            let _: () = msg_send![main_w, addChildWindow: mpv_w, ordered: NS_WINDOW_BELOW];
        }
        fit_to_parent(mpv, main);
    }

    /// Size mpv's window to the app window's *content* area (below the
    /// titlebar), in screen coordinates. Called on resize / scale-factor /
    /// fullscreen changes; child-window attachment handles plain moves.
    pub fn fit_to_parent(mpv: isize, main: isize) {
        unsafe {
            let main_w = main as *mut AnyObject;
            let mpv_w = mpv as *mut AnyObject;
            let frame: CGRect = msg_send![main_w, frame];
            let content: CGRect = msg_send![main_w, contentRectForFrameRect: frame];
            let _: () = msg_send![mpv_w, setFrame: content, display: true];
        }
    }
}

/// macOS OpenGL plumbing for the libmpv render API (Milestone 38). We create our
/// own borderless host window with an `NSOpenGLContext`, glue it behind the
/// transparent app window (`video_host::glue`), and the `mpv::player` render
/// thread draws `mpv_render_context_render` into it. This *replaces* the old
/// "mpv owns its window; we find + demote it" hack. Ported from the validated
/// probe (`examples/render_api_probe_macos.rs`, Tier 2). All AppKit object
/// creation here must run on the main thread; the render loop helpers run on the
/// render thread (the context is made current there).
#[cfg(target_os = "macos")]
pub mod render_mac {
    use objc2::encode::{Encode, Encoding};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, sel};
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::sync::OnceLock;

    // CoreGraphics geometry mirrors (same pattern as `video_host`).
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

    // dlopen/dlsym from libSystem (implicitly linked) for the GL proc loader.
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_LAZY: c_int = 0x1;
    const RTLD_LOCAL: c_int = 0x4;

    // CGL lock/unlock from the (deprecated but present) OpenGL framework. Used to
    // serialize the render thread's drawing against a main-thread `-update` so a
    // resize can't reconfigure the drawable mid-render.
    #[link(name = "OpenGL", kind = "framework")]
    extern "C" {
        fn CGLLockContext(ctx: *mut c_void) -> c_int;
        fn CGLUnlockContext(ctx: *mut c_void) -> c_int;
    }

    // NSWindow / NSOpenGL constants.
    const NS_WINDOW_STYLE_BORDERLESS: u64 = 0;
    const NS_BACKING_STORE_BUFFERED: u64 = 2;
    const NSOPENGL_PFA_ACCELERATED: u32 = 73;
    const NSOPENGL_PFA_DOUBLE_BUFFER: u32 = 5;
    const NSOPENGL_PFA_COLOR_SIZE: u32 = 8;
    const NSOPENGL_PFA_OPENGL_PROFILE: u32 = 99;
    const NSOPENGL_PROFILE_VERSION_3_2_CORE: u32 = 0x3200;

    /// GL proc loader handed to mpv: dlopen the OpenGL framework once, then dlsym
    /// each symbol. Matches `MpvOpenglInitParams::get_proc_address`.
    static OPENGL_FW: OnceLock<usize> = OnceLock::new();
    pub unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
        let handle = *OPENGL_FW.get_or_init(|| {
            let path = CString::new(
                "/System/Library/Frameworks/OpenGL.framework/Versions/Current/OpenGL",
            )
            .unwrap();
            dlopen(path.as_ptr(), RTLD_LAZY | RTLD_LOCAL) as usize
        });
        if handle == 0 {
            return std::ptr::null_mut();
        }
        dlsym(handle as *mut c_void, name)
    }

    /// Create the borderless host window + an `NSOpenGLContext` (3.2 core) bound
    /// to its content view, and glue it behind the app window. Returns
    /// `(window, context, view)` as `isize`. **Must run on the main thread.**
    pub fn create_gl_host(main: isize) -> Result<(isize, isize, isize), String> {
        unsafe {
            // Provisional size; `glue` fits it to the app window's content area.
            let rect = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize { width: 1280.0, height: 720.0 },
            };
            let win: *mut AnyObject = msg_send![class!(NSWindow), alloc];
            let win: *mut AnyObject = msg_send![
                win,
                initWithContentRect: rect,
                styleMask: NS_WINDOW_STYLE_BORDERLESS,
                backing: NS_BACKING_STORE_BUFFERED,
                defer: false
            ];
            if win.is_null() {
                return Err("NSWindow init failed".into());
            }
            let _: () = msg_send![win, setReleasedWhenClosed: false];

            let view: *mut AnyObject = msg_send![win, contentView];
            // 1:1 backing (no retina doubling) keeps the FBO size = point size, so
            // the render thread can use the view bounds as the FBO dimensions.
            let _: () = msg_send![view, setWantsBestResolutionOpenGLSurface: false];

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
            let nil: *mut AnyObject = std::ptr::null_mut();
            let ctx: *mut AnyObject = msg_send![class!(NSOpenGLContext), alloc];
            let ctx: *mut AnyObject = msg_send![ctx, initWithFormat: pf, shareContext: nil];
            if ctx.is_null() {
                return Err("NSOpenGLContext init failed".into());
            }
            let _: () = msg_send![ctx, setView: view];

            // Demote behind the app window: borderless, child ordered below,
            // ignores mouse, fitted to the content area (reused from the old path).
            super::video_host::glue(main, win as isize);

            Ok((win as isize, ctx as isize, view as isize))
        }
    }

    /// Make the context current on the calling (render) thread.
    pub fn make_current(ctx: isize) {
        unsafe {
            let c = ctx as *mut AnyObject;
            let _: () = msg_send![c, makeCurrentContext];
        }
    }

    /// The underlying `CGLContextObj`, used for lock/unlock around a render.
    pub fn cgl_context(ctx: isize) -> isize {
        unsafe {
            let c = ctx as *mut AnyObject;
            let cgl: *mut c_void = msg_send![c, CGLContextObj];
            cgl as isize
        }
    }

    /// Present the rendered frame (the NSOpenGL analogue of `SwapBuffers`).
    pub fn flush_buffer(ctx: isize) {
        unsafe {
            let c = ctx as *mut AnyObject;
            let _: () = msg_send![c, flushBuffer];
        }
    }

    pub fn lock(cgl: isize) {
        unsafe {
            CGLLockContext(cgl as *mut c_void);
        }
    }

    pub fn unlock(cgl: isize) {
        unsafe {
            CGLUnlockContext(cgl as *mut c_void);
        }
    }

    /// Release the context on the render thread at teardown.
    pub fn clear_current() {
        unsafe {
            let _: () = msg_send![class!(NSOpenGLContext), clearCurrentContext];
        }
    }

    /// Tell NSOpenGL its drawable resized. `-update` touches AppKit, so it must
    /// run on the main thread; dispatch without waiting so a modal drag-resize on
    /// the main thread can't deadlock the render thread.
    pub fn update_on_main(ctx: isize) {
        unsafe {
            let c = ctx as *mut AnyObject;
            let nil: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                c,
                performSelectorOnMainThread: sel!(update),
                withObject: nil,
                waitUntilDone: false
            ];
        }
    }

    /// Current backing size of the host view (clamped to ≥ 1×1).
    pub fn view_size(view: isize) -> (i32, i32) {
        unsafe {
            let v = view as *mut AnyObject;
            let b: CGRect = msg_send![v, bounds];
            ((b.size.width as i32).max(1), (b.size.height as i32).max(1))
        }
    }
}
