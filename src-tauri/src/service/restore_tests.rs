//! 복원 시험 — 준비(메모리) → 대기 파일 → 다음 시작에서 적용. 가상값만 쓴다.

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{Db, DB_FILE};
use crate::domain::instructor::InstructorInput;
use crate::service::instructor;
use crate::service::issuance::issuance_tests::{world_in, World};
use crate::service::issuance::{for_output, reconstruct};
use crate::service::portable::portable_tests::{package, pw, ready, PW};
use crate::service::portable::{counts, prepare};

const NOW: &str = "2026-10-02T09:00:00";

/// 파일 자료로 만든 세계: 발급 1건 + 복구 비밀번호
fn file_world(tag: &str) -> (std::path::PathBuf, World, i64) {
    let dir = tmp_dir(tag);
    let w = world_in(Db::open(&dir.join(DB_FILE)).unwrap());
    let id = ready(&w);
    (dir, w, id)
}

fn add_instructor(db: &Db, name: &str) {
    db.write(|c| instructor::create(c, InstructorInput { name: name.into(), ..Default::default() }, NOW)).unwrap();
}

/// 준비 → before_restore 백업 → 대기 파일 → 연결 닫기 → 적용
fn restore_into(db: Db, bytes: &[u8]) -> (std::path::PathBuf, Applied) {
    let path = db.path().to_path_buf();
    let prepared = prepare(bytes, &pw(PW)).unwrap();
    backup::create(&db, backup::Kind::BeforeRestore, chrono::Local::now()).unwrap();
    stage(&prepared, &path, NOW).unwrap();
    drop(db);
    let applied = apply_pending(&path, NOW);
    (path, applied)
}

#[test]
fn 같은_pc_에서_복원하면_백업_시점_자료가_되고_발급본이_그대로_열린다() {
    let (dir, w, id) = file_world("restore-same");
    let bytes = package(&w.db);
    let at_backup = w.db.read(counts).unwrap();
    let original = w.db.read(|c| reconstruct(c, id)).unwrap();
    add_instructor(&w.db, "백업 뒤 추가"); // 복원하면 사라져야 한다

    let prepared = prepare(&bytes, &pw(PW)).unwrap();
    assert!(prepared.same_windows_user);
    assert_eq!(prepared.verified_certificates, 1);
    drop(prepared);

    let (path, applied) = restore_into(w.db, &bytes);
    assert!(matches!(applied, Applied::Done { .. }), "{applied:?}");
    let db = Db::open(&path).unwrap();
    assert_eq!(db.read(counts).unwrap(), at_backup);
    let again = db.read(|c| reconstruct(c, id)).unwrap();
    assert_eq!(again.doc, original.doc, "복호화·doc_hash·재구성 모두 같다");
    assert_eq!(again.row.doc_hash, original.row.doc_hash);
    assert!(db.read(|c| for_output(c, id)).is_ok(), "정식 발급본을 다시 만들 수 있다");
    // 지금 자료의 before_restore 백업이 남아 있다
    let backups = backup::list(&dir.join("backups")).unwrap();
    assert!(backups.iter().any(|e| e.kind == backup::Kind::BeforeRestore));
    // 정리
    assert!(!pending_path(&path).exists() && !marker_path(&path).exists());
    assert!(!dir.join(format!("{DB_FILE}.replaced")).exists());
}

#[test]
fn 다른_pc_나_계정이면_복구_비밀번호로_키를_풀어_이_pc_의_dpapi_로_다시_감싼다() {
    let (_dir, w, id) = file_world("restore-other");
    // 다른 PC 흉내: 백업 안의 DPAPI 사본을 이 PC 가 풀 수 없는 값으로
    w.db.write(|c| Ok(c.execute("UPDATE key_store SET dpapi_blob = X'0102030405'", [])?)).unwrap();
    assert!(w.db.read(|c| reconstruct(c, id)).is_err(), "이 상태로는 이 PC 에서 못 연다");
    let bytes = package(&w.db);

    let prepared = prepare(&bytes, &pw(PW)).unwrap();
    assert!(!prepared.same_windows_user, "다른 PC 로 판정");
    assert_eq!(prepared.verified_certificates, 1, "재포장 뒤 메모리에서 모든 발급본 확인");
    drop(prepared);

    let (path, applied) = restore_into(w.db, &bytes);
    assert!(matches!(applied, Applied::Done { .. }), "{applied:?}");
    let db = Db::open(&path).unwrap();
    for row in db.read(|c| keys::all(c)).unwrap() {
        assert!(keys::open_dpapi(&row).is_ok(), "이 PC 의 DPAPI 로 풀린다");
        assert_ne!(row.dpapi_blob, vec![1, 2, 3, 4, 5]);
    }
    assert!(db.read(|c| for_output(c, id)).is_ok());
}

