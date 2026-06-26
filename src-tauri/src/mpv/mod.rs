pub mod player;

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
    use objc2::{class, msg_send};

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

    /// Locate mpv's own video window: the one visible app window that isn't the
    /// main (`main`) window. Returns its `NSWindow` pointer, or 0 if it hasn't
    /// been created yet. Must run on the main (AppKit) thread.
    pub fn find_video_window(main: isize) -> isize {
        unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let windows: *mut AnyObject = msg_send![app, windows];
            let count: u64 = msg_send![windows, count];
            for i in 0..count {
                let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
                if w.is_null() || w as isize == main {
                    continue;
                }
                let visible: bool = msg_send![w, isVisible];
                if visible {
                    return w as isize;
                }
            }
            0
        }
    }

    /// Glue mpv's window (`mpv`) behind the app window (`main`): strip its
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
