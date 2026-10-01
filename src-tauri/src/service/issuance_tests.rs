//! 발급 확정·불변 스냅샷 시험. 가상값만 쓴다(주민번호 모양 줄에는 `privacy:fake`).

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{Db, DB_FILE};
use crate::domain::career::{CareerInput, CareerStatus, EndReason};
use crate::domain::certificate::IssueInput;
use crate::domain::instructor::InstructorInput;
use crate::domain::settings::{SettingsInput, DEFAULT_PURPOSE, DEFAULT_TITLE};
use crate::repo::settings as settings_repo;
use crate::service::{career, instructor};

pub(crate) const NOW: &str = "2026-10-01T09:00:00";
pub(crate) const LATER: &str = "2026-11-02T09:00:00";
pub(crate) const FAKE_RRN: &str = "880808-2000002"; // privacy:fake
pub(crate) const FAKE_ADDRESS: &str = "가상시 연습구 시험로 123-45";

pub(crate) struct World {
    pub(crate) db: Db,
    pub(crate) who: i64,
    pub(crate) active: i64,
    pub(crate) ended: i64,
}

pub(crate) fn school(c: &Connection, manager: &str) {
    settings_repo::save(
        c,
        SettingsInput {
            school_name: "○○초등학교".into(),
            certificate_title: DEFAULT_TITLE.into(),
            issuer_title: "○○초등학교장".into(),
            department: "방과후학교".into(),
            manager_name: manager.into(),
            phone: "000-0000-0000".into(),
            default_purpose: DEFAULT_PURPOSE.into(),
        },
        NOW,
    )
    .unwrap();
}

pub(crate) fn career_input(program: &str, start: &str, end: Option<&str>, planned: Option<&str>) -> CareerInput {
    CareerInput {
        program_name: program.into(),
        position: "강사".into(),
        duty: format!("방과후학교 {program}"),
        start_date: start.into(),
        status: if end.is_some() { CareerStatus::Ended } else { CareerStatus::Active },
        end_date: end.map(String::from),
        end_reason: end.map(|_| EndReason::ContractEnd),
        planned_end_date: planned.map(String::from),
        memo: "".into(),
    }
}

pub(crate) fn world_in(db: Db) -> World {
    db.write(|c| {
        school(c, "가상담당");
        Ok(())
    })
    .unwrap();
    let who = db
        .write(|c| instructor::create(c, InstructorInput { name: "김가람".into(), ..Default::default() }, NOW))
        .unwrap()
        .id;
    let ended = db
        .write(|c| career::create(c, who, career_input("마술", "2025-03-05", Some("2026-02-06"), None), false, NOW))
        .unwrap()
        .id;
    let active = db
        .write(|c| career::create(c, who, career_input("마술", "2026-03-04", None, Some("2027-02-05")), false, NOW))
        .unwrap()
        .id;
    World { db, who, active, ended }
}

pub(crate) fn world() -> World {
    world_in(Db::memory())
}

pub(crate) fn request(w: &World, issue_no: &str) -> PrepareRequest {
    PrepareRequest {
        instructor_id: w.who,
        career_ids: vec![w.ended, w.active],
        issue: IssueInput {
            rrn: FAKE_RRN.into(),
            address: FAKE_ADDRESS.into(),
            issue_no: issue_no.into(),
            purpose: "기관제출".into(),
            issued_on: "2026-10-01".into(),
            mask_rrn: false,
        },
    }
}

/// 내용 확인 → (경고 모두 확인) → 발급 확정
pub(crate) fn issue_now(w: &World, issue_no: &str) -> AppResult<i64> {
    let req = request(w, issue_no);
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW))?;
    let token = review_token(&built.doc);
    let acks = built.warnings.iter().map(|x| x.ack_key()).collect();
    w.db.write(|c| issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from: None }, NOW, LOCAL_ACTOR))
}

pub(crate) fn periods(doc: &CertificateDoc) -> Vec<String> {
    doc.items.iter().map(|i| format!("{} ~ {}", i.from_text, i.to_text)).collect()
}

pub(crate) fn reopen(w: &World, id: i64) -> CertificateDoc {
    w.db.read(|c| reconstruct(c, id)).unwrap().doc
}

// ---------------- 발급과 재구성 ----------------

