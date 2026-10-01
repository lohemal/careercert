//! WebView2 설정 — 개인정보가 브라우저 프로필에 남지 않게, 미리보기에서 정식 출력이 새지 않게.
//!
//! * **입력 자동완성 저장 끄기**: WebView2 는 기본으로 입력값을 자동완성용으로 저장한다(General
//!   Autofill). 그대로 두면 증명서 작성 화면에 친 주민번호·주소가 `%LOCALAPPDATA%\…\EBWebView`
//!   프로필에 남을 수 있다. 창마다(메인·출력) 이것과 비밀번호 저장을 끈다. 입력 칸에도 `autoComplete="off"`.
//! * **PDF 뷰어의 저장·인쇄 버튼 숨기기 + 브라우저 단축키(Ctrl+P·Ctrl+S 등) 끄기** (메인 창):
//!   발급 전 미리보기 PDF 를 정식 출력처럼 저장·인쇄하지 못하게 한다(Phase 5 결정).
//!   그래도 새어 나가면 미리보기에는 '발급 전 미리보기' 워터마크가 있다.

use std::sync::{Arc, Mutex};

use tauri::{Manager, WebviewWindow};

/// 끄기에 성공했는가. `None` = 아직 모름, `Some(false)` = 실패(화면이 알린다).
pub type AutofillState = Arc<Mutex<Option<bool>>>;

fn put(state: &AutofillState, v: bool) {
    *state.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
}

/// 메인 창: 자동완성 저장 끄기 + 미리보기 PDF 의 저장·인쇄 막기.
pub fn harden_main(app: &tauri::App, state: AutofillState) {
    let Some(window) = app.get_webview_window("main") else {
        put(&state, false);
        return;
    };
    #[cfg(windows)]
    {
        let s = state.clone();
        if window
            .with_webview(move |wv| {
                let c = wv.controller();
                let ok = unsafe { autofill_off(&c) && lock_pdf_viewer(&c) };
                put(&s, ok);
            })
            .is_err()
        {
            put(&state, false);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        put(&state, false);
    }
}

/// 출력 창: 자동완성 저장 끄기.
pub fn disable_autofill_on(window: &WebviewWindow) {
    #[cfg(windows)]
    {
        let _ = window.with_webview(|wv| unsafe {
            autofill_off(&wv.controller());
        });
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
unsafe fn autofill_off(controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller) -> bool {
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

#[cfg(windows)]
unsafe fn lock_pdf_viewer(controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller) -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows::core::Interface;

    let Ok(core) = controller.CoreWebView2() else { return false };
    let Ok(settings) = core.Settings() else { return false };
    let hide = COREWEBVIEW2_PDF_TOOLBAR_ITEMS(
        COREWEBVIEW2_PDF_TOOLBAR_ITEMS_SAVE.0 | COREWEBVIEW2_PDF_TOOLBAR_ITEMS_SAVE_AS.0 | COREWEBVIEW2_PDF_TOOLBAR_ITEMS_PRINT.0,
    );
    let pdf_ok = settings
        .cast::<ICoreWebView2Settings7>()
        .and_then(|s7| s7.SetHiddenPdfToolbarItems(hide))
        .is_ok();
    let keys_ok = settings
        .cast::<ICoreWebView2Settings3>()
        .and_then(|s3| s3.SetAreBrowserAcceleratorKeysEnabled(false))
        .is_ok();
    pdf_ok && keys_ok
}
