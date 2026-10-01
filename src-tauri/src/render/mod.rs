//! 증명서 HTML 만들기 — `CertificateDoc` → HTML 의 **유일한 길**.
//!
//! * 양식 판(`template_version`)마다 함수가 따로 있고, **옛 판을 지우지 않는다.** 양식을 고치면 새 판을
//!   더한다. Phase 6 발급 기록은 판 번호를 함께 저장하므로 지난 증명서는 그때의 판으로 다시 그린다.
//! * 출력 모드는 문자열을 나중에 고치는 것이 아니라 **render 의 인자**다.
//!     - `PreviewDraft` — 작성 중 미리보기. '발급 전 미리보기' 워터마크가 언제나 들어간다.
//!     - `Issued`       — 발급 확정된 증명서. 워터마크 없음. **발급 기록이 있어야만**(`IssuedProof`) 고를 수 있다.
//! * 사용자 값은 모두 이스케이프한다(HTML 본문·속성, CSS 문자열).
//! * 주민번호는 `rrn_text` 가 표시 방식(`RrnDisplay`)에 따라 만든다 — 원본은 바꾸지 않는다.

mod v1;
mod v2;

use crate::domain::certificate::{CertificateDoc, RrnDisplay};
use crate::error::{AppError, AppResult};

/// 발급 기록에서 왔다는 증표 — 정의는 `service::issuance` 에 있고 **그 모듈만 만들 수 있다**
/// (만드는 함수가 그 모듈 밖에 보이지 않는다). 정식 인쇄·PDF 저장은 이것을 요구하므로, 작성 중
/// `CertificateDoc` 을 정식 출력에 넘기는 길이 없다.
pub use crate::service::issuance::IssuedProof;

/// 출력 모드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 작성 중 미리보기 — 워터마크
    PreviewDraft,
    /// 발급된 증명서 — 워터마크 없음
    Issued(IssuedProof),
}

impl Mode {
    pub fn is_draft(self) -> bool {
        matches!(self, Mode::PreviewDraft)
    }
}

/// 미리보기 워터마크 글자
pub const DRAFT_MARK: &str = "발급 전 미리보기";

/// 증명서 HTML (`<head>…</head><body>…</body>` — 출력 창이 `<html>` 안에 넣는다).
pub fn render(doc: &CertificateDoc, mode: Mode) -> AppResult<String> {
    match doc.template_version {
        1 => Ok(v1::render(doc, mode)),
        2 => v2::render(doc, mode),
        v => Err(AppError::new("TEMPLATE_UNKNOWN", "이 프로그램이 모르는 증명서 양식입니다. 프로그램을 업데이트해 주세요.")
            .detail(format!("template_version={v}"))),
    }
}

/// 증명서에 찍을 주민번호 — 전체 `YYMMDD-NNNNNNN` / 뒷자리 가림 `YYMMDD-N******`.
pub fn rrn_text(doc: &CertificateDoc) -> String {
    match doc.rrn_display {
        RrnDisplay::Full => doc.holder.rrn.as_str().to_string(),
        RrnDisplay::MaskBack => doc.holder.rrn.masked(),
    }
}

/// HTML 본문·속성 이스케이프.
pub(crate) fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// CSS 문자열(`content: "…"`) 이스케이프. `<style>` 밖으로 빠져나가지 못하게 `<`·`>` 도 바꾼다.
pub(crate) fn css_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\22 "),
            '\\' => out.push_str("\\5C "),
            '<' => out.push_str("\\3C "),
            '>' => out.push_str("\\3E "),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