#[test]
fn 발급하면_스냅샷에서_같은_문서를_다시_만든다() {
    let w = world();
    let req = request(&w, "제2026-152호");
    let built = w.db.read(|c| drafting::prepare(c, req, NOW)).unwrap();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let again = reopen(&w, id);
    assert_eq!(again, built.doc, "발급 당시 문서와 똑같다");
    assert_eq!(periods(&again), vec!["2025.03.05 ~ 2026.02.06", "2026.03.04 ~ 현재"]);
    let row = w.db.read(|c| repo::get(c, id)).unwrap();
    assert_eq!(row.status, "ISSUED");
    assert_eq!(row.masked_rrn, "880808-2******");
    assert_eq!(row.rrn_display, "FULL");
    assert_eq!(row.created_by, "local");
    assert_eq!(row.source_instructor_id, Some(w.who));
}

#[test]
fn 문서_지문은_다시_만들어도_같다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let a = w.db.read(|c| reconstruct(c, id)).unwrap();
    let b = w.db.read(|c| reconstruct(c, id)).unwrap();
    assert_eq!(a.doc, b.doc);
    assert_eq!(a.row.doc_hash.len(), 64);
}

// ---------------- 불변 스냅샷 (설계안 §4.4) ----------------

#[test]
fn 재직중이던_경력을_나중에_종료해도_옛_증명서는_현재다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    // 그 뒤 실제로 종료 처리
    w.db.write(|c| career::end(c, w.active, "2027-02-05", EndReason::ContractEnd, true, LATER)).unwrap();
    assert_eq!(w.db.read(|c| career::get(c, w.active)).unwrap().period(), "2026.03.04 ~ 2027.02.05");
    // 옛 증명서는 그대로
    assert_eq!(periods(&reopen(&w, id))[1], "2026.03.04 ~ 현재");
}

#[test]
fn 강사_경력_학교_설정을_바꿔도_옛_증명서는_그대로다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let before = reopen(&w, id);

    w.db.write(|c| instructor::update(c, w.who, InstructorInput { name: "김바뀜".into(), ..Default::default() }, LATER))
        .unwrap();
    let mut changed = career_input("로봇과학", "2025-03-01", Some("2026-02-28"), Some("2026-02-28"));
    changed.position = "주강사".into();
    changed.duty = "바뀐 지도사항".into();
    w.db.write(|c| career::update(c, w.ended, changed, false, LATER)).unwrap();
    let mut planned = career_input("마술", "2026-03-04", None, Some("2027-03-31"));
    planned.memo = "예정 종료일 변경".into();
    w.db.write(|c| career::update(c, w.active, planned, false, LATER)).unwrap();
    w.db.write(|c| {
        settings_repo::save(
            c,
            SettingsInput {
                school_name: "△△초등학교".into(),
                certificate_title: "바뀐 제목".into(),
                issuer_title: "△△초등학교장".into(),
                department: "교무실".into(),
                manager_name: "새담당".into(),
                phone: "111-1111-1111".into(),
                default_purpose: "취업용".into(),
            },
            LATER,
        )
    })
    .unwrap();

    let after = reopen(&w, id);
    assert_eq!(after, before, "발급 기록은 지금 자료를 읽지 않는다");
    assert_eq!(after.holder.name, "김가람");
    assert_eq!(after.items[0].program_name, "마술");
    assert_eq!(after.school.manager_name, "가상담당");
    assert_eq!(after.title, DEFAULT_TITLE);
}

// ---------------- 확정 직전 재검증 ----------------

#[test]
fn 내용_확인_뒤에_자료가_바뀌면_발급하지_않는다() {
    let w = world();
    let req = request(&w, "제2026-152호");
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW)).unwrap();
    let token = review_token(&built.doc);
    let acks: Vec<String> = built.warnings.iter().map(|x| x.ack_key()).collect();
    // 그사이 담당자 이름이 바뀌었다
    w.db.write(|c| {
        school(c, "다른담당");
        Ok(())
    })
    .unwrap();
    let e = w
        .db
        .write(|c| issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from: None }, NOW, LOCAL_ACTOR))
        .unwrap_err();
    assert_eq!(e.code, "CERT_CHANGED");
    assert_eq!(count(&w.db, "certificates"), 0);
}

#[test]
fn 경고를_모두_확인해야_발급한다() {
    let w = world();
    let mut req = request(&w, "제2026-152호");
    req.issue.rrn = "901301-1000001".into(); // privacy:fake (13월 — 생년월일 경고)
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW)).unwrap();
    let keys: Vec<String> = built.warnings.iter().map(|x| x.ack_key()).collect();
    assert_eq!(keys, vec!["RRN_BIRTH_DATE"]);
    let token = review_token(&built.doc);
    let e = w
        .db
        .write(|c| issue(c, IssueRequest { prepare: req.clone(), review_token: token.clone(), acknowledged: vec![], copied_from: None }, NOW, LOCAL_ACTOR))
        .unwrap_err();
    assert_eq!(e.code, "CERT_WARNINGS_UNCONFIRMED");
    assert!(w.db.write(|c| issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: keys, copied_from: None }, NOW, LOCAL_ACTOR)).is_ok());
}

