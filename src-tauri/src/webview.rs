//! WebView2 설정 — 개인정보가 브라우저 프로필에 남지 않게.
//!
//! WebView2 는 기본으로 **입력한 값을 자동완성용으로 저장**한다(General Autofill). 그대로 두면
//! 증명서 작성 화면에 친 주민번호·주소가 `%LOCALAPPDATA%\…\EBWebView` 프로필에 남을 수 있다.
//! 앱을 켤 때 이것과 비밀번호 저장을 끈다. 화면의 입력 칸에도 `autoComplete="off"` 를 둔다(이중).

use std::sync::{Arc, Mutex};

use tauri::Manager;

/// 끄기에 성공했는가. `None` = 아직 모름, `Some(false)` = 실패(화면이 알린다).
pub type AutofillState = Arc<Mutex<Option<bool>>>;

pub fn disable_autofill(app: &tauri::App, state: AutofillState) {
    let Some(window) = app.get_webview_window("main") else {
        *state.lock().unwrap_or_else(|e| e.into_inner()) = Some(false);
        return;
    };
    #[cfg(windows)]
    {
        let s = state.clone();
        let r = window.with_webview(move |wv| {
            let ok = unsafe { apply(&wv.controller()) };
            *s.lock().unwrap_or_else(|e| e.into_inner()) = Some(ok);
        });
        if r.is_err() {
            *state.lock().unwrap_or_else(|e| e.into_inner()) = Some(false);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        *state.lock().unwrap_or_else(|e| e.into_inner()) = Some(false);
    }
}

#[cfg(windows)]
unsafe fn apply(controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller) -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use windows::core::Interface;

    let Ok(core) = controller.CoreWebView2() else { return false };
    let Ok(settings) = core.Settings() else { return false };
    let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() else { return false };
    if s4.SetIsGeneralAutofillEnabled(false).is_err() || s4.SetIsPasswordAutosaveEnabled(false).is_err() {
        return false;
    }
    // 실제로 꺼졌는지 다시 읽어 확인한다
    let mut general = windows::core::BOOL::default();
    let mut password = windows::core::BOOL::default();
    s4.IsGeneralAutofillEnabled(&mut general).is_ok()
        && s4.IsPasswordAutosaveEnabled(&mut password).is_ok()
        && !general.as_bool()
        && !password.as_bool()
}
