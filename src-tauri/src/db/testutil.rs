//! 시험 도우미.

use std::ops::Deref;
use std::path::{Path, PathBuf};

/// 시험마다 따로 쓰는 빈 폴더 (`%TEMP%/careercert-test-<tag>-<nanos>`).
///
/// **시험이 끝나면(실패해서 풀려 나가도) 지운다** — 가상 시험 자료가 TEMP 에 쌓이지 않게.
/// 디버깅하려고 남기고 싶을 때만 환경변수 `CAREERCERT_KEEP_TEST_DIRS=1` 을 켠다.
/// 이 값을 쥔 변수가 DB 보다 **먼저** 선언되어야 DB 가 닫힌 뒤에 지워진다(변수는 거꾸로 버려진다).
pub struct TempDir {
    path: PathBuf,
}

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if std::env::var_os("CAREERCERT_KEEP_TEST_DIRS").is_some_and(|v| v == "1") {
            return;
        }
        // 막 닫힌 파일이 아직 잡혀 있을 수 있어 몇 번 다시 해 본다
        for _ in 0..10 {
            if std::fs::remove_dir_all(&self.path).is_ok() || !self.path.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

pub fn tmp_dir(tag: &str) -> TempDir {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("careercert-test-{tag}-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    TempDir { path }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 시험_폴더는_끝나면_지워진다() {
        let kept = {
            let d = tmp_dir("selfcheck");
            std::fs::write(d.join("x.db"), b"x").unwrap();
            d.to_path_buf()
        };
        if std::env::var_os("CAREERCERT_KEEP_TEST_DIRS").is_none() {
            assert!(!kept.exists());
        }
    }
}
