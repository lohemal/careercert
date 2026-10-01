//! 설정 → 데이터 관리: 복구 비밀번호 · 이동용 백업 내보내기 · 백업 복원 · 엑셀 가져오기.
//!
//! * 비밀번호는 받아서 쓰고 버린다(`Password` — 메모리에서 지움, Debug 가림). 로그·오류·변경 기록에 넣지 않는다.
//! * 파일 위치는 Rust 의 파일 대화상자로 고르고, **경로는 화면에 돌려주지 않는다**(파일 이름만).
//! * 무거운 일(Argon2id · 암호화 · 발급본 전체 검증 · 엑셀 읽기)은 화면을 멈추지 않게 따로 돌린다.
//! * 복원·가져오기는 두 단계 — 준비(미리보기) 결과를 Rust 가 들고 있다가 확인을 받으면 그것으로만 진행한다.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use super::now;
use crate::crypto::recovery::Password;
use crate::db::{backup, Db};
use crate::domain::audit::Action;
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::service::import::{self, Decisions, Extract};
use crate::service::portable::{self, Counts, Header, Prepared};
use crate::service::{recovery, restore};
use crate::AppState;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::internal(e.to_string()))?
}

fn label(iso: &str) -> String {
    let d = iso.get(..10).and_then(|s| date::parse_iso(s).ok()).map(date::display).unwrap_or_default();
    format!("{d} {}", iso.get(11..16).unwrap_or(""))
}

// ---------------------------------------------------------------
// 복구 비밀번호
// ---------------------------------------------------------------

#[tauri::command]
pub async fn recovery_set(state: State<'_, AppState>, password: Password, confirm: Password) -> AppResult<()> {
    let db = state.db.clone();
    blocking(move || db.write(|c| recovery::set(c, &password, &confirm, &now()))).await
}

#[tauri::command]
pub async fn recovery_change(state: State<'_, AppState>, old: Password, password: Password, confirm: Password) -> AppResult<()> {
    let db = state.db.clone();
    blocking(move || db.write(|c| recovery::change(c, &old, &password, &confirm, &now()))).await
}

// ---------------------------------------------------------------
// 이동용 백업 내보내기
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// false = 저장 창에서 취소
    pub saved: bool,
    pub file_name: Option<String>,
}

/// ① 복구 설정·비밀번호 확인(틀리면 아무것도 만들지 않는다) → ② 저장 위치 → ③ 메모리로 뜨고 무결성 →
/// ④ 전체 암호화 → ⑤ `.part` 로 쓰고 이름 바꾸기 → ⑥ 만든 파일을 다시 열어 인증·무결성 확인
#[tauri::command]
pub async fn backup_export(app: tauri::AppHandle, state: State<'_, AppState>, password: Password) -> AppResult<ExportResult> {
    let db = state.db.clone();
    {
        let (db, pw) = (db.clone(), password.clone());
        blocking(move || db.read(|c| recovery::open_all(c, &pw).map(|_| ()))).await?;
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("이동용 백업 저장")
        .set_file_name(portable::default_file_name(&now()))
        .add_filter("이동용 백업", &[portable::EXTENSION])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(picked) = rx.await.map_err(|e| AppError::internal(e.to_string()))? else {
        return Ok(ExportResult { saved: false, file_name: None });
    };
    let mut path: PathBuf = picked.into_path().map_err(|e| AppError::internal(e.to_string()))?;
    if path.extension().and_then(|e| e.to_str()) != Some(portable::EXTENSION) {
        let mut s = path.into_os_string();
        s.push(format!(".{}", portable::EXTENSION));
        path = PathBuf::from(s);
    }
    let version = app.package_info().version.to_string();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string());
    blocking(move || {
        let at = now();
        let bytes = db.read(|c| portable::build(c, &password, &at, &version))?;
        portable::write_atomic(&path, &bytes)?;
        drop(bytes);
        // 다시 열어 본다 — 실패하면 만든 파일을 지운다(불완전한 백업을 남기지 않는다)
        let check = std::fs::read(&path).map_err(AppError::from).and_then(|b| portable::open(&b, &password).map(|_| ()));
        if let Err(e) = check {
            let _ = std::fs::remove_file(&path);
            return Err(AppError::new("BACKUP_VERIFY_FAILED", "만든 백업을 다시 열어 확인하지 못해 지웠습니다.").detail(e.code));
        }
        db.write(|c| {
            let k = portable::counts(c)?;
            audit::add(
                c,
                &at,
                Action::BackupExport,
                None,
                &format!("이동용 백업 만듦 · 강사 {} · 경력 {} · 발급 {}", k.instructors, k.careers, k.certificates_issued + k.certificates_voided),
            )
        })
    })
    .await?;
    Ok(ExportResult { saved: true, file_name })
}

