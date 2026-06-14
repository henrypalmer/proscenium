//! macOS playback verification harness (not part of the app).
//!
//! Exercises the exact code path the app uses to render video on macOS:
//! create a real NSWindow, hand its content view to `mpv::video_host::create`
//! (which inserts the video NSView behind it), build an `MpvPlayer` pointed at
//! that view via `wid`, play a stream, and prove three things:
//!
//!   1. frames advance (decode + playback running),
//!   2. mpv did NOT spawn its own fallback window (so it rendered into *our*
//!      embedded view — the app window count stays at 1), and
//!   3. a frame can be captured from the video output to a PNG.
//!
//! Run with the stream URL as the only argument:
//!   cargo run --example macos_video_check -- "https://host/live/u/p/123.ts"

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2::encode::{Encode, Encoding};
use proscenium_lib::mpv::player::{MpvConfig, MpvPlayer};
use std::ffi::c_void;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: *const c_void;
    fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source: bool) -> i32;
}

fn main() {
    let url = std::env::args().nth(1).expect("usage: macos_video_check <stream-url>");
    let out_png = "/tmp/prosc-macos-frame.png";
    let _ = std::fs::remove_file(out_png);

    unsafe {
        // Minimal Cocoa app + a real titled window, like the app shell.
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setActivationPolicy: 0i64]; // Regular
        let nil: *mut AnyObject = std::ptr::null_mut();

        let rect = CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 640.0, height: 360.0 } };
        let window: *mut AnyObject = msg_send![class!(NSWindow), alloc];
        // Titled | Closable | Resizable
        let window: *mut AnyObject =
            msg_send![window, initWithContentRect: rect, styleMask: 11u64, backing: 2u64, defer: false];
        let _: () = msg_send![window, center];
        let _: () = msg_send![window, makeKeyAndOrderFront: nil];
        let _: () = msg_send![app, activateIgnoringOtherApps: true];

        let main = window as isize;

        // Build the player on a BACKGROUND thread, exactly like the app's async
        // playback command. mpv's macOS `force-window` creates its window by
        // dispatch_sync to the main queue; building it on THIS (main) thread
        // would deadlock — the real app never does that.
        let (ptx, prx) = std::sync::mpsc::channel::<Arc<MpvPlayer>>();
        let url2 = url.clone();
        std::thread::spawn(move || {
            let player = MpvPlayer::new(
                MpvConfig { wid: None, hwdec: true, headless: false },
                Box::new(|_state| {}),
            )
            .expect("MpvPlayer::new failed");
            player.load_url(&url2, None).expect("load_url failed");
            ptx.send(player).unwrap();
        });

        // Wait for the player handle while pumping the main run loop so mpv's
        // main-queue work (window creation) actually runs.
        let player;
        let init_deadline = Instant::now() + Duration::from_secs(15);
        loop {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
            if let Ok(p) = prx.try_recv() {
                player = p;
                break;
            }
            if Instant::now() > init_deadline {
                eprintln!("TIMEOUT: player never initialized");
                std::process::exit(5);
            }
        }
        eprintln!("player initialized");

        // Pump until mpv's window appears (glue it) and frames flow.
        let mut mpv_win = 0isize;
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut next_log = Instant::now() + Duration::from_secs(1);
        let mut last;
        loop {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
            if mpv_win == 0 {
                mpv_win = proscenium_lib::mpv::video_host::find_video_window(main);
                if mpv_win != 0 {
                    proscenium_lib::mpv::video_host::glue(main, mpv_win);
                    eprintln!("glued mpv window {mpv_win:#x} behind main {main:#x}");
                }
            }
            last = player.get_state();
            if let Some(err) = &last.error {
                eprintln!("PLAYBACK ERROR: {err}");
                std::process::exit(2);
            }
            if Instant::now() > next_log {
                eprintln!(
                    "  ...pos={:.2} playing={} buffering={} paused={} dur={:?}",
                    last.position, last.playing, last.buffering, last.paused, last.duration
                );
                next_log = Instant::now() + Duration::from_secs(1);
            }
            if mpv_win != 0 && last.position > 1.0 {
                break;
            }
            if Instant::now() > deadline {
                eprintln!(
                    "TIMEOUT: mpv_win={mpv_win:#x} pos={:.2} (window/frames never ready)",
                    last.position
                );
                std::process::exit(3);
            }
        }

        // Verify the glue: mpv window is a borderless child of `main`, ordered
        // below it, sized to the content area.
        let parent: *mut AnyObject = msg_send![mpv_win as *mut AnyObject, parentWindow];
        let style: u64 = msg_send![mpv_win as *mut AnyObject, styleMask];
        let mpv_frame: CGRect = msg_send![mpv_win as *mut AnyObject, frame];
        let main_frame: CGRect = msg_send![window, frame];
        let content: CGRect = msg_send![window, contentRectForFrameRect: main_frame];
        let parent_ok = parent as isize == main;
        let borderless = style == 0;
        let fitted = (mpv_frame.size.width - content.size.width).abs() < 2.0
            && (mpv_frame.size.height - content.size.height).abs() < 2.0;
        eprintln!(
            "glue check: parent_ok={parent_ok} borderless={borderless} fitted={fitted} (mpv {}x{} vs content {}x{})",
            mpv_frame.size.width, mpv_frame.size.height, content.size.width, content.size.height
        );

        // Give the vo a moment to present, then capture a frame from output.
        for _ in 0..10 {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
        }
        let shot = player.screenshot_to_file(out_png, false);
        for _ in 0..10 {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
        }

        let png_len = std::fs::metadata(out_png).map(|m| m.len()).unwrap_or(0);

        // Enumerate every NSApp window so we can tell an mpv fallback window
        // apart from our own (and any benign helper).
        let windows: *mut AnyObject = msg_send![app, windows];
        let count: u64 = msg_send![windows, count];
        eprintln!("--- NSApp windows ({count}) ---");
        for i in 0..count {
            let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
            let cls: *mut AnyObject = msg_send![w, className];
            let cls_utf8: *const std::os::raw::c_char = msg_send![cls, UTF8String];
            let cls_str = std::ffi::CStr::from_ptr(cls_utf8).to_string_lossy();
            let title: *mut AnyObject = msg_send![w, title];
            let title_str = if title.is_null() {
                "<nil>".to_string()
            } else {
                let t: *const std::os::raw::c_char = msg_send![title, UTF8String];
                std::ffi::CStr::from_ptr(t).to_string_lossy().into_owned()
            };
            let visible: bool = msg_send![w, isVisible];
            let f: CGRect = msg_send![w, frame];
            eprintln!(
                "  [{i}] class={cls_str} title='{title_str}' visible={visible} frame={}x{}",
                f.size.width, f.size.height
            );
        }

        eprintln!("--- macOS playback verification ---");
        eprintln!("position:        {:.2}s (advancing => decoding+playing)", last.position);
        eprintln!("duration:        {:?}", last.duration);
        eprintln!("audio tracks:    {}", last.audio_tracks.len());
        eprintln!("hwdec-current:   {:?}", last.hwdec_current);
        eprintln!("screenshot:      {:?}, png_bytes={png_len}", shot);

        let frame_ok = png_len > 2000; // a real frame is many KB
        if last.position > 1.0 && parent_ok && borderless && fitted && frame_ok && last.error.is_none()
        {
            eprintln!("RESULT: PASS — video plays and its window is glued behind the app window.");
        } else {
            eprintln!(
                "RESULT: FAIL — parent_ok={parent_ok} borderless={borderless} fitted={fitted} frame_ok={frame_ok} pos={:.2}",
                last.position
            );
            std::process::exit(1);
        }
    }
}
