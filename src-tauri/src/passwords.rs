//! Saved passwords in panel windows.
//!
//! WebView2 ships with its password manager and form autofill switched off
//! (`IsPasswordAutosaveEnabled` / `IsGeneralAutofillEnabled` default to false). Panel windows
//! switch both on so logins typed into a panel can be saved. The data stays in Panelport's own
//! WebView2 data folder on this PC, encrypted for the Windows user; it is not synced and not
//! shared with Microsoft Edge. Bundled pages are not changed (their inputs set
//! `autocomplete="off"`).

use tauri::WebviewWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AutofillState {
    pub password_autosave: bool,
    pub general_autofill: bool,
}

/// Switches password saving and form autofill on for a panel window. Never blocks (it may be
/// called on the main thread); failures are logged.
pub fn enable(w: &WebviewWindow) {
    #[cfg(windows)]
    {
        let label = w.label().to_string();
        let res = w.with_webview(move |pv| {
            // SAFETY: COM calls on the WebView2 controller, made on the UI thread Tauri gives us.
            if let Err(e) = unsafe { settings_call(&pv, Some(true)) } {
                log::error!("enabling password saving in {label} failed: {e}");
            }
        });
        if let Err(e) = res {
            log::error!("enabling password saving: {e}");
        }
    }
    #[cfg(not(windows))]
    let _ = w;
}

/// Reads the two settings back. Blocks until the UI thread answers, so it must not be called
/// on the main thread.
#[cfg(windows)]
pub fn read_blocking(w: &WebviewWindow) -> Result<AutofillState, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    w.with_webview(move |pv| {
        // SAFETY: as in `enable`.
        let _ = tx.send(unsafe { settings_call(&pv, None) });
    })
    .map_err(|e| format!("with_webview: {e}"))?;
    rx.recv_timeout(std::time::Duration::from_secs(15))
        .map_err(|_| "WebView2 did not answer within 15 s".to_string())?
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