// ---------------------------------------------------------------
// 복원
// ---------------------------------------------------------------

/// 복원 진행 상태 (Rust 가 들고 있다)
pub struct RestoreSession {
    file_name: String,
    bytes: Arc<Vec<u8>>,
    prepared: Option<Prepared>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderView {
    pub file_name: String,
    pub created_at_label: String,
    pub schema_version: i32,
    pub app_version: String,
    pub format_version: u32,
}

fn header_view(file_name: &str, h: &Header) -> HeaderView {
    HeaderView {
        file_name: file_name.into(),
        created_at_label: label(&h.created_at),
        schema_version: h.schema_version,
        app_version: h.app_version.clone(),
        format_version: h.format_version,
    }
}

/// ① 파일 고르기 → ② 머리·형식 검사 (비밀번호 없이)
#[tauri::command]
pub async fn restore_pick(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<Option<HeaderView>> {
    *state.restore.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("복원할 이동용 백업 고르기")
        .add_filter("이동용 백업", &[portable::EXTENSION])
        .pick_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(picked) = rx.await.map_err(|e| AppError::internal(e.to_string()))? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| AppError::internal(e.to_string()))?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let bytes = blocking(move || {
        if std::fs::metadata(&path)?.len() > 2 * 1024 * 1024 * 1024 {
            return Err(AppError::new("BACKUP_INVALID", "파일이 너무 큽니다."));
        }
        Ok(std::fs::read(&path)?)
    })
    .await?;
    let header = portable::read_header(&bytes)?;
    let view = header_view(&file_name, &header);
    *state.restore.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(RestoreSession { file_name, bytes: Arc::new(bytes), prepared: None });
    Ok(Some(view))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub header: HeaderView,
    /// 백업에 든 자료
    pub backup: Counts,
    /// 지금 자료 (전부 바뀐다)
    pub current: Counts,
    pub migrated_from: i32,
    pub latest_schema: i32,
    pub verified_certificates: usize,
    pub same_windows_user: bool,
}

/// ③ 비밀번호 → ④ 인증·복호화 → ⑤ 메모리 DB → ⑥⑦⑧⑨ app_id·무결성·구조 버전·마이그레이션
/// → 데이터 키 복구·이 PC DPAPI 재포장 → 모든 발급본 검증 → ⑩ 미리보기. 지금 자료는 건드리지 않는다.
#[tauri::command]
pub async fn restore_unlock(state: State<'_, AppState>, password: Password) -> AppResult<RestorePreview> {
    let (bytes, file_name) = {
        let mut g = state.restore.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.as_mut().ok_or_else(|| AppError::invalid("먼저 백업 파일을 골라 주세요."))?;
        s.prepared = None;
        (s.bytes.clone(), s.file_name.clone())
    };
    let prepared = blocking(move || portable::prepare(&bytes, &password)).await?;
    let current = state.db.read(portable::counts)?;
    let view = RestorePreview {
        header: header_view(&file_name, &prepared.header),
        backup: prepared.counts,
        current,
        migrated_from: prepared.migrated_from,
        latest_schema: crate::db::migrate::latest_version(),
        verified_certificates: prepared.verified_certificates,
        same_windows_user: prepared.same_windows_user,
    };
    let mut g = state.restore.lock().unwrap_or_else(|e| e.into_inner());
    match g.as_mut() {
        Some(s) if s.file_name == file_name => s.prepared = Some(prepared),
        _ => return Err(AppError::invalid("복원 준비가 취소되었습니다. 다시 시작해 주세요.")),
    }
    Ok(view)
}

/// ⑪ 최종 확인 뒤: ⑫ 지금 자료 before_restore_ 백업 → 대기 파일 → 연결 놓기 → ⑱ 다시 시작
/// (⑰ 교체와 ⑲ 열림 검증은 다음 시작에서 연결을 열기 전에 — `service::restore::apply_pending`)
#[tauri::command]
pub async fn restore_confirm(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let session = state
        .restore
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or_else(|| AppError::invalid("복원 준비가 없습니다. 처음부터 다시 해 주세요."))?;
    let prepared = session.prepared.ok_or_else(|| AppError::invalid("먼저 복구 비밀번호로 백업을 확인해 주세요."))?;
    let db: Arc<Db> = state.db.clone();
    blocking(move || {
        backup::create(&db, backup::Kind::BeforeRestore, chrono::Local::now())?;
        restore::stage(&prepared, db.path(), &now())?;
        drop(prepared);
        db.release()
    })
    .await?;
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(700));
        handle.restart();
    });
    Ok(())
}

