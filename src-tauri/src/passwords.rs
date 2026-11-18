//! Saved passwords in panel webviews.
//!
//! WebView2 ships with its password manager and form autofill switched off
//! (`IsPasswordAutosaveEnabled` / `IsGeneralAutofillEnabled` default to false). Panel webviews
//! switch both on (unless the user turned "save passwords" off) so logins typed into a panel can
//! be saved. The data stays in Panelport's own WebView2 data folder on this PC, protected by
//! WebView2 for the Windows user; it is not synced and not shared with Microsoft Edge. Bundled
//! pages are not changed (their inputs set `autocomplete="off"`).

use tauri::Webview;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AutofillState {
    pub password_autosave: bool,
    pub general_autofill: bool,
}

/// Switches password saving and form autofill on for a panel webview. Never blocks.
pub fn enable(w: &Webview) {
    set(w, true)
}

/// Switches password saving and form autofill on or off for a panel webview. Never blocks (it
/// may be called on the main thread); failures are logged.
pub fn set(w: &Webview, on: bool) {
    #[cfg(windows)]
    {
        let label = w.label().to_string();
        let res = w.with_webview(move |pv| {
            // SAFETY: COM calls on the WebView2 controller, on the UI thread Tauri gives us.
            if let Err(e) = unsafe { settings_call(&pv, Some(on)) } {
                log::error!("setting password saving in {label} failed: {e}");
            }
        });
        if let Err(e) = res {
            log::error!("setting password saving: {e}");
        }
    }
    #[cfg(not(windows))]
    let _ = (w, on);
}

/// Reads the two settings back. Blocks until the UI thread answers, so it must not be called
/// on the main thread.
#[cfg(windows)]
pub fn read_blocking(w: &Webview) -> Result<AutofillState, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    w.with_webview(move |pv| {
        // SAFETY: as in `set`.
        let _ = tx.send(unsafe { settings_call(&pv, None) });
    })
    .map_err(|e| format!("with_webview: {e}"))?;
    rx.recv_timeout(std::time::Duration::from_secs(15))
        .map_err(|_| "WebView2 did not answer within 15 s".to_string())?
}

#[cfg(windows)]
type Done = std::sync::Arc<std::sync::Mutex<Option<Box<dyn FnOnce(Result<(), String>) + Send>>>>;

/// Deletes the passwords WebView2 saved in Panelport's data folder (all panels share it).
/// `done` gets the outcome. Windows only.
#[cfg(windows)]
pub fn clear(w: &Webview, done: impl FnOnce(Result<(), String>) + Send + 'static) {
    let done: Box<dyn FnOnce(Result<(), String>) + Send> = Box::new(done);
    let slot: Done = std::sync::Arc::new(std::sync::Mutex::new(Some(done)));
    let fail = slot.clone();
    let res = w.with_webview(move |pv| {
        // SAFETY: as in `set`.
        if let Err(e) = unsafe { clear_call(&pv, slot) } {
            if let Some(f) = fail.lock().ok().and_then(|mut d| d.take()) {
                f(Err(e));
            }
        }
    });
    if let Err(e) = res {
        log::error!("deleting saved passwords: {e}");
    }
}

#[cfg(windows)]
unsafe fn clear_call(pv: &tauri::webview::PlatformWebview, done: Done) -> Result<(), String> {
    use webview2_com::ClearBrowsingDataCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile2, ICoreWebView2_13, COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE,
    };
    use windows_core::Interface;
    let core: ICoreWebView2_13 = pv
        .controller()
        .CoreWebView2()
        .and_then(|c| c.cast())
        .map_err(|e| format!("ICoreWebView2_13: {e}"))?;
    let profile: ICoreWebView2Profile2 = core
        .Profile()
        .and_then(|p| p.cast())
        .map_err(|e| format!("ICoreWebView2Profile2: {e}"))?;
    let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
        if let Some(f) = done.lock().ok().and_then(|mut d| d.take()) {
            f(result.map_err(|e| e.to_string()));
        }
        Ok(())
    }));
    profile
        .ClearBrowsingData(COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE, &handler)
        .map_err(|e| format!("ClearBrowsingData: {e}"))
}

#[cfg(windows)]
unsafe fn settings_call(
    pv: &tauri::webview::PlatformWebview,
    set: Option<bool>,
) -> Result<AutofillState, String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use windows_core::{Interface, BOOL};
    let e = |what: &str, err: windows_core::Error| format!("{what}: {err}");
    let core = pv
        .controller()
        .CoreWebView2()
        .map_err(|x| e("CoreWebView2", x))?;
    let settings: ICoreWebView2Settings4 = core
        .Settings()
        .map_err(|x| e("Settings", x))?
        .cast()
        .map_err(|x| e("ICoreWebView2Settings4", x))?;
    if let Some(v) = set {
        settings
            .SetIsPasswordAutosaveEnabled(v)
            .map_err(|x| e("SetIsPasswordAutosaveEnabled", x))?;
        settings
            .SetIsGeneralAutofillEnabled(v)
            .map_err(|x| e("SetIsGeneralAutofillEnabled", x))?;
    }
    let mut pw = BOOL::default();
    settings
        .IsPasswordAutosaveEnabled(&mut pw)
        .map_err(|x| e("IsPasswordAutosaveEnabled", x))?;
    let mut af = BOOL::default();
    settings
        .IsGeneralAutofillEnabled(&mut af)
        .map_err(|x| e("IsGeneralAutofillEnabled", x))?;
    Ok(AutofillState {
        password_autosave: pw.as_bool(),
        general_autofill: af.as_bool(),
    })
}
