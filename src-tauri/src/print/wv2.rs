//! WebView2 COM 호출 (Windows 전용). `with_webview` 콜백 안 — 메인(UI) 스레드에서 부른다.
//!
//! 인쇄 설정은 여기서 **고정**한다: A4 세로, 배율 100%, 브라우저 머리글·바닥글 없음, 배경 인쇄.
//! 여백은 0 으로 두고 양식 CSS 의 `@page` 여백을 쓴다(쪽 번호 칸이 그 여백에 들어간다).

use tokio::sync::oneshot;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{PrintCompletedHandler, PrintToPdfStreamCompletedHandler};
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG};

use super::{PrintStatus, Printers};

/// A4 (인치)
const A4_W: f64 = 8.27;
const A4_H: f64 = 11.69;

unsafe fn core16(c: &ICoreWebView2Controller) -> Result<ICoreWebView2_16, String> {
    c.CoreWebView2()
        .and_then(|w| w.cast::<ICoreWebView2_16>())
        .map_err(|e| format!("WebView2 가 출력 기능을 지원하지 않음(런타임 업데이트 필요): {e}"))
}

/// 고정 인쇄 설정.
unsafe fn settings(c: &ICoreWebView2Controller) -> Result<ICoreWebView2PrintSettings, String> {
    let core = c.CoreWebView2().map_err(|e| e.to_string())?;
    let env = core
        .cast::<ICoreWebView2_2>()
        .and_then(|w| w.Environment())
        .and_then(|e| e.cast::<ICoreWebView2Environment6>())
        .map_err(|e| format!("인쇄 설정을 만들 수 없음: {e}"))?;
    let s = env.CreatePrintSettings().map_err(|e| e.to_string())?;
    (|| -> windows::core::Result<()> {
        s.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)?;
        s.SetScaleFactor(1.0)?;
        s.SetPageWidth(A4_W)?;
        s.SetPageHeight(A4_H)?;
        s.SetMarginTop(0.0)?;
        s.SetMarginBottom(0.0)?;
        s.SetMarginLeft(0.0)?;
        s.SetMarginRight(0.0)?;
        s.SetShouldPrintBackgrounds(true)?;
        s.SetShouldPrintSelectionOnly(false)?;
        s.SetShouldPrintHeaderAndFooter(false)?;
        Ok(())
    })()
    .map_err(|e| format!("인쇄 설정 실패: {e}"))?;
    Ok(s)
}

/// PDF 를 메모리 스트림으로 받는다 — 임시 파일 없음.
pub unsafe fn print_to_pdf_stream(c: &ICoreWebView2Controller, tx: oneshot::Sender<Result<Vec<u8>, String>>) {
    let start = || -> Result<(ICoreWebView2_16, ICoreWebView2PrintSettings), String> { Ok((core16(c)?, settings(c)?)) };
    let (core, s) = match start() {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(Err(e));
            return;
        }
    };
    let tx = std::sync::Mutex::new(Some(tx));
    let send = move |r: Result<Vec<u8>, String>| {
        if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(r);
        }
    };
    let send = std::sync::Arc::new(send);
    let send2 = send.clone();
    let handler = PrintToPdfStreamCompletedHandler::create(Box::new(move |hr, stream| {
        let r = hr
            .map_err(|e| format!("PDF 만들기 실패: {e}"))
            .and_then(|_| stream.ok_or_else(|| "PDF 스트림이 비어 있음".to_string()))
            .and_then(|s| read_all(&s));
        send2(r);
        Ok(())
    }));
    if let Err(e) = core.PrintToPdfStream(&s, &handler) {
        send(Err(format!("PDF 만들기를 시작하지 못함: {e}")));
    }
}

unsafe fn read_all(stream: &IStream) -> Result<Vec<u8>, String> {
    let mut stat = STATSTG::default();
    let _ = stream.Stat(&mut stat, STATFLAG_NONAME);
    let mut out = Vec::with_capacity(stat.cbSize as usize);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        let hr = stream.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read));
        if hr.is_err() {
            return Err(format!("PDF 스트림 읽기 실패: {hr:?}"));
        }
        if read == 0 {
            break;
        }
        out.extend_from_slice(&buf[..read as usize]);
    }
    if !out.starts_with(b"%PDF") {
        return Err("PDF 가 아닌 결과".into());
    }
    Ok(out)
}

/// 프린터로 보낸다. 결과(스풀러에 넘김/프린터 없음/실패)를 알 수 있다.
pub unsafe fn print(
    c: &ICoreWebView2Controller,
    printer: Option<String>,
    copies: u32,
    tx: oneshot::Sender<Result<PrintStatus, String>>,
) {
    let start = || -> Result<(ICoreWebView2_16, ICoreWebView2PrintSettings), String> {
        let core = core16(c)?;
        let s = settings(c)?;
        let s2 = s.cast::<ICoreWebView2PrintSettings2>().map_err(|e| e.to_string())?;
        s2.SetCopies(copies.clamp(1, 20) as i32).map_err(|e| e.to_string())?;
        if let Some(p) = &printer {
            s2.SetPrinterName(&HSTRING::from(p.as_str())).map_err(|e| e.to_string())?;
        }
        Ok((core, s))
    };
    let (core, s) = match start() {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(Err(e));
            return;
        }
    };
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx2 = tx.clone();
    let handler = PrintCompletedHandler::create(Box::new(move |hr, status| {
        let r = hr.map_err(|e| format!("인쇄 실패: {e}")).map(|_| match status {
            COREWEBVIEW2_PRINT_STATUS_SUCCEEDED => PrintStatus::Succeeded,
            COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE => PrintStatus::PrinterUnavailable,
            _ => PrintStatus::OtherError,
        });
        if let Some(tx) = tx2.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(r);
        }
        Ok(())
    }));
    if let Err(e) = core.Print(&s, &handler) {
        if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(Err(format!("인쇄를 시작하지 못함: {e}")));
        }
    }
}

/// Windows 인쇄 대화상자 (개발 검증용).
pub unsafe fn show_print_ui(c: &ICoreWebView2Controller) -> Result<(), String> {
    core16(c)?
        .ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_SYSTEM)
        .map_err(|e| e.to_string())
}

/// 설치된 프린터 목록 (EnumPrinters) 과 기본 프린터.
pub fn printers() -> Printers {
    use windows::core::PWSTR;
    use windows::Win32::Graphics::Printing::{
        EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
    };
    let mut names = Vec::new();
    unsafe {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut need, mut count) = (0u32, 0u32);
        let _ = EnumPrintersW(flags, None, 4, None, &mut need, &mut count);
        if need > 0 {
            let mut buf = vec![0u8; need as usize];
            if EnumPrintersW(flags, None, 4, Some(&mut buf), &mut need, &mut count).is_ok() {
                let items = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
                for it in items {
                    if let Ok(s) = it.pPrinterName.to_string() {
                        names.push(s);
                    }
                }
            }
        }
        let mut len = 0u32;
        let _ = GetDefaultPrinterW(None, &mut len);
        let mut default = None;
        if len > 0 {
            let mut w = vec![0u16; len as usize];
            if GetDefaultPrinterW(Some(PWSTR(w.as_mut_ptr())), &mut len).as_bool() {
                default = PWSTR(w.as_mut_ptr()).to_string().ok();
            }
        }
        Printers { names, default }
    }
}