#[test]
fn 틀린_비밀번호면_준비하지_않고_지금_자료는_그대로() {
    let (_dir, w, _) = file_world("restore-wrong");
    let bytes = package(&w.db);
    let now_counts = w.db.read(counts).unwrap();
    assert_eq!(prepare(&bytes, &pw("틀린 복구 비밀번호")).unwrap_err().code, "BACKUP_UNLOCK_FAILED");
    assert!(!pending_path(w.db.path()).exists());
    assert_eq!(w.db.read(counts).unwrap(), now_counts);
}

#[test]
fn 손상된_패키지는_거절한다() {
    let (_dir, w, _) = file_world("restore-broken");
    let mut bytes = package(&w.db);
    let n = bytes.len();
    bytes[n - 40] ^= 0xFF;
    assert!(prepare(&bytes, &pw(PW)).is_err());
    assert!(!pending_path(w.db.path()).exists());
}

#[test]
fn 발급본_검증에_실패하면_준비하지_않는다() {
    let (_dir, w, _) = file_world("restore-tamper");
    // 백업 안의 발급 기록을 바꿔 둔다(트리거를 풀고) — doc_hash 가 맞지 않게
    w.db.write(|c| {
        c.execute_batch("DROP TRIGGER certificates_only_void; UPDATE certificates SET purpose = '바뀐 용도';")?;
        Ok(())
    })
    .unwrap();
    let bytes = package(&w.db);
    assert_eq!(prepare(&bytes, &pw(PW)).unwrap_err().code, "RESTORE_VERIFY_FAILED");
}

#[test]
fn 대기_파일이_바뀌었으면_적용하지_않고_지금_자료를_지킨다() {
    let (_dir, w, _) = file_world("restore-swap");
    let bytes = package(&w.db);
    add_instructor(&w.db, "지금 자료에만 있는 강사");
    let live = w.db.read(counts).unwrap();
    let path = w.db.path().to_path_buf();
    let prepared = prepare(&bytes, &pw(PW)).unwrap();
    stage(&prepared, &path, NOW).unwrap();
    drop(prepared);
    drop(w);
    // 대기 파일을 다른 내용으로
    let other = tmp_dir("restore-swap-other").join(DB_FILE);
    {
        let o = Db::open(&other).unwrap();
        o.backup_to(&pending_path(&path)).unwrap();
    }
    let applied = apply_pending(&path, NOW);
    assert!(matches!(applied, Applied::Failed { retry_later: false, .. }), "{applied:?}");
    let db = Db::open(&path).unwrap();
    assert_eq!(db.read(counts).unwrap(), live, "지금 자료 그대로");
    assert!(!pending_path(&path).exists());
}

#[test]
fn 적용_뒤_검증에_실패하면_되돌려_지금_자료를_지킨다() {
    let (dir, w, _) = file_world("restore-rollback");
    let bytes = package(&w.db);
    add_instructor(&w.db, "지금 자료에만 있는 강사");
    let live = w.db.read(counts).unwrap();
    let path = w.db.path().to_path_buf();
    let prepared = prepare(&bytes, &pw(PW)).unwrap();
    stage(&prepared, &path, NOW).unwrap();
    drop(prepared);
    drop(w);
    // 기대 건수를 일부러 틀리게 — 파일은 그대로라 ⑤ 를 지나 ⑦ 에서 걸린다
    let mut m: Marker = serde_json::from_slice(&std::fs::read(marker_path(&path)).unwrap()).unwrap();
    m.counts.instructors += 7;
    std::fs::write(marker_path(&path), serde_json::to_vec(&m).unwrap()).unwrap();

    let applied = apply_pending(&path, NOW);
    assert!(matches!(applied, Applied::Failed { .. }), "{applied:?}");
    let db = Db::open(&path).unwrap();
    assert_eq!(db.read(counts).unwrap(), live, "되돌린 지금 자료");
    assert!(!dir.join(format!("{DB_FILE}.replaced")).exists());
}

#[test]
fn 대기_중인_복원이_없으면_아무것도_하지_않는다() {
    let dir = tmp_dir("restore-none");
    assert_eq!(apply_pending(&dir.join(DB_FILE), NOW), Applied::Nothing);
}
