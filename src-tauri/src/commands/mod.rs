pub mod app;
pub mod bulk_end;
pub mod career;
pub mod certificate;
pub mod dashboard;
pub mod instructor;
pub mod print;
pub mod settings;

/// 지금 시각 `YYYY-MM-DDTHH:MM:SS` (현지). service 에 넘기는 시각은 언제나 이것이다.
pub(crate) fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}
