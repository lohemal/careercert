//! 앱 전역 오류 타입.
//!
//! 화면에는 `user_message`(사용자가 해결할 수 있는 한국어 설명)만 보여 주고,
//! `detail`(개발자용 원문)은 [자세히] 접기 안에 둔다.
//! **성명·주민번호·주소 같은 개인정보는 `detail` 에도 넣지 않는다.**

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

pub type AppResult<T> = std::result::Result<T, AppError>;

/// 화면으로 보낼 때(직렬화)만 쓰는 정리 규칙 — 안의 `detail` 은 시험·개발용으로 그대로 둔다.
///
/// * 설치본(release)에서는 [자세히]에 **오류 코드만** 보낸다. Rust·SQLite·WebView2 원문을 보이지 않는다.
/// * 개발 빌드에서는 원문을 보내되 `sanitize` 를 지난다(전체 경로·주민번호 모양 숫자·긴 16진수 지움).
/// * 사용자 문장도 `sanitize` 를 지난다 — 실수로 경로나 값이 섞여도 화면에는 가려진다.
#[derive(Debug, Clone)]
pub struct AppError {
    /// 프로그램이 분기 처리할 때 쓰는 코드. 화면에 그대로 노출하지 않는다.
    pub code: String,
    /// 사용자에게 보여 줄 한국어 문장.
    pub user_message: String,
    /// 개발자용 원문. 접기 안에 표시.
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, user_message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            user_message: user_message.into(),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// 입력값이 잘못된 경우
    pub fn invalid(user_message: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", user_message)
    }

    /// 찾는 대상이 없는 경우
    pub fn not_found(user_message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", user_message)
    }

    /// 예상하지 못한 내부 오류
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            "INTERNAL",
            "작업을 완료하지 못했습니다. 같은 문제가 계속되면 프로그램을 다시 시작해 주세요.",
        )
        .detail(detail)
    }

    /// 화면의 [자세히]에 보일 글 — 설치본은 오류 코드만
    pub fn detail_for_ui(&self) -> String {
        let code = format!("오류 코드: {}", self.code);
        match (&self.detail, cfg!(debug_assertions)) {
            (Some(d), true) => format!("{code}\n{}", sanitize(d)),
            _ => code,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("code", &self.code)?;
        st.serialize_field("userMessage", &sanitize(&self.user_message))?;
        st.serialize_field("detail", &self.detail_for_ui())?;
        st.end()
    }
}

/// 화면에 보일 글에서 지운다: Windows 전체 경로(`C:\…`, `\\?\…`), 주민번호 모양 숫자(6자리-7자리·13자리),
/// 32자 이상 16진수(키·암호문·지문).
pub fn sanitize(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let stop = |c: char| matches!(c, '"' | '\'' | ')' | '(' | '\n' | '|' | '<' | '>' | ',');
    while i < chars.len() {
        let c = chars[i];
        // 경로: 드라이브 문자 + ":\" 또는 "\\"
        let drive = c.is_ascii_alphabetic() && chars.get(i + 1) == Some(&':') && matches!(chars.get(i + 2), Some('\\') | Some('/'));
        let unc = c == '\\' && chars.get(i + 1) == Some(&'\\');
        if drive || unc {
            let mut j = i;
            while j < chars.len() && !stop(chars[j]) && !(chars[j] == ' ' && chars.get(j + 1) == Some(&':')) {
                j += 1;
            }
            out.push_str("<경로>");
            i = j;
            continue;
        }
        // 숫자 덩어리
        if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '-') {
                j += 1;
            }
            let run: String = chars[i..j].iter().collect();
            let digits = run.chars().filter(char::is_ascii_digit).count();
            let rrn_like = digits >= 13 || run.split('-').any(|p| p.len() == 6) && run.split('-').any(|p| p.len() == 7);
            out.push_str(if rrn_like { "******" } else { &run });
            i = j;
            continue;
        }
        // 긴 16진수
        if c.is_ascii_hexdigit() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j - i >= 32 {
                out.push_str("<…>");
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod sanitize_tests {
    use super::*;

    #[test]
    fn 경로와_주민번호_모양과_긴_16진수를_지운다() {
        let s = sanitize(r"unable to open C:\Users\누구\AppData\Roaming\kr.school.careercert\careercert.db :: x");
        assert!(!s.contains("Users") && s.contains("<경로>"), "{s}");
        assert_eq!(sanitize("주민번호 880808-2000002 끝"), "주민번호 ****** 끝"); // privacy:fake
        assert_eq!(sanitize("8808082000002"), "******"); // privacy:fake
        assert_eq!(sanitize(&"ab".repeat(32)), "<…>");
        assert_eq!(sanitize("2026-10-01 발급 3건"), "2026-10-01 발급 3건", "날짜·건수는 그대로");
        assert_eq!(sanitize("제2026-152호"), "제2026-152호");
        assert_eq!(sanitize(r"\\?\C:\x\y.db"), "<경로>");
    }

    #[test]
    fn 화면으로_보낼_때_설치본은_오류_코드만() {
        let e = AppError::new("DB_ERROR", "자료를 저장하거나 불러오지 못했습니다.").detail(r"C:\Users\x\a.db 880808-2000002"); // privacy:fake
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("오류 코드: DB_ERROR"));
        assert!(!json.contains(r"Users") && !json.contains("2000002"), "{json}");
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.user_message)?;
        if let Some(d) = &self.detail {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::Error as E;
        match &e {
            E::QueryReturnedNoRows => AppError::not_found("요청하신 자료를 찾을 수 없습니다."),
            E::SqliteFailure(err, msg) => {
                let text = msg.clone().unwrap_or_default();
                if text.contains("UNIQUE constraint failed") {
                    AppError::new("DUPLICATE", "이미 같은 내용이 등록되어 있습니다.")
                        .detail(format!("{err:?} {text}"))
                } else if text.contains("FOREIGN KEY constraint failed") {
                    AppError::new(
                        "IN_USE",
                        "다른 자료에서 사용 중이라 처리할 수 없습니다.",
                    )
                    .detail(format!("{err:?} {text}"))
                } else if text.contains("CHECK constraint failed") {
                    AppError::invalid("입력한 값이 올바르지 않습니다. 다시 확인해 주세요.")
                        .detail(format!("{err:?} {text}"))
                } else {
                    AppError::new("DB_ERROR", "자료를 저장하거나 불러오지 못했습니다.")
                        .detail(format!("{err:?} {text}"))
                }
            }
            other => AppError::new("DB_ERROR", "자료를 저장하거나 불러오지 못했습니다.")
                .detail(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(format!("JSON 처리 실패: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("IO_ERROR", "파일을 읽거나 쓰지 못했습니다.").detail(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::internal(format!("앱 내부 오류: {e}"))
    }
}