// ---------------- 발급번호 ----------------

#[test]
fn 같은_발급번호는_다시_쓸_수_없고_공백만_다른_것도_같은_번호다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    assert_eq!(issue_now(&w, "제2026-152호").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
    assert_eq!(issue_now(&w, "제 2026-152 호").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
    assert!(issue_now(&w, "제2026-153호").is_ok());
}

#[test]
fn 취소된_발급번호도_다시_쓸_수_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| void(c, id, "주소 오기", LATER)).unwrap();
    assert_eq!(issue_now(&w, "제2026-152호").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
}

#[test]
fn 발급번호는_원문을_지킨다() {
    let w = world();
    let id = issue_now(&w, "  제 2026 - 152 호  ").unwrap();
    let row = w.db.read(|c| repo::get(c, id)).unwrap();
    assert_eq!(row.issue_no, "제 2026 - 152 호", "앞뒤 공백만 뺀다");
    assert_eq!(reopen(&w, id).issue_no, "제 2026 - 152 호");
}

#[test]
fn db_도_발급번호_중복을_막는다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let r = w.db.read(|c| {
        Ok(c.execute(
            "INSERT INTO certificates SELECT NULL, 'u2' || substr(uuid, 3), issue_no, issue_no_key || 'x', issued_on, template_version,
                    title, purpose, holder_name, sensitive_nonce, sensitive_cipher, key_id, masked_rrn, rrn_display, issuer_title,
                    department, manager_name, phone, item_count, doc_hash, 'ISSUED', NULL, NULL, source_instructor_id, NULL,
                    created_at, created_by FROM certificates WHERE id = ?1",
            [id],
        ))
    });
    assert!(matches!(r, Ok(Err(rusqlite::Error::SqliteFailure(_, Some(ref m)))) if m.contains("UNIQUE")), "{r:?}");
}

// ---------------- DB 수준 불변 ----------------

pub(crate) fn count(db: &Db, table: &str) -> i64 {
    db.read(|c| Ok(c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)).unwrap()
}

pub(crate) fn sql(db: &Db, s: &str) -> Result<usize, String> {
    db.read(|c| Ok(c.execute(s, []).map_err(|e| e.to_string()))).unwrap()
}

#[test]
fn 발급_기록은_고치거나_지울_수_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    for s in [
        format!("UPDATE certificates SET holder_name = '다른이름' WHERE id = {id}"),
        format!("UPDATE certificates SET issue_no = '제2026-999호' WHERE id = {id}"),
        format!("UPDATE certificates SET doc_hash = substr(doc_hash, 2) || '0' WHERE id = {id}"),
        format!("UPDATE certificates SET status = 'VOIDED', voided_at = 'x', void_reason = 'x', purpose = '바꿈' WHERE id = {id}"),
        format!("DELETE FROM certificates WHERE id = {id}"),
        format!("UPDATE certificate_items SET to_text = '2027.02.05' WHERE certificate_id = {id}"),
        format!("DELETE FROM certificate_items WHERE certificate_id = {id}"),
        format!(
            "INSERT INTO certificate_items (certificate_id, seq, start_date, from_text, to_text, program_name, position, duty)
             VALUES ({id}, 3, '2020-03-02', '2020.03.02', '현재', '끼움', '강사', '끼움')"
        ),
    ] {
        let r = sql(&w.db, &s);
        assert!(r.is_err(), "거부되어야 한다: {s}");
    }
    assert_eq!(periods(&reopen(&w, id)), vec!["2025.03.05 ~ 2026.02.06", "2026.03.04 ~ 현재"]);
}

#[test]
fn 상태는_발급에서_취소로만_바뀐다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    assert!(sql(&w.db, &format!("UPDATE certificates SET status = 'VOIDED', voided_at = 'x', void_reason = '오기' WHERE id = {id}")).is_ok());
    assert!(
        sql(&w.db, &format!("UPDATE certificates SET status = 'ISSUED', voided_at = NULL, void_reason = NULL WHERE id = {id}")).is_err(),
        "취소를 되돌릴 수 없다"
    );
    assert!(sql(&w.db, &format!("UPDATE certificates SET void_reason = '사유 고침' WHERE id = {id}")).is_err(), "취소 뒤 사유도 고칠 수 없다");
}

