//! 증명서 출력의 경계 — **작성 중 문서는 미리보기만, 정식 출력은 발급 기록에서만.**
//!
//! ```text
//!   작성 중 CertificateDoc ──▶ draft_preview_pdf ──▶ Mode::PreviewDraft (워터마크) ──▶ 화면 미리보기
//!   발급 기록 + IssuedProof ──▶ issued_pdf / issued_print ──▶ Mode::Issued ──▶ PDF 저장 · 인쇄 (Phase 6)
//! ```
//!
//! 정식 출력 함수는 `IssuedProof` 를 요구한다. 증표는 발급 기록을 읽은 쪽(Phase 6)만 만들 수 있으므로
//! 작성 중 내용을 정식 출력에 넘기는 길이 없다. 정식 출력 명령은 Phase 6 전까지 화면에 연결하지 않는다.

use std::path::Path;

use tauri::AppHandle;

use super::{Engine, PrintStatus};
use crate::domain::certificate::CertificateDoc;
use crate::error::{AppError, AppResult};
use crate::render::{self, IssuedProof, Mode};

/// 작성 중 미리보기 PDF — 언제나 '발급 전 미리보기' 워터마크가 들어간다.
pub async fn draft_preview_pdf(engine: &Engine, app: &AppHandle, doc: &CertificateDoc) -> AppResult<Vec<u8>> {
    let html = render::render(doc, Mode::PreviewDraft)?;
    engine.pdf(app, html).await
}

/// 발급된 증명서 PDF (워터마크 없음).
#[allow(dead_code)] // Phase 6 발급이력·발급 확정이 부른다
pub async fn issued_pdf(engine: &Engine, app: &AppHandle, doc: &CertificateDoc, proof: IssuedProof) -> AppResult<Vec<u8>> {
    let html = render::render(doc, Mode::Issued(proof))?;
    engine.pdf(app, html).await
}

/// 발급된 증명서를 프린터로.
#[allow(dead_code)] // Phase 6
pub async fn issued_print(
    engine: &Engine,
    app: &AppHandle,
    doc: &CertificateDoc,
    proof: IssuedProof,
    printer: Option<String>,
    copies: u32,
) -> AppResult<PrintStatus> {
    let html = render::render(doc, Mode::Issued(proof))?;
    engine.print(app, html, printer, copies).await
}

/// 사용자가 고른 경로에 PDF 를 쓴다. 다 쓰기 전에는 그 이름으로 보이지 않게 옆에 쓰고 바꿔 단다.
#[allow(dead_code)] // Phase 6 (정식 PDF 저장)
pub fn save_pdf(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if !bytes.starts_with(b"%PDF") {
        return Err(AppError::internal("PDF 가 아닌 내용"));
    }
    let tmp = path.with_extension("pdf.part");
    std::fs::write(&tmp, bytes).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        AppError::new("PDF_SAVE_FAILED", "PDF 를 저장하지 못했습니다. 다른 위치를 골라 주세요.").detail(e.to_string())
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        AppError::new("PDF_SAVE_FAILED", "PDF 를 저장하지 못했습니다. 같은 이름의 파일이 열려 있지 않은지 확인해 주세요.")
            .detail(e.to_string())
    })
}
