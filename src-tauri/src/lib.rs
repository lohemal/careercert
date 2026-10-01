//! 방과후 강사 경력증명서 발급 시스템.
//!
//! 층 나눔 (설계안 §3)
//!   commands/  Tauri 에만 묶인 얇은 층 — 입력 받기 → 아래 층 호출 → 결과
//!   db/        연결 · 마이그레이션 · 백업
//!   domain/    업무 규칙 (DB·Tauri 를 모름)
//!   repo/      SQL
//!   service/   업무 절차 (읽고 → domain 으로 판단 → repo 로 쓰기)
//!   (이후 Phase) render/ · print/ · crypto/

mod commands;
mod db;
mod domain;
pub mod error;
mod repo;
mod print;
mod render;
mod service;
mod webview;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::commands::app::StartupNote;
use crate::db::{backup, Db, DB_FILE};

pub struct AppState {
    pub db: Arc<Db>,
    /// 연습용(`.sandbox`) 실행인가 — 화면이 눈에 띄게 알린다.
    pub sandbox: bool,
    /// 시작할 때 있었던 일 (무결성·자동 백업). 화면이 한 번 보여 준다.
    pub startup: Mutex<Vec<StartupNote>>,
    /// WebView2 입력 자동완성 저장을 껐는가 (설정 → 데이터 관리가 보여 준다)
    pub autofill_off: webview::AutofillState,
    /// 출력 엔진 (숨은 출력 창 하나)
    pub print: Arc<print::Engine>,
}

impl AppState {
    pub fn notes(&self) -> Vec<StartupNote> {
        self.startup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // %APPDATA%\kr.school.careercert\careercert.db
            // 연습용은 %APPDATA%\kr.school.careercert.sandbox\ — 실제 자료와 섞이지 않는다.
            // 설치 폴더와 따로다 — 프로그램을 지우거나 새로 깔아도 자료는 남는다.
            let sandbox = app.config().identifier.ends_with(".sandbox");
            let db_path = app.path().app_data_dir()?.join(DB_FILE);
            let now = chrono::Local::now();
            let mut notes: Vec<StartupNote> = Vec::new();

            let db = Db::open(&db_path)?;

            // 자료가 성한가
            let sound = match db.integrity() {
                Ok(v) if v == "ok" => true,
                Ok(v) => {
                    notes.push(StartupNote::new(
                        "INTEGRITY",
                        "error",
                        format!(
                            "자료 파일에 문제가 있습니다({v}). 새로 저장하기 전에 \
                             백업 폴더의 최근 백업을 확인해 주세요."
                        ),
                    ));
                    false
                }
                Err(e) => {
                    notes.push(StartupNote::new("INTEGRITY", "error", e.user_message.clone()));
                    false
                }
            };

            // 하루 한 번 자동 백업 — **성할 때만** 만든다.
            if sound {
                if let Err(e) = backup::auto_if_needed(&db, now) {
                    notes.push(StartupNote::new(
                        "BACKUP_FAILED",
                        "warn",
                        format!("자동 백업을 만들지 못했습니다. {}", e.user_message),
                    ));
                }
            }

            let autofill_off = webview::AutofillState::default();
            webview::harden_main(app, autofill_off.clone());

            app.manage(AppState {
                db: Arc::new(db),
                sandbox,
                startup: Mutex::new(notes),
                autofill_off,
                print: Arc::new(print::Engine::default()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::settings::settings_get,
            commands::settings::settings_save,
            commands::dashboard::dashboard_overview,
            // 강사
            commands::instructor::instructor_search,
            commands::instructor::instructor_get,
            commands::instructor::instructor_create,
            commands::instructor::instructor_update,
            commands::instructor::instructor_archive,
            commands::instructor::instructor_unarchive,
            // 경력
            commands::career::career_list,
            commands::career::career_create,
            commands::career::career_update,
            commands::career::career_end,
            commands::career::career_archive,
            commands::career::career_unarchive,
            commands::career::career_hints,
            // 증명서 작성 (읽기만 — 발급 기록 저장은 Phase 6)
            commands::certificate::certificate_choices,
            commands::certificate::certificate_prepare,
            commands::certificate::certificate_preview,
            // 출력 창이 부르는 것
            commands::print::print_host_ready,
            commands::print::print_take,
            commands::print::print_loaded,
            commands::print::dev_print_spike,
            // 일괄 종료
            commands::bulk_end::bulk_end_candidates,
            commands::bulk_end::bulk_end_preview,
            commands::bulk_end::bulk_end_apply,
        ])
        .run(tauri::generate_context!())
        .expect("경력증명서 발급 시스템을 시작하지 못했습니다.");
}
