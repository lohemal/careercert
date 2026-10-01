//! 출력 창(print.html)이 부르는 명령과 개발 검증용 명령.

use tauri::State;

use crate::print::Loaded;
use crate::AppState;

/// 출력 창이 처음 떴다.
#[tauri::command]
pub fn print_host_ready(state: State<'_, AppState>) {
    state.print.host_ready();
}

/// 맡겨 둔 html 을 **가져간다**(두 번째는 None). 출력 창만 부른다.
#[tauri::command]
pub fn print_take(state: State<'_, AppState>, id: u64) -> Option<String> {
    state.print.take(id)
}

/// 출력 창이 문서를 다 불러왔다(글꼴까지).
#[tauri::command]
pub fn print_loaded(state: State<'_, AppState>, id: u64, ok: bool, font_ok: bool, error: Option<String>) {
    let r = if ok {
        Ok(Loaded { font_ok })
    } else {
        Err(error.unwrap_or_else(|| "출력 창이 문서를 불러오지 못함".into()))
    };
    state.print.loaded(id, r);
}

/// 개발 검증용 — **배포 빌드에서는 거절한다.** 가상 자료로만 그린다(실제 강사 자료를 읽지 않는다).
///
/// * `spike` · `dialog` · `print` · `printers` — 작은 시험 문서로 WebView2 출력 기능 확인
/// * `render` — 가상 증명서(경력 `rows` 건, `mask`)의 **발급 전 미리보기** PDF 바이트
/// * `render-facts` — 위 PDF 의 쪽 수·크기·글꼴과 기본 파일 이름(JSON)
///
/// 발급본(워터마크 없음) 출력은 여기서 만들 수 없다 — 발급 증표(`IssuedProof`)는 발급 기록을 읽는
/// `service::issuance` 만 만든다. 정식 출력 확인은 연습용 자료로 실제로 발급한 뒤 정식 명령으로 한다.
#[tauri::command]
pub async fn dev_print_spike(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    kind: String,
    printer: Option<String>,
    rows: Option<u32>,
    mask: Option<bool>,
) -> crate::error::AppResult<tauri::ipc::Response> {
    use crate::error::AppError;
    use crate::print::inspect;
    use crate::render::{self, Mode};
    use tauri::ipc::Response;

    if !cfg!(debug_assertions) {
        return Err(AppError::new("NOT_AVAILABLE", "개발용 기능입니다."));
    }
    let engine = state.print.clone();
    let rows = rows.unwrap_or(5) as usize;
    match kind.as_str() {
        "spike" => Ok(Response::new(engine.pdf(&app, spike_html(rows as u32)).await?)),
        "dialog" => {
            engine.show_dialog(&app, spike_html(3)).await?;
            Ok(Response::new(b"dialog".to_vec()))
        }
        "print" => {
            let status = engine.print(&app, spike_html(rows as u32), printer, 1).await?;
            Ok(Response::new(format!("{status:?}").into_bytes()))
        }
        "printers" => Ok(Response::new(serde_json::to_vec(&crate::print::printers())?)),
        "render" | "render-facts" => {
            let doc = dev_doc(&state, rows, mask.unwrap_or(false))?;
            let pdf = engine.pdf(&app, render::render(&doc, Mode::PreviewDraft)?).await?;
            if kind == "render" {
                Ok(Response::new(pdf))
            } else {
                let f = inspect::facts(&pdf);
                let file = crate::domain::certificate::pdf_file_name(&doc);
                let display = doc.rrn_display.code();
                Ok(Response::new(serde_json::to_vec(
                    &serde_json::json!({ "facts": f, "fileName": file, "rrnDisplay": display }),
                )?))
            }
        }
        _ => Err(AppError::invalid("kind")),
    }
}