/// 복원을 그만둔다 — 준비한 메모리 자료를 버린다
#[tauri::command]
pub fn restore_cancel(state: State<'_, AppState>) {
    *state.restore.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// ---------------------------------------------------------------
// 엑셀 가져오기
// ---------------------------------------------------------------

pub struct ImportSession {
    token: String,
    path: PathBuf,
    file_name: String,
    extract: Arc<Extract>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub field: import::Field,
    pub label: &'static str,
    pub header: String,
    pub column: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    pub row_no: u32,
    pub name: String,
    pub program: String,
    pub position: String,
    pub duty: String,
    pub start_text: String,
    pub end_text: String,
    /// 읽은 결과 `2022.03.04` · `현재` · ''
    pub start_label: String,
    pub end_label: String,
    pub status: import::RowStatus,
    pub messages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    pub id: i64,
    pub distinguisher: String,
    pub phone: String,
    pub careers: i64,
    pub programs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupView {
    pub name: String,
    pub row_nos: Vec<u32>,
    pub phones: Vec<String>,
    pub candidates: Vec<CandidateView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisView {
    pub token: String,
    pub file_name: String,
    pub kind: import::SourceKind,
    pub sheet: String,
    pub sheets: Vec<String>,
    pub header_row: u32,
    pub columns: Vec<ColumnView>,
    pub rows: Vec<RowView>,
    pub groups: Vec<GroupView>,
    pub formats: Vec<(String, usize)>,
}

fn analysis_view(s: &ImportSession, a: import::Analysis) -> AnalysisView {
    let ex = &s.extract;
    AnalysisView {
        token: s.token.clone(),
        file_name: s.file_name.clone(),
        kind: ex.kind,
        sheet: ex.sheet.clone(),
        sheets: ex.sheets.clone(),
        header_row: ex.header_row,
        columns: ex
            .columns
            .iter()
            .map(|(f, h, c)| ColumnView { field: *f, label: f.label(), header: h.clone(), column: c.clone() })
            .collect(),
        rows: a
            .rows
            .into_iter()
            .map(|p| RowView {
                start_label: p.start.map(date::display).unwrap_or_default(),
                end_label: match &p.end {
                    import::End::Date(d) => date::display(*d),
                    import::End::Current => date::CURRENT_LABEL.into(),
                    _ => String::new(),
                },
                row_no: p.row_no,
                name: p.name,
                program: p.program,
                position: p.position,
                duty: p.duty,
                start_text: p.start_text,
                end_text: p.end_text,
                status: p.status,
                messages: p.messages,
            })
            .collect(),
        groups: a
            .groups
            .into_iter()
            .map(|g| GroupView {
                name: g.name,
                row_nos: g.row_nos,
                phones: g.phones,
                candidates: g
                    .candidates
                    .into_iter()
                    .map(|c| CandidateView { id: c.id, distinguisher: c.distinguisher, phone: c.phone, careers: c.careers, programs: c.programs })
                    .collect(),
            })
            .collect(),
        formats: a.formats.into_iter().collect(),
    }
}

fn today() -> AppResult<chrono::NaiveDate> {
    crate::service::career::today(&now())
}

/// 파일 고르기(sheet 없음) 또는 같은 파일의 다른 시트로 다시 분석
#[tauri::command]
pub async fn import_open(app: tauri::AppHandle, state: State<'_, AppState>, sheet: Option<String>) -> AppResult<Option<AnalysisView>> {
    let (path, file_name) = match &sheet {
        Some(_) => {
            let g = state.import.lock().unwrap_or_else(|e| e.into_inner());
            let s = g.as_ref().ok_or_else(|| AppError::invalid("먼저 엑셀 파일을 골라 주세요."))?;
            (s.path.clone(), s.file_name.clone())
        }
        None => {
            *state.import.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let (tx, rx) = tokio::sync::oneshot::channel();
            app.dialog()
                .file()
                .set_title("가져올 엑셀 파일 고르기")
                .add_filter("엑셀 통합 문서", &["xlsx", "xlsm"])
                .pick_file(move |p| {
                    let _ = tx.send(p);
                });
            let Some(picked) = rx.await.map_err(|e| AppError::internal(e.to_string()))? else {
                return Ok(None);
            };
            let path = picked.into_path().map_err(|e| AppError::internal(e.to_string()))?;
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            (path, name)
        }
    };
    let p2 = path.clone();
    let extract = blocking(move || import::read(&p2, sheet.as_deref())).await?;
    let today = today()?;
    let analysis = state.db.read(|c| import::analyze(c, &extract, today))?;
    let session = ImportSession { token: uuid::Uuid::new_v4().to_string(), path, file_name, extract: Arc::new(extract) };
    let view = analysis_view(&session, analysis);
    *state.import.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);
    Ok(Some(view))
}

fn session_extract(state: &AppState, token: &str) -> AppResult<Arc<Extract>> {
    let g = state.import.lock().unwrap_or_else(|e| e.into_inner());
    match g.as_ref() {
        Some(s) if s.token == token => Ok(s.extract.clone()),
        _ => Err(AppError::new("IMPORT_CHANGED", "가져오기 분석이 바뀌었거나 끝났습니다. 파일을 다시 골라 주세요.")),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub summary: import::Summary,
    pub problems: Vec<String>,
    pub outcomes: Vec<(u32, import::Outcome)>,
    pub fingerprint: String,
}

/// 반영 미리보기 — 결정대로 실제로 넣을 것
#[tauri::command]
pub fn import_plan(state: State<'_, AppState>, token: String, decisions: Decisions) -> AppResult<PlanView> {
    let ex = session_extract(&state, &token)?;
    let today = today()?;
    let p = state.db.read(|c| {
        let a = import::analyze(c, &ex, today)?;
        import::plan(c, &a, &decisions)
    })?;
    Ok(PlanView { summary: p.summary, problems: p.problems, outcomes: p.outcomes.into_iter().collect(), fingerprint: p.fingerprint })
}

/// 반영 — 미리보기 지문이 같을 때만. before_import_ 백업 → 한 트랜잭션
#[tauri::command]
pub async fn import_apply(state: State<'_, AppState>, token: String, decisions: Decisions, fingerprint: String) -> AppResult<import::Applied> {
    let ex = session_extract(&state, &token)?;
    let db = state.db.clone();
    let r = blocking(move || import::run(&db, &ex, &decisions, &fingerprint, &now())).await?;
    *state.import.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(r)
}

#[tauri::command]
pub fn import_cancel(state: State<'_, AppState>) {
    *state.import.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