#[test]
fn 처음부터_취소_상태로_넣을_수_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let r = sql(
        &w.db,
        &format!(
            "INSERT INTO certificates SELECT NULL, 'v' || substr(uuid, 2), 'n2', 'n2', issued_on, template_version, title, purpose,
                    holder_name, sensitive_nonce, sensitive_cipher, key_id, masked_rrn, rrn_display, issuer_title, department,
                    manager_name, phone, item_count, doc_hash, 'VOIDED', 'x', 'x', NULL, NULL, created_at, created_by
               FROM certificates WHERE id = {id}"
        ),
    );
    assert!(r.unwrap_err().contains("ISSUED"));
}

// ---------------- 취소 ----------------

#[test]
fn 취소는_사유가_필요하고_내용은_바꾸지_않는다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let before = reopen(&w, id);
    assert_eq!(w.db.write(|c| void(c, id, "   ", LATER)).unwrap_err().code, "INVALID_INPUT");
    w.db.write(|c| void(c, id, "주소 오기", LATER)).unwrap();
    let r = w.db.read(|c| reconstruct(c, id)).unwrap();
    assert_eq!(r.row.status, "VOIDED");
    assert_eq!(r.row.voided_at.as_deref(), Some(LATER));
    assert_eq!(r.row.void_reason.as_deref(), Some("주소 오기"));
    assert_eq!(r.doc, before, "내용은 그대로 (지문도 맞는다)");
    assert_eq!(w.db.write(|c| void(c, id, "다시", LATER)).unwrap_err().code, "CERT_NOT_ISSUED");
}

#[test]
fn 취소된_발급_건은_정식_출력할_수_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    assert!(w.db.read(|c| for_output(c, id)).is_ok());
    w.db.write(|c| void(c, id, "오기", LATER)).unwrap();
    let e = w.db.read(|c| for_output(c, id)).unwrap_err();
    assert_eq!(e.code, "CERT_VOIDED");
    assert_eq!(e.user_message, "취소된 발급 건입니다. 정식 출력할 수 없습니다.");
}

#[test]
fn 정식_출력_증표는_발급_기록에서만_나온다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
    assert_eq!(proof.certificate_id, id);
    assert_eq!(doc, reopen(&w, id));
    assert_eq!(w.db.read(|c| for_output(c, 999)).unwrap_err().code, "NOT_FOUND");
}

// ---------------- 변조 감지 ----------------

#[test]
fn 스냅샷이_바뀌면_지문_검증에_걸린다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    // 트리거를 일부러 걷어 내고 값을 바꾼다 (DB 파일을 직접 고친 경우를 흉내)
    sql(&w.db, "DROP TRIGGER certificates_only_void").unwrap();
    sql(&w.db, &format!("UPDATE certificates SET purpose = '바뀐 용도' WHERE id = {id}")).unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, id)).unwrap_err().code, "CERT_TAMPERED");
    assert_eq!(w.db.read(|c| for_output(c, id)).unwrap_err().code, "CERT_TAMPERED", "정식 출력도 멈춘다");
}

#[test]
fn 경력_줄이_바뀌어도_지문_검증에_걸린다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    sql(&w.db, "DROP TRIGGER certificate_items_no_update").unwrap();
    sql(&w.db, &format!("UPDATE certificate_items SET to_text = '2027.02.05' WHERE certificate_id = {id} AND seq = 2")).unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, id)).unwrap_err().code, "CERT_TAMPERED");
}

#[test]
fn 암호문이_바뀌면_열리지_않는다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    sql(&w.db, "DROP TRIGGER certificates_only_void").unwrap();
    sql(&w.db, &format!("UPDATE certificates SET sensitive_cipher = zeroblob(length(sensitive_cipher)) WHERE id = {id}")).unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, id)).unwrap_err().code, "CRYPTO_TAMPERED");
}

// ---------------- 출력 이력 ----------------

#[test]
fn 출력은_성공과_실패를_나눠_남긴다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| record_output(c, id, OutputKind::Pdf, None, true, "", NOW)).unwrap();
    w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 2 }, Some("가상 프린터"), true, "", NOW)).unwrap();
    w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 1 }, Some("가상 프린터"), false, "PRINTER_UNAVAILABLE", NOW))
        .unwrap();
    let s = w.db.read(|c| crate::service::history::detail(c, id)).unwrap();
    let rows: Vec<(String, String, Option<String>, Option<i64>)> = s
        .outputs
        .iter()
        .map(|o| (o.output_type.clone(), o.result.clone(), o.printer_name.clone(), o.copies))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("PDF".into(), "SUCCESS".into(), None, None),
            ("PRINT".into(), "SUCCESS".into(), Some("가상 프린터".into()), Some(2)),
            ("PRINT".into(), "FAILED".into(), Some("가상 프린터".into()), Some(1)),
        ]
    );
    assert!(sql(&w.db, "UPDATE certificate_outputs SET result = 'SUCCESS'").is_err(), "실패를 성공으로 고칠 수 없다");
    assert!(sql(&w.db, "DELETE FROM certificate_outputs").is_err());
}

