//! 발급 확정 · 발급 기록 · 정식 출력(PDF 저장·인쇄) · 발급 취소.
//!
//! 정식 출력은 **언제나 발급 번호(certificate_id)로** 부른다. 명령은 DB 의 발급 기록에서 문서를 다시 만들고
//! 지문을 검증한 뒤(`service::issuance::for_output`)에만 출력한다 — 화면이 보낸 내용을 출력하는 길은 없다.
//!
//! 출력 이력: 정말로 성공했을 때만 SUCCESS. 실패는 FAILED 로 따로 남긴다. 저장 위치(경로)는 남기지 않는다.

use serde::Serialize;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use super::now;
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::print::{output, PrintStatus, Printers};
use crate::service::issuance::{self as service, IssueRequest, OutputKind, Summary};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputView {
    /// PRINT · PDF
    pub output_type: String,
    /// SUCCESS · FAILED
    pub result: String,
    pub printer_name: Option<String>,
    pub copies: Option<i64>,
    pub at: String,
}

/// 발급 기록 요약 — 주민번호는 가린 값만.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedView {
    pub id: i64,
    pub issue_no: String,
    pub issued_on_label: String,
    pub holder_name: String,
    pub masked_rrn: String,
    /// FULL · MASK_BACK
    pub rrn_display: String,
    pub purpose: String,
    pub item_count: i64,
    pub issuer_title: String,
    /// ISSUED · VOIDED
    pub status: String,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    /// 정식 출력할 수 있는가 (취소된 건은 아니다)
    pub can_output: bool,
    /// 정식 출력할 수 없을 때 보일 말
    pub message: Option<String>,
    pub created_at: String,
    pub outputs: Vec<OutputView>,
}

fn view(s: Summary) -> IssuedView {
    let r = s.row;
    let issued = r.status == "ISSUED";
    IssuedView {
        issued_on_label: date::parse_iso(&r.issued_on).map(date::display).unwrap_or_else(|_| r.issued_on.clone()),
        can_output: issued,
        message: (!issued).then(|| "취소된 발급 건입니다. 정식 출력할 수 없습니다.".to_string()),
        id: r.id,
        issue_no: r.issue_no,
        holder_name: r.holder_name,
        masked_rrn: r.masked_rrn,
        rrn_display: r.rrn_display,
        purpose: r.purpose,
        item_count: r.item_count,
        issuer_title: r.issuer_title,
        status: r.status,
        voided_at: r.voided_at,
        void_reason: r.void_reason,
        created_at: r.created_at,
        outputs: s
            .outputs
            .into_iter()
            .map(|o| OutputView {
                output_type: o.output_type,
                result: o.result,
                printer_name: o.printer_name,
                copies: o.copies,
                at: o.at,
            })
            .collect(),
    }
}

/// 발급 확정 — 내용 확인과 같은 요청을 다시 받아 서버에서 문서를 다시 만들고 검증한 뒤 스냅샷으로 남긴다.
#[tauri::command]
pub fn certificate_issue(state: State<'_, AppState>, request: IssueRequest) -> AppResult<IssuedView> {
    let now = now();
    let id = state.db.write(|c| service::issue(c, request, &now, service::LOCAL_ACTOR))?;
    Ok(view(state.db.read(|c| service::summary(c, id))?))
}

#[tauri::command]
pub fn certificate_issued(state: State<'_, AppState>, id: i64) -> AppResult<IssuedView> {
    Ok(view(state.db.read(|c| service::summary(c, id))?))
}

/// 발급 취소 — 사유 필수. 내용은 그대로, 정식 출력은 막힌다.
#[tauri::command]
pub fn certificate_void(state: State<'_, AppState>, id: i64, reason: String) -> AppResult<IssuedView> {
    let now = now();
    state.db.write(|c| service::void(c, id, &reason, &now))?;
    Ok(view(state.db.read(|c| service::summary(c, id))?))
}

/// 설치된 프린터와 기본 프린터·상태.
#[tauri::command]
pub fn printers_list() -> Printers {
    crate::print::printers()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    /// false = 저장 창에서 취소함(아무것도 기록하지 않는다)
    pub saved: bool,
    /// 저장한 파일 이름(폴더는 돌려주지도 기록하지도 않는다)
    pub file_name: Option<String>,
    pub issued: IssuedView,
}

/// 정식 PDF 저장 — 저장 위치는 사용자가 고른다. 기본 이름은 `경력증명서_성명_발급번호.pdf`.
#[tauri::command]
pub async fn certificate_save_pdf(app: tauri::AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<SaveResult> {
    let (doc, proof) = state.db.read(|c| service::for_output(c, id))?;
    let engine = state.print.clone();
    let pdf = output::issued_pdf(&engine, &app, &doc, proof).await?;
    let default_name = crate::domain::certificate::pdf_file_name(&doc);
    drop(doc);

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("증명서 PDF 저장")
        .set_file_name(&default_name)
        .add_filter("PDF 문서", &["pdf"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let picked = rx.await.map_err(|e| AppError::internal(e.to_string()))?;
    let Some(picked) = picked else {
        return Ok(SaveResult { saved: false, file_name: None, issued: view(state.db.read(|c| service::summary(c, id))?) });
    };
    let mut path = picked.into_path().map_err(|e| AppError::internal(e.to_string()))?;
    if path.extension().is_none() {
        path.set_extension("pdf");
    }
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string());
    let result = output::save_pdf(&path, &pdf);
    let now = now();
    let (ok, detail) = match &result {
        Ok(()) => (true, String::new()),
        Err(e) => (false, e.code.clone()),
    };
    state.db.write(|c| service::record_output(c, id, OutputKind::Pdf, None, ok, &detail, &now))?;
    result?;
    Ok(SaveResult { saved: true, file_name, issued: view(state.db.read(|c| service::summary(c, id))?) })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintResult {
    /// SUCCEEDED · PRINTER_UNAVAILABLE · OTHER_ERROR
    pub status: PrintStatus,
    pub issued: IssuedView,
}

/// 정식 인쇄 — 화면에서 고른 프린터·매수로. 결과(스풀러에 넘김/실패)를 돌려주고 이력에 남긴다.
#[tauri::command]
pub async fn certificate_print(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    printer: String,
    copies: u32,
) -> AppResult<PrintResult> {
    if !(1..=20).contains(&copies) {
        return Err(AppError::invalid("매수는 1~20부 사이로 골라 주세요."));
    }
    let printer = printer.trim().to_string();
    if printer.is_empty() {
        return Err(AppError::invalid("프린터를 골라 주세요."));
    }
    let (doc, proof) = state.db.read(|c| service::for_output(c, id))?;
    let engine = state.print.clone();
    let outcome = output::issued_print(&engine, &app, &doc, proof, Some(printer.clone()), copies).await;
    drop(doc);
    let now = now();
    let (status, detail) = match &outcome {
        Ok(s) => (Some(*s), format!("{s:?}")),
        Err(e) => (None, e.code.clone()),
    };
    let ok = status == Some(PrintStatus::Succeeded);
    state
        .db
        .write(|c| service::record_output(c, id, OutputKind::Print { copies }, Some(&printer), ok, &detail, &now))?;
    let status = outcome?;
    Ok(PrintResult { status, issued: view(state.db.read(|c| service::summary(c, id))?) })
}
