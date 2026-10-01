//! 이동용 백업 패키지 시험. 가상값만 쓴다.

use super::*;
use crate::crypto::recovery::Password;
use crate::db::Db;
use crate::service::issuance::issuance_tests::{issue_now, world, World, FAKE_ADDRESS, FAKE_RRN};
use crate::service::recovery;

const NOW: &str = "2026-10-01T09:00:00";
pub(crate) const PW: &str = "가상 복구 비밀번호 문장";

pub(crate) fn pw(s: &str) -> Password {
    Password::new(s)
}

/// 발급 1건 + 복구 비밀번호 설정까지 된 자료
pub(crate) fn ready(w: &World) -> i64 {
    let id = issue_now(w, "제2026-152호").unwrap();
    w.db.write(|c| recovery::set(c, &pw(PW), &pw(PW), NOW)).unwrap();
    id
}

pub(crate) fn package(db: &Db) -> Vec<u8> {
    db.read(|c| build(c, &pw(PW), NOW, "0.0.1")).unwrap()
}

fn contains(hay: &[u8], needle: &str) -> bool {
    let n = needle.as_bytes();
    let u16: Vec<u8> = needle.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    hay.windows(n.len()).any(|w| w == n) || hay.windows(u16.len()).any(|w| w == u16.as_slice())
}

#[test]
fn 복구_비밀번호가_없으면_만들지_않는다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    assert_eq!(w.db.read(|c| build(c, &pw(PW), NOW, "0.0.1")).unwrap_err().code, "RECOVERY_NOT_SET");
}

#[test]
fn 비밀번호가_틀리면_만들지_않는다() {
    let w = world();
    ready(&w);
    assert_eq!(w.db.read(|c| build(c, &pw("틀린 복구 비밀번호"), NOW, "0.0.1")).unwrap_err().code, "RECOVERY_PASSWORD_WRONG");
}

#[test]
fn 패키지에는_머리_정보가_있고_비밀번호로_다시_열린다() {
    let w = world();
    ready(&w);
    let bytes = package(&w.db);
    let h = read_header(&bytes).unwrap();
    assert_eq!(h.format, "careercert-backup");
    assert_eq!(h.format_version, 1);
    assert_eq!(h.app_id, APP_ID);
    assert_eq!(h.schema_version, migrate::latest_version());
    assert_eq!(h.created_at, NOW);
    assert_eq!(h.kdf.alg, "argon2id");
    assert_eq!(h.cipher, "AES-256-GCM");
    let (_, mem) = open(&bytes, &pw(PW)).unwrap();
    assert_eq!(counts(&mem).unwrap(), w.db.read(counts).unwrap());
}

#[test]
fn 패키지_파일에는_업무_자료와_개인정보가_보이지_않는다() {
    let w = world();
    ready(&w);
    let bytes = package(&w.db);
    assert!(!contains(&bytes, "SQLite format 3"), "SQLite 파일로 보이지 않는다");
    for needle in ["김가람", "마술", "방과후학교 마술", "○○초등학교", "가상담당", "000-0000-0000", "제2026-152호", FAKE_ADDRESS, FAKE_RRN, "8808082000002", "880808-2******", PW] { // privacy:fake
        assert!(!contains(&bytes, needle), "'{needle}' 이 보인다");
    }
}

