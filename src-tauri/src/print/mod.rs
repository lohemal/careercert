//! 출력 엔진 — 하나의 HTML 을 하나의 엔진(WebView2)으로 PDF·인쇄한다.
//!
//! ```text
//!   html ─▶ Engine::pdf / Engine::print
//!             │ ① 숨은 출력 창("printer", print.html)이 떠 있는지 확인
//!             │ ② 작업 번호로 html 을 맡겨 두고 창에 "이 번호를 불러와" (eval)
//!             │ ③ 창이 print_take 로 html 을 **가져가고(맡긴 곳에서 지움)** 글꼴까지 다 읽으면 print_loaded
//!             │ ④ WebView2 PrintToPdfStream(메모리) / Print(프린터)
//!             └ ⑤ 창의 문서를 비운다 (성공·실패 모두)
//! ```
//!
//! 개인정보
//!   * PDF 는 **메모리 스트림**으로 받는다(`PrintToPdfStream`). 임시 파일을 만들지 않는다.
//!   * html 은 작업이 끝나면 맡긴 곳·출력 창 어디에도 남지 않는다. 로그에 쓰지 않는다.
//!   * 한 번에 한 작업만 한다(출력 창이 하나뿐이다).

pub mod inspect;
pub mod output;
#[cfg(windows)]
mod wv2;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};

pub const WINDOW_LABEL: &str = "printer";
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// 출력 창이 문서를 다 불러왔다는 신호.
#[derive(Debug, Clone, Copy)]
pub struct Loaded {
    /// 내장 글꼴이 실제로 쓰였는가 — 아니면 출력하지 않는다(PC 마다 글꼴이 달라진다)
    pub font_ok: bool,
}

/// 인쇄 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrintStatus {
    /// 프린터(스풀러)에 넘겼다
    Succeeded,
    /// 프린터를 쓸 수 없다
    PrinterUnavailable,
    /// 그 밖의 실패
    OtherError,
}

#[derive(Default)]
pub struct Engine {
    next: AtomicU64,
    /// 맡겨 둔 html (작업 번호 → html). 출력 창이 가져가면 지운다.
    jobs: Mutex<HashMap<u64, String>>,
    /// 불러오기를 기다리는 작업
    waiters: Mutex<HashMap<u64, oneshot::Sender<Result<Loaded, String>>>>,
    /// 출력 창의 첫 준비 신호
    host_ready: Mutex<Option<oneshot::Sender<()>>>,
    /// 한 번에 한 작업
    serial: tokio::sync::Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn fail(msg: impl Into<String>) -> AppError {
    AppError::new("PRINT_FAILED", "출력 중 문제가 생겼습니다. 다시 시도해 주세요.").detail(msg)
}

impl Engine {
    // ------------------------------------------------------------
    // 출력 창이 부르는 것 (commands::print)
    // ------------------------------------------------------------

    /// 맡겨 둔 html 을 **가져간다.** 두 번째 부르면 없다.
    pub fn take(&self, id: u64) -> Option<String> {
        lock(&self.jobs).remove(&id)
    }

    pub fn loaded(&self, id: u64, result: Result<Loaded, String>) {
        if let Some(tx) = lock(&self.waiters).remove(&id) {
            let _ = tx.send(result);
        }
    }

    pub fn host_ready(&self) {
        if let Some(tx) = lock(&self.host_ready).take() {
            let _ = tx.send(());
        }
    }

    // ------------------------------------------------------------
    // 출력
    // ------------------------------------------------------------

    /// html 을 A4 PDF 로. PDF 바이트를 메모리로 돌려준다.
    pub async fn pdf(&self, app: &AppHandle, html: String) -> AppResult<Vec<u8>> {
        let _one = self.serial.lock().await;
        let window = self.window(app).await?;
        let result = async {
            self.load(&window, html).await?;
            self.run_pdf(&window).await
        }
        .await;
        clear(&window);
        result
    }

    /// html 을 프린터로. `printer` 가 없으면 기본 프린터.
    pub async fn print(&self, app: &AppHandle, html: String, printer: Option<String>, copies: u32) -> AppResult<PrintStatus> {
        let _one = self.serial.lock().await;
        let window = self.window(app).await?;
        let result = async {
            self.load(&window, html).await?;
            self.run_print(&window, printer, copies).await
        }
        .await;
        clear(&window);
        result
    }

