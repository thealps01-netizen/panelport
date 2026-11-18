//! Custom title bar support.
//!
//! Every Panelport window runs without the system frame (`decorations(false)`); its title bar is
//! a bundled page (the launcher's top row, or the `titlebar.html` webview of a panel window).
//! The page draws the minimize, maximize and close glyphs. On Windows a small native child
//! window sits exactly over those three buttons and answers `WM_NCHITTEST` with `HTMINBUTTON`,
//! `HTMAXBUTTON` and `HTCLOSE`. That gives the buttons the system's own behaviour, including the
//! Windows 11 Snap Layouts flyout on the maximize button, which the system shows only for a
//! window that reports `HTMAXBUTTON`. The child window draws nothing
//! (`WS_EX_NOREDIRECTIONBITMAP`); it reports hover and press back to the page, which paints the
//! state. Sources: Microsoft Learn, "Support snap layouts for desktop apps on Windows 11", and
//! the same input-sink pattern in Windows Terminal (`NonClientIslandWindow`). Tauri's own
//! undecorated-resize border (`TAURI_DRAG_RESIZE_WINDOW`) is a same-thread child window that
//! returns hit-test codes the same way.

/// Width of one caption button and height of the title bar, in device-independent pixels. The
/// pages use the same numbers (`--bar-h`, `--cap-w` in app.css).
pub const BUTTON_W: f64 = 46.0;
pub const BAR_H: f64 = 44.0;

#[cfg(windows)]
use tauri::Manager;
use tauri::Window;

/// Installs the caption-button hit area on `window`; `webview` is the label of the webview that
/// draws the title bar (it receives the hover and press states). The child window must belong to
/// the thread that owns the window (the main thread): a child owned by a thread without a
/// message loop would block the window. Callers on other threads (async commands) are queued.
#[cfg(windows)]
pub fn install(window: &Window, webview: &str) {
    let w = window.clone();
    let label = webview.to_string();
    if let Err(e) = window
        .app_handle()
        .run_on_main_thread(move || win::install(&w, &label))
    {
        log::error!("installing the caption buttons: {e}");
    }
}

#[cfg(not(windows))]
pub fn install(_window: &Window, _webview: &str) {}

/// Puts the hit area back on top after child webviews were added or resized.
#[cfg(windows)]
pub fn raise(window: &Window) {
    let w = window.clone();
    if let Err(e) = window
        .app_handle()
        .run_on_main_thread(move || win::raise(&w))
    {
        log::warn!("raising the caption buttons: {e}");
    }
}

#[cfg(not(windows))]
pub fn raise(_window: &Window) {}

/// Hit-test codes the three buttons answer, read from the real window (self-test). `None` when
/// the hit area is missing.
#[cfg(windows)]
pub fn probe(window: &Window) -> Option<[isize; 3]> {
    win::probe(window)
}

