//! 시험 도우미.

use std::path::PathBuf;

/// 시험마다 따로 쓰는 빈 폴더 (`%TEMP%/careercert-test-<tag>-<nanos>`).
pub fn tmp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("careercert-test-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