#[test]
fn 머리나_본문을_바꾸면_열리지_않는다() {
    let w = world();
    ready(&w);
    let bytes = package(&w.db);
    // 본문 끝 한 바이트
    let mut a = bytes.clone();
    let last = a.len() - 1;
    a[last] ^= 1;
    assert_eq!(open(&a, &pw(PW)).unwrap_err().code, "BACKUP_UNLOCK_FAILED");
    // 머리의 만든 시각 한 글자 (형식은 그대로 — 인증이 막는다)
    let mut b = bytes.clone();
    let at = b.windows(4).position(|x| x == b"2026").unwrap();
    b[at + 3] = b'7';
    assert_eq!(open(&b, &pw(PW)).unwrap_err().code, "BACKUP_UNLOCK_FAILED");
    // 잘림
    assert_eq!(open(&bytes[..bytes.len() / 2], &pw(PW)).unwrap_err().code, "BACKUP_UNLOCK_FAILED");
    assert_eq!(open(&bytes[..20], &pw(PW)).unwrap_err().code, "BACKUP_INVALID");
    // 다른 파일
    assert_eq!(read_header(b"SQLite format 3\0........").unwrap_err().code, "BACKUP_INVALID");
}

#[test]
fn 틀린_비밀번호로는_열리지_않는다() {
    let w = world();
    ready(&w);
    let bytes = package(&w.db);
    assert_eq!(open(&bytes, &pw("틀린 복구 비밀번호")).unwrap_err().code, "BACKUP_UNLOCK_FAILED");
}

#[test]
fn 다른_앱의_자료는_거절한다() {
    let w = world();
    ready(&w);
    let mem = w.db.read(snapshot_to_memory).unwrap();
    mem.execute("UPDATE app_meta SET value = 'kr.other.app' WHERE key = 'app_id'", []).unwrap();
    let plain = mem.serialize(DatabaseName::Main).unwrap().to_vec();
    let v = migrate::current_version(&mem).unwrap();
    let bytes = seal_bytes(&plain, v, &pw(PW), NOW, "0.0.1").unwrap();
    assert_eq!(open(&bytes, &pw(PW)).unwrap_err().code, "BACKUP_OTHER_APP");
}

#[test]
fn 손상된_자료는_거절한다() {
    let w = world();
    ready(&w);
    let mem = w.db.read(snapshot_to_memory).unwrap();
    let mut plain = mem.serialize(DatabaseName::Main).unwrap().to_vec();
    let v = migrate::current_version(&mem).unwrap();
    // 두 번째 쪽(페이지)부터 한 쪽을 쓰레기로
    let page = 4096.min(plain.len() / 4);
    for b in &mut plain[page..page * 2] {
        *b = 0xA5;
    }
    let bytes = seal_bytes(&plain, v, &pw(PW), NOW, "0.0.1").unwrap();
    let e = open(&bytes, &pw(PW)).unwrap_err();
    assert!(["BACKUP_INTEGRITY", "BACKUP_INVALID", "BACKUP_OTHER_APP", "DB_ERROR"].contains(&e.code.as_str()), "{}", e.code);
}

#[test]
fn 더_새_구조의_백업은_거절한다() {
    let w = world();
    ready(&w);
    let mem = w.db.read(snapshot_to_memory).unwrap();
    let plain = mem.serialize(DatabaseName::Main).unwrap().to_vec();
    let bytes = seal_bytes(&plain, migrate::latest_version() + 1, &pw(PW), NOW, "9.9.9").unwrap();
    assert_eq!(read_header(&bytes).unwrap_err().code, "SCHEMA_TOO_NEW");
}

#[test]
fn 파일은_part_로_쓰고_끝나면_part_가_남지_않는다() {
    let dir = crate::db::testutil::tmp_dir("portable-write");
    let path = dir.join(default_file_name(NOW));
    write_atomic(&path, b"CCBACKUP-test").unwrap();
    assert!(path.exists());
    assert!(!part_path(&path).exists());
    // 쓸 수 없는 곳 — 실패하고 .part 도 남기지 않는다
    let bad = dir.join("없는 폴더").join("x.careercert-backup");
    assert!(write_atomic(&bad, b"x").is_err());
    assert!(!part_path(&bad).exists());
    let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".part")).collect();
    assert!(leftovers.is_empty());
}

#[test]
fn 기본_파일_이름에는_개인정보가_없다() {
    assert_eq!(default_file_name(NOW), "방과후강사경력관리_백업_20261001.careercert-backup");
}