    /// Windows 인쇄 대화상자를 띄운다(개발 검증용 — 끝났는지 알 수 없어 정식 출력에는 쓰지 않는다).
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub async fn show_dialog(&self, app: &AppHandle, html: String) -> AppResult<()> {
        let _one = self.serial.lock().await;
        let window = self.window(app).await?;
        self.load(&window, html).await?;
        #[cfg(windows)]
        {
            let (tx, rx) = oneshot::channel();
            window
                .with_webview(move |wv| {
                    let _ = tx.send(unsafe { wv2::show_print_ui(&wv.controller()) });
                })
                .map_err(|e| fail(e.to_string()))?;
            rx.await.map_err(|e| fail(e.to_string()))?.map_err(fail)?;
        }
        // 대화상자가 문서를 쓰는 동안은 비우지 않는다 — 다음 작업이 비운다
        Ok(())
    }

    /// 숨은 출력 창. 없으면 만들고 첫 준비 신호를 기다린다.
    async fn window(&self, app: &AppHandle) -> AppResult<WebviewWindow> {
        if let Some(w) = app.get_webview_window(WINDOW_LABEL) {
            return Ok(w);
        }
        let (tx, rx) = oneshot::channel();
        *lock(&self.host_ready) = Some(tx);
        let w = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("print.html".into()))
            .title("출력")
            .visible(false)
            .skip_taskbar(true)
            .inner_size(800.0, 1100.0)
            .build()
            .map_err(|e| fail(format!("출력 창을 만들지 못함: {e}")))?;
        tokio::time::timeout(LOAD_TIMEOUT, rx)
            .await
            .map_err(|_| fail("출력 창이 준비되지 않음(시간 초과)"))?
            .map_err(|e| fail(e.to_string()))?;
        crate::webview::disable_autofill_on(&w);
        Ok(w)
    }

    /// html 을 맡기고 출력 창이 다 불러올 때까지 기다린다.
    async fn load(&self, window: &WebviewWindow, html: String) -> AppResult<()> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        lock(&self.jobs).insert(id, html);
        lock(&self.waiters).insert(id, tx);
        let cleanup = |e: AppError| {
            lock(&self.jobs).remove(&id);
            lock(&self.waiters).remove(&id);
            e
        };
        window
            .eval(format!("window.__certLoad({id})"))
            .map_err(|e| cleanup(fail(e.to_string())))?;
        let loaded = tokio::time::timeout(LOAD_TIMEOUT, rx)
            .await
            .map_err(|_| cleanup(fail("문서를 불러오지 못함(시간 초과)")))?
            .map_err(|e| cleanup(fail(e.to_string())))?
            .map_err(|e| cleanup(fail(e)))?;
        if !loaded.font_ok {
            return Err(AppError::new(
                "PRINT_FONT",
                "증명서 글꼴을 불러오지 못해 출력하지 않았습니다. 프로그램을 다시 설치해 주세요.",
            ));
        }
        Ok(())
    }

    async fn run_pdf(&self, window: &WebviewWindow) -> AppResult<Vec<u8>> {
        #[cfg(windows)]
        {
            let (tx, rx) = oneshot::channel();
            window
                .with_webview(move |wv| unsafe { wv2::print_to_pdf_stream(&wv.controller(), tx) })
                .map_err(|e| fail(e.to_string()))?;
            tokio::time::timeout(RUN_TIMEOUT, rx)
                .await
                .map_err(|_| fail("PDF 만들기 시간 초과"))?
                .map_err(|e| fail(e.to_string()))?
                .map_err(fail)
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            Err(fail("Windows 에서만 출력할 수 있습니다"))
        }
    }

    async fn run_print(&self, window: &WebviewWindow, printer: Option<String>, copies: u32) -> AppResult<PrintStatus> {
        #[cfg(windows)]
        {
            let (tx, rx) = oneshot::channel();
            window
                .with_webview(move |wv| unsafe { wv2::print(&wv.controller(), printer, copies, tx) })
                .map_err(|e| fail(e.to_string()))?;
            tokio::time::timeout(RUN_TIMEOUT, rx)
                .await
                .map_err(|_| fail("인쇄 시간 초과"))?
                .map_err(|e| fail(e.to_string()))?
                .map_err(fail)
        }
        #[cfg(not(windows))]
        {
            let _ = (window, printer, copies);
            Err(fail("Windows 에서만 출력할 수 있습니다"))
        }
    }
}

/// 출력 창의 문서를 비운다 — 주민번호·주소가 담긴 DOM 을 남기지 않는다.
fn clear(window: &WebviewWindow) {
    let _ = window.eval("window.__certClear()");
}

/// 설치된 프린터 이름과 기본 프린터.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Printers {
    pub names: Vec<String>,
    pub default: Option<String>,
}

pub fn printers() -> Printers {
    #[cfg(windows)]
    {
        wv2::printers()
    }
    #[cfg(not(windows))]
    {
        Printers { names: vec![], default: None }
    }
}
