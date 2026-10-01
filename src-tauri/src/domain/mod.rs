//! 업무 규칙. DB·Tauri 를 모른다 — 서버로 옮겨도 그대로 쓰는 층.

// 강사·경력 규칙은 Phase 3 화면이 쓰기 시작한다 — 그전까지 시험만 쓴다.
#[cfg_attr(not(test), allow(dead_code))]
pub mod career;
#[cfg_attr(not(test), allow(dead_code))]
pub mod date;
#[cfg_attr(not(test), allow(dead_code))]
pub mod instructor;
pub mod settings;
