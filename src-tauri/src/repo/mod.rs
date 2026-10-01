//! SQL. 표마다 한 파일. 함수는 `&Connection` 을 받는다 — `Db::read/write` 안에서 부른다.

#[cfg_attr(not(test), allow(dead_code))]
pub mod career;
#[cfg_attr(not(test), allow(dead_code))]
pub mod instructor;
pub mod settings;
