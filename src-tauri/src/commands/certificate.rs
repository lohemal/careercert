//! 증명서 작성 — 화면에 보내는 모양과 명령. **읽기만 한다**(Phase 4).
//!
//! 주민번호·주소는 `certificate_prepare` 의 입력으로 들어와 결과로 돌아갈 뿐이다.
//! 이 파일은 그 값을 로그·기록·오류에 넣지 않는다.

use serde::Serialize;
use tauri::State;

use super::career::{view, CareerView};
use super::now;
use crate::domain::certificate::{Built, CertificateDoc};
use crate::domain::date;
use crate::error::AppResult;
use crate::service::career::today;
use crate::service::certificate::{self as service, PrepareRequest};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceView {
    #[serde(flatten)]
    pub career: CareerView,
    /// 이 발급일의 증명서에 넣을 수 있는가
    pub eligible: bool,
    /// 넣을 수 없는 까닭
    pub reason: Option<String>,
    /// 발급일 기준 '부터' · '까지' (넣을 수 있을 때만) — domain 이 만든 글자
    pub on_issue_from: Option<String>,
    pub on_issue_to: Option<String>,
    /// 예정 종료일 표시 `2027.02.05` — 관리 참고용
    pub planned_end_label: Option<String>,
}

#[tauri::command]
pub fn certificate_choices(state: State<'_, AppState>, instructor_id: i64, issued_on: String) -> AppResult<Vec<ChoiceView>> {
    let now = now();
    let t = today(&now)?;
    let issued = date::parse_iso(&issued_on).ok();
    let list = state.db.read(|c| service::choices(c, instructor_id, &issued_on))?;
    Ok(list
        .into_iter()
        .map(|ch| {
            let start = ch.career.fields.start_date;
            let planned = ch.career.fields.planned_end_date;
            let (eligible, reason, parts) = match (ch.on_issue, issued) {
                (Ok(end), _) => (true, None, Some(date::display_period_parts(start, end))),
                (Err(why), Some(on)) => (false, Some(why.message(on)), None),
                (Err(_), None) => (false, None, None),
            };
            ChoiceView {
                career: view(ch.career, t),
                eligible,
                reason,
                on_issue_from: parts.as_ref().map(|p| p.0.clone()),
                on_issue_to: parts.map(|p| p.1),
                planned_end_label: planned.map(date::display),
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub career_id: i64,
    pub from: String,
    pub to: String,
    pub program_name: String,
    pub position: String,
    pub duty: String,
}

/// 화면이 '증명서 내용 확인' 에 보여 줄 모양. 담당자가 입력한 주민번호·주소를 그대로 돌려준다
/// (작성 화면 안에서만 쓰고, 다른 화면·저장소로는 가지 않는다).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocView {
    pub template_version: u32,
    pub title: String,
    pub issue_no: String,
    /// `2026.10.01`
    pub issued_on_label: String,
    pub purpose: String,
    pub holder_name: String,
    pub holder_rrn: String,
    /// 증명서에 찍힐 모양 (전체 / 뒷자리 가림) — render 가 만든다
    pub holder_rrn_shown: String,
    pub rrn_masked: bool,
    pub holder_address: String,
    pub items: Vec<ItemView>,
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
}

impl From<CertificateDoc> for DocView {
    fn from(d: CertificateDoc) -> Self {
        let shown = crate::render::rrn_text(&d);
        DocView {
            holder_rrn_shown: shown,
            rrn_masked: d.rrn_display == crate::domain::certificate::RrnDisplay::MaskBack,
            template_version: d.template_version,
            title: d.title,
            issue_no: d.issue_no,
            issued_on_label: date::display(d.issued_on),
            purpose: d.purpose,
            holder_name: d.holder.name,
            holder_rrn: d.holder.rrn.as_str().to_string(),
            holder_address: d.holder.address,
            items: d
                .items
                .into_iter()
                .map(|i| ItemView {
                    career_id: i.source_career_id,
                    from: i.from_text,
                    to: i.to_text,
                    program_name: i.program_name,
                    position: i.position,
                    duty: i.duty,
                })
                .collect(),
            issuer_title: d.school.issuer_title,
            department: d.school.department,
            manager_name: d.school.manager_name,
            phone: d.school.phone,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningView {
    pub code: &'static str,
    pub message: String,
    pub career_id: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedView {
    pub doc: DocView,
    pub warnings: Vec<WarningView>,
}

/// 실제 출력 미리보기 — 작성 중 내용으로 **'발급 전 미리보기' PDF** 를 만든다(정식 출력 아님).
///
/// `certificate_prepare` 와 같은 검사를 지난 내용만 그린다. PDF 는 메모리로만 오가고 파일로 남지 않는다.
#[tauri::command]
pub async fn certificate_preview(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: PrepareRequest,
) -> AppResult<tauri::ipc::Response> {
    let now = now();
    let Built { doc, .. } = state.db.read(|c| service::prepare(c, request, &now))?;
    let engine = state.print.clone();
    let pdf = crate::print::output::draft_preview_pdf(&engine, &app, &doc).await?;
    Ok(tauri::ipc::Response::new(pdf))
}

#[tauri::command]
pub fn certificate_prepare(state: State<'_, AppState>, request: PrepareRequest) -> AppResult<PreparedView> {
    let now = now();
    let Built { doc, warnings } = state.db.read(|c| service::prepare(c, request, &now))?;
    Ok(PreparedView {
        doc: doc.into(),
        warnings: warnings
            .into_iter()
            .map(|w| WarningView {
                code: w.code,
                message: w.message,
                career_id: w.career_id,
            })
            .collect(),
    })
}