// ---------------- 개인정보 ----------------

#[test]
fn db_와_wal_에_주민번호와_주소_평문이_없다() {
    let dir = tmp_dir("issuance-pii");
    let w = world_in(Db::open(&dir.join(DB_FILE)).unwrap());
    let id = issue_now(&w, "제2026-152호").unwrap();
    let mut masked_req = request(&w, "제2026-153호");
    masked_req.issue.mask_rrn = true;
    let built = w.db.read(|c| drafting::prepare(c, masked_req.clone(), NOW)).unwrap();
    let token = review_token(&built.doc);
    let acks = built.warnings.iter().map(|x| x.ack_key()).collect();
    w.db.write(|c| issue(c, IssueRequest { prepare: masked_req, review_token: token, acknowledged: acks, copied_from: None }, NOW, LOCAL_ACTOR))
        .unwrap();
    w.db.write(|c| void(c, id, "가상 사유", LATER)).unwrap();

    let digits: String = FAKE_RRN.chars().filter(|c| c.is_ascii_digit()).collect();
    let back = &FAKE_RRN[7..]; // 뒷자리 7자리
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        if !p.is_file() {
            continue;
        }
        files += 1;
        let b = std::fs::read(&p).unwrap();
        let hay = String::from_utf8_lossy(&b);
        for needle in [FAKE_RRN, digits.as_str(), FAKE_ADDRESS, "시험로"] {
            assert!(!hay.contains(needle), "{} 에 '{needle}' 평문", p.display());
        }
        assert!(!hay.contains(&format!("-{back}")), "{} 에 주민번호 뒷자리", p.display());
        let utf16: Vec<u8> = FAKE_ADDRESS.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert!(!b.windows(utf16.len()).any(|x| x == utf16), "UTF-16 주소");
    }
    assert!(files >= 2, "본 DB 와 WAL 을 모두 보았다 ({files}개)");
    assert!(dir.join(format!("{DB_FILE}-wal")).exists(), "WAL 파일이 있어야 검사가 의미 있다");
}

#[test]
fn key_store_에는_키_평문이_없다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    let key = w.db.read(|c| keys::current(c)).unwrap().unwrap();
    let blob: Vec<u8> = w.db.read(|c| Ok(c.query_row("SELECT dpapi_blob FROM key_store", [], |r| r.get(0))?)).unwrap();
    assert!(!blob.windows(32).any(|x| x == key.expose()), "DPAPI 로 감싼 사본에 키가 그대로 있으면 안 된다");
    assert_eq!(count(&w.db, "key_store"), 1, "키는 한 번만 만든다");
    issue_now(&w, "제2026-153호").unwrap();
    assert_eq!(count(&w.db, "key_store"), 1);
    assert!(sql(&w.db, "DELETE FROM key_store").is_err(), "키는 지울 수 없다");
}

#[test]
fn 변경_기록에는_개인정보도_발급번호도_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| void(c, id, "가상 주소 오기 사유", LATER)).unwrap();
    let log = w.db.read(audit::all).unwrap();
    let mine: Vec<&str> = log.iter().filter(|e| e.target_type == "certificate").map(|e| e.summary.as_str()).collect();
    assert_eq!(mine, vec!["발급 · 경력 2건 · 주민번호 표시 FULL", "발급 취소"]);
    for e in &log {
        for bad in ["880808", "시험로", "김가람", "제2026-152호", "사유"] {
            assert!(!e.summary.contains(bad), "{e:?}");
        }
    }
}

#[test]
fn 내용_확인_표에는_개인정보가_들어가지_않는다() {
    let w = world();
    let mut a = request(&w, "제2026-152호");
    let mut b = a.clone();
    b.issue.rrn = "900101-1000001".into(); // privacy:fake
    b.issue.address = "다른 가상 주소".into();
    a.issue.mask_rrn = false;
    let ta = w.db.read(|c| drafting::prepare(c, a, NOW)).map(|x| review_token(&x.doc)).unwrap();
    let tb = w.db.read(|c| drafting::prepare(c, b, NOW)).map(|x| review_token(&x.doc)).unwrap();
    assert_eq!(ta, tb, "주민번호·주소는 표의 입력이 아니다(그래서 표를 화면에 보내도 된다)");
}
