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
        ShowWindow, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_POPUP,
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
                    style: 0,
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