#[cfg(windows)]
mod win {
    use std::sync::Once;
    use tauri::{AppHandle, Manager, Window};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
    };
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::*;

    const CLASS: PCWSTR = w!("PANELPORT_CAPTION_BUTTONS");
    const SUBCLASS_ID: usize = 0x5050_4342; // "PPCB"
    const WM_NCMOUSELEAVE: u32 = 0x02A2;

    struct Data {
        app: AppHandle,
        webview: String,
        parent: HWND,
        hover: u32,
        pressed: u32,
    }

    fn parent_of(window: &Window) -> Option<HWND> {
        match window.hwnd() {
            Ok(h) => Some(HWND(h.0 as _)),
            Err(e) => {
                log::error!("no native handle for window {}: {e}", window.label());
                None
            }
        }
    }

    fn scale(parent: HWND) -> f64 {
        let dpi = unsafe { GetDpiForWindow(parent) };
        if dpi == 0 {
            1.0
        } else {
            dpi as f64 / 96.0
        }
    }

    fn find_child(parent: HWND) -> Option<HWND> {
        unsafe { FindWindowExW(Some(parent), None, CLASS, PCWSTR::null()) }.ok()
    }

    pub fn install(window: &Window, webview: &str) {
        let Some(parent) = parent_of(window) else {
            return;
        };
        if find_child(parent).is_some() {
            return;
        }
        static REGISTER: Once = Once::new();
        let hinstance = unsafe { GetModuleHandleW(None) }.ok().map(|h| h.into());
        REGISTER.call_once(|| unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(child_proc),
                hInstance: hinstance.unwrap_or_default(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 {
                log::error!("registering the caption button class failed");
            }
        });
        unsafe {
            // The flyout and Win+Up/Down need the window to declare minimize/maximize boxes and a
            // system menu; tao may leave them out of a frameless window.
            let style = GetWindowLongPtrW(parent, GWL_STYLE);
            let wanted = (WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0 as isize;
            if style & wanted != wanted {
                SetWindowLongPtrW(parent, GWL_STYLE, style | wanted);
            }
            let data = Box::into_raw(Box::new(Data {
                app: window.app_handle().clone(),
                webview: webview.to_string(),
                parent,
                hover: 0,
                pressed: 0,
            }));
            let child = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP,
                CLASS,
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                hinstance,
                Some(data as _),
            );
            let child = match child {
                Ok(c) => c,
                Err(e) => {
                    drop(Box::from_raw(data));
                    log::error!("creating the caption button area failed: {e}");
                    return;
                }
            };
            // Fully opaque but with no redirection surface: invisible, yet it receives input.
            let _ = SetLayeredWindowAttributes(child, COLORREF(0), 255, LWA_ALPHA);
            place(parent, child);
            if !SetWindowSubclass(parent, Some(parent_proc), SUBCLASS_ID, child.0 as usize)
                .as_bool()
            {
                log::error!("watching the window size for the caption buttons failed");
            }
        }
    }

    pub fn raise(window: &Window) {
        if let Some(parent) = parent_of(window) {
            if let Some(child) = find_child(parent) {
                place(parent, child);
            }
        }
    }

    pub fn probe(window: &Window) -> Option<[isize; 3]> {
        let parent = parent_of(window)?;
        let child = find_child(parent)?;
        let mut r = RECT::default();
        unsafe { GetWindowRect(child, &mut r) }.ok()?;
        let bw = (r.right - r.left) / 3;
        let y = r.top + (r.bottom - r.top) / 2;
        let mut out = [0isize; 3];
        for (i, slot) in out.iter_mut().enumerate() {
            let x = r.left + bw * i as i32 + bw / 2;
            let lp = LPARAM(((y as u16 as u32) << 16 | (x as u16 as u32)) as isize);
            *slot = unsafe { SendMessageW(child, WM_NCHITTEST, None, Some(lp)) }.0;
        }
        Some(out)
    }

    fn place(parent: HWND, child: HWND) {
        let s = scale(parent);
        let bw = (super::BUTTON_W * s).round() as i32;
        let bh = (super::BAR_H * s).round() as i32;
        let mut rc = RECT::default();
        if unsafe { GetClientRect(parent, &mut rc) }.is_err() {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                child,
                Some(HWND_TOP),
                rc.right - 3 * bw,
                0,
                3 * bw,
                bh,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        };
    }

    fn data<'a>(child: HWND) -> Option<&'a mut Data> {
        let p = unsafe { GetWindowLongPtrW(child, GWLP_USERDATA) } as *mut Data;
        unsafe { p.as_mut() }
    }

    fn name(code: u32) -> &'static str {
        match code {
            HTMINBUTTON => "min",
            HTMAXBUTTON => "max",
            HTCLOSE => "close",
            _ => "",
        }
    }

    /// Runs a script in the title-bar page. Never from inside the window procedure itself: Tauri
    /// handles calls from the main thread immediately, which would re-enter the event loop while
    /// Windows is in the middle of a message. A short-lived thread queues it instead.
    fn eval_later(d: &Data, js: String) {
        let app = d.app.clone();
        let label = d.webview.clone();
        std::thread::spawn(move || {
            if let Some(w) = app.get_webview(&label) {
                if let Err(e) = w.eval(js) {
                    log::warn!("caption state to {label}: {e}");
                }
            }
        });
    }

    /// Tells the title-bar page which button is hovered or pressed.
    fn paint(d: &Data) {
        let js = format!(
            "window.__ppCaption&&window.__ppCaption('{}','{}')",
            name(if d.pressed != 0 { d.pressed } else { d.hover }),
            if d.pressed != 0 { "press" } else { "hover" }
        );
        eval_later(d, js);
    }

    fn maximized_to_page(d: &Data) {
        let max = unsafe { IsZoomed(d.parent) }.as_bool();
        eval_later(d, format!("window.__ppMax&&window.__ppMax({max})"));
    }

    unsafe extern "system" fn parent_proc(
        parent: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        child: usize,
    ) -> LRESULT {
        let child = HWND(child as _);
        match msg {
            WM_SIZE | WM_DPICHANGED => {
                place(parent, child);
                if let Some(d) = data(child) {
                    maximized_to_page(d);
                }
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(parent, Some(parent_proc), SUBCLASS_ID);
            }
            _ => {}
        }
        DefSubclassProc(parent, msg, wparam, lparam)
    }

    unsafe extern "system" fn child_proc(
        child: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(child, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(child, msg, wparam, lparam)
            }
            WM_NCHITTEST => {
                let mut r = RECT::default();
                if GetWindowRect(child, &mut r).is_err() || r.right <= r.left {
                    return LRESULT(HTNOWHERE as isize);
                }
                let x = (lparam.0 & 0xffff) as i16 as i32;
                let third = ((x - r.left) * 3 / (r.right - r.left)).clamp(0, 2);
                LRESULT([HTMINBUTTON, HTMAXBUTTON, HTCLOSE][third as usize] as isize)
            }
            WM_NCMOUSEMOVE => {
                if let Some(d) = data(child) {
                    let code = wparam.0 as u32;
                    if code != d.hover {
                        d.hover = code;
                        if d.pressed != 0 && d.pressed != code {
                            d.pressed = 0;
                        }
                        paint(d);
                    }
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: child,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }
                DefWindowProcW(child, msg, wparam, lparam)
            }
            WM_NCMOUSELEAVE => {
                if let Some(d) = data(child) {
                    d.hover = 0;
                    d.pressed = 0;
                    paint(d);
                }
                LRESULT(0)
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                if let Some(d) = data(child) {
                    d.pressed = wparam.0 as u32;
                    paint(d);
                }
                // Not passed on: the default handler would run its own classic button loop.
                LRESULT(0)
            }
            WM_NCLBUTTONUP => {
                if let Some(d) = data(child) {
                    let code = wparam.0 as u32;
                    let act = d.pressed == code;
                    d.pressed = 0;
                    paint(d);
                    if act {
                        let cmd = match code {
                            HTMINBUTTON => Some(SC_MINIMIZE),
                            HTMAXBUTTON if IsZoomed(d.parent).as_bool() => Some(SC_RESTORE),
                            HTMAXBUTTON => Some(SC_MAXIMIZE),
                            HTCLOSE => Some(SC_CLOSE),
                            _ => None,
                        };
                        if let Some(cmd) = cmd {
                            let _ = PostMessageW(
                                Some(d.parent),
                                WM_SYSCOMMAND,
                                WPARAM(cmd as usize),
                                LPARAM(0),
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_NCRBUTTONUP => LRESULT(0),
            WM_SETCURSOR => {
                if let Ok(c) = LoadCursorW(None, IDC_ARROW) {
                    SetCursor(Some(c));
                }
                LRESULT(1)
            }
            WM_NCDESTROY => {
                let p = GetWindowLongPtrW(child, GWLP_USERDATA) as *mut Data;
                if !p.is_null() {
                    SetWindowLongPtrW(child, GWLP_USERDATA, 0);
                    drop(Box::from_raw(p));
                }
                DefWindowProcW(child, msg, wparam, lparam)
            }
            _ => DefWindowProcW(child, msg, wparam, lparam),
        }
    }
}