/// 가상 증명서 — 학교 설정은 지금 값, 강사·경력·주민번호·주소는 지어낸 값.
fn dev_doc(
    state: &State<'_, AppState>,
    rows: usize,
    mask: bool,
) -> crate::error::AppResult<crate::domain::certificate::CertificateDoc> {
    use chrono::{Duration, NaiveDate};

    use crate::domain::career::{Career, CareerFields, EndReason, Term};
    use crate::domain::certificate::{build, IssueInput};
    use crate::domain::instructor::Instructor;

    let school = state.db.read(crate::repo::settings::get)?;
    let who = Instructor {
        id: 1,
        uuid: "0".repeat(36),
        name: "김가람".into(),
        distinguisher: "".into(),
        phone: "".into(),
        memo: "".into(),
        archived_at: None,
        created_at: "".into(),
        updated_at: "".into(),
    };
    let today = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap_or_default();
    let programs = ["마술", "생명과학", "배드민턴", "로봇과학", "바둑", "창의미술", "코딩"];
    let careers: Vec<Career> = (0..rows)
        .map(|i| {
            let start = NaiveDate::from_ymd_opt(1990 + (i as i32 % 36), 3, 4).unwrap_or(today);
            let p = programs[i % programs.len()];
            let duty = if i % 4 == 3 {
                format!("방과후학교 {p} 지도 및 학생 활동 기록 관리, 학기말 발표회 운영 지원 (긴 지도사항 줄바꿈 확인용)")
            } else {
                format!("방과후학교 {p}")
            };
            let last = i + 1 == rows;
            Career {
                id: i as i64 + 1,
                uuid: "0".repeat(36),
                instructor_id: 1,
                fields: CareerFields {
                    program_name: p.into(),
                    position: "강사".into(),
                    duty,
                    start_date: if last { NaiveDate::from_ymd_opt(2026, 3, 4).unwrap_or(today) } else { start },
                    term: if last {
                        Term::Active
                    } else {
                        Term::Ended { end_date: start + Duration::days(340), reason: EndReason::ContractEnd }
                    },
                    planned_end_date: None,
                    memo: "".into(),
                },
                import_id: None,
                archived_at: None,
                created_at: "".into(),
                updated_at: "".into(),
            }
        })
        .collect();
    let built = build(
        &who,
        &careers,
        IssueInput {
            rrn: "880808-2000002".into(), // privacy:fake (개발 검증용 가상값)
            address: "가상시 연습구 시험로 123-45".into(),
            issue_no: "제2026-152호".into(),
            purpose: school.default_purpose.clone(),
            issued_on: "2026-10-01".into(),
            mask_rrn: mask,
        },
        &school,
        today,
    )?;
    Ok(built.doc)
}

fn spike_html(rows: u32) -> String {
    let body: String = (1..=rows)
        .map(|i| format!("<tr><td>{i}</td><td>2026.03.04 ~ 현재</td><td>방과후학교 마술 지도사항 {i}</td></tr>"))
        .collect();
    let style = r#"
@font-face { font-family: CertSans; src: url('/fonts/NotoSansKR-Regular.ttf') format('truetype'); font-weight: 400; }
@font-face { font-family: CertSans; src: url('/fonts/NotoSansKR-Bold.ttf') format('truetype'); font-weight: 700; }
@page { size: A4; margin: 20mm 15mm 18mm; @bottom-right { content: "제2026-152호 · 시험 · " counter(page) " / " counter(pages); font: 8pt CertSans; color: #666; } }
html, body { margin: 0; font-family: CertSans; }
h1 { font-weight: 700; font-size: 20pt; text-align: center; }
table { width: 100%; border-collapse: collapse; }
thead { display: table-header-group; }
td, th { border: 1px solid #000; padding: 4px; font-size: 11pt; }
.wm { position: fixed; top: 40%; left: 0; right: 0; text-align: center; font-size: 60pt; color: rgba(200,0,0,.15); transform: rotate(-30deg); }
.end { break-inside: avoid; margin-top: 20px; border: 2px solid #000; padding: 10px; }
"#;
    format!(
        r#"<head><meta charset="utf-8"><style>{style}</style></head><body>
<div class="wm">시험 문서</div>
<h1>출력 시험 문서</h1>
<p>한글 굵기 시험: <b>굵게 700</b> · 보통 400 · 한자 金 · 기호 ○ △ ~ · 1234567890</p>
<table><thead><tr><th>번호</th><th>기간</th><th>지도사항</th></tr></thead><tbody>{body}</tbody></table>
<div class="end">용도 · 위와 같이 증명합니다 · 2026.10.01 · ○○초등학교장 (직인)</div>
</body>"#
    )
}
