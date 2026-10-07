//! v0.1.3 오발급 폐기 시험 — 발급 취소와 다르게, ISSUED 기록을 지우고 발급번호를 다시 쓸 수 있게 한다.
//! 가상값만 쓴다(주민번호 모양 줄에는 `privacy:fake`).

use image::ImageFormat;

use super::issuance_tests::{count, issue_now, request, sql, world, World, FAKE_ADDRESS, FAKE_RRN, NOW};
use super::*;
use crate::domain::logo::logo_tests::fake_logo;
use crate::render::{self, Mode};
use crate::service::{history, reminder};

const REASON: &str = "발급번호 오입력";

fn req(no: &str) -> DiscardRequest {
    DiscardRequest { reason: REASON.into(), issue_no_confirm: no.into(), outputs_acknowledged: false }
}

fn discard_now(w: &World, id: i64, r: &DiscardRequest) -> AppResult<()> {
    w.db.write(|c| discard(c, id, r, NOW))
}

fn code(r: AppResult<()>) -> String {
    r.unwrap_err().code
}

fn issue_copy(w: &World, no: &str, from: i64) -> i64 {
    let req = request(w, no);
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW)).unwrap();
    let token = review_token(&built.doc);
    let acks = built.warnings.iter().map(|x| x.ack_key()).collect();
    w.db.write(|c| {
        issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from: Some(from) }, NOW, LOCAL_ACTOR)
    })
    .unwrap()
}

fn printed(w: &World, id: i64) {
    w.db.write(|c| record_output(c, id, OutputKind::Pdf, None, true, "", NOW)).unwrap();
}

fn audit_rows(w: &World) -> Vec<crate::repo::audit::Entry> {
    w.db.read(crate::repo::audit::all).unwrap()
}

// ---------------- 폐기와 발급번호 재사용 ----------------

#[test]
fn 발급된_건을_폐기하면_딸린_것까지_지워지고_같은_번호를_다시_쓴다() {
    let w = world();
    let id = issue_now(&w, "TEST-001").unwrap();
    assert_eq!(count(&w.db, "certificate_items"), 2);
    discard_now(&w, id, &req("TEST-001")).unwrap();
    assert_eq!(count(&w.db, "certificates"), 0);
    assert_eq!(count(&w.db, "certificate_items"), 0);
    assert!(!w.db.read(crate::repo::discard::gate_open).unwrap(), "허가는 남지 않는다");
    assert_eq!(w.db.read(|c| crate::repo::certificate::find(c, id)).unwrap().map(|r| r.id), None);
    let again = issue_now(&w, "TEST-001").unwrap();
    assert!(again > id, "지운 발급 번호(내부 id)는 다시 쓰지 않는다 — 변경 기록이 가리킨다");
    assert_eq!(w.db.read(|c| crate::repo::certificate::get(c, again)).unwrap().issue_no, "TEST-001");
}

#[test]
fn 공백만_다른_번호도_폐기_뒤에는_쓸_수_있다() {
    let w = world();
    let id = issue_now(&w, "2026-가-0001").unwrap();
    assert_eq!(issue_now(&w, "2026-가- 0001").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
    discard_now(&w, id, &req("2026-가-0001")).unwrap();
    issue_now(&w, "2026-가- 0001").unwrap();
}

#[test]
fn 발급_취소한_번호는_계속_쓸_수_없고_폐기도_안_된다() {
    let w = world();
    let id = issue_now(&w, "TEST-002").unwrap();
    w.db.write(|c| void(c, id, "효력 취소", NOW)).unwrap();
    assert_eq!(code(discard_now(&w, id, &req("TEST-002"))), "CERT_NOT_ISSUED");
    assert_eq!(issue_now(&w, "TEST-002").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
    assert_eq!(issue_now(&w, " TEST- 002 ").unwrap_err().code, "CERT_ISSUE_NO_TAKEN");
    assert_eq!(count(&w.db, "certificates"), 1, "취소 기록은 그대로");
    // DB 도 취소된 건에는 폐기 허가를 주지 않는다
    assert!(sql(&w.db, &format!("INSERT INTO certificate_discard_gate VALUES ({id})")).unwrap_err().contains("only ISSUED"));
}

#[test]
fn 사유와_발급번호_재입력이_맞아야_한다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let blank = DiscardRequest { reason: "   ".into(), ..req("제2026-152호") };
    assert_eq!(code(discard_now(&w, id, &blank)), "INVALID_INPUT");
    let long = DiscardRequest { reason: "가".repeat(DISCARD_REASON_MAX + 1), ..req("제2026-152호") };
    assert_eq!(code(discard_now(&w, id, &long)), "INVALID_INPUT");
    for wrong in ["", "제2026-153호", "제 2026-152호", "제2026-152"] {
        assert_eq!(code(discard_now(&w, id, &req(wrong))), "CERT_DISCARD_MISMATCH", "{wrong:?}");
    }
    assert_eq!(count(&w.db, "certificates"), 1, "거절되면 그대로");
    // 앞뒤 공백만은 원문 규칙대로 정리한다
    discard_now(&w, id, &req("  제2026-152호 ")).unwrap();
}

#[test]
fn 출력한_적이_있으면_따로_확인해야_폐기된다() {
    let w = world();
    let id = issue_now(&w, "TEST-003").unwrap();
    printed(&w, id);
    w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 1 }, Some("가상 프린터"), true, "", NOW)).unwrap();
    assert_eq!(code(discard_now(&w, id, &req("TEST-003"))), "CERT_DISCARD_OUTPUTS_UNCONFIRMED");
    assert_eq!(count(&w.db, "certificate_outputs"), 2);
    discard_now(&w, id, &DiscardRequest { outputs_acknowledged: true, ..req("TEST-003") }).unwrap();
    assert_eq!(count(&w.db, "certificate_outputs"), 0);
    assert_eq!(count(&w.db, "certificates"), 0);
}

#[test]
fn 실패한_출력만_있으면_추가_확인은_없다() {
    let w = world();
    let id = issue_now(&w, "TEST-004").unwrap();
    w.db.write(|c| record_output(c, id, OutputKind::Pdf, None, false, "", NOW)).unwrap();
    discard_now(&w, id, &req("TEST-004")).unwrap();
    assert_eq!(count(&w.db, "certificate_outputs"), 0);
}

// ---------------- 로고 보관본 ----------------

#[test]
fn 다른_발급본이_안_쓰는_로고_보관본만_함께_지운다() {
    let w = world();
    w.db.write(|c| crate::service::school_logo::set(c, &fake_logo(300, 300, ImageFormat::Png), NOW)).unwrap();
    let a = issue_now(&w, "TEST-010").unwrap();
    let b = issue_now(&w, "TEST-011").unwrap();
    assert_eq!(count(&w.db, "certificate_logos"), 1, "같은 로고는 한 번");
    discard_now(&w, a, &req("TEST-010")).unwrap();
    assert_eq!(count(&w.db, "certificate_logos"), 1, "b 가 쓰고 있다");
    let html = |id| {
        let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
        render::render(&doc, Mode::Issued(proof)).unwrap()
    };
    assert!(html(b).contains("<div class=\"logo-mark\"><img"), "b 는 로고 그대로");
    discard_now(&w, b, &req("TEST-011")).unwrap();
    assert_eq!(count(&w.db, "certificate_logos"), 0);
    // 학교 설정의 지금 로고는 건드리지 않는다
    assert!(w.db.read(crate::repo::logo::current).unwrap().is_some());
}

// ---------------- copied_from ----------------

#[test]
fn 이_발급본을_참고한_새_발급본은_그대로_남고_참조만_비운다() {
    let w = world();
    let parent = issue_now(&w, "TEST-020").unwrap();
    let child = issue_copy(&w, "TEST-021", parent);
    w.db.write(|c| void(c, child, "효력 취소", NOW)).ok();
    let child2 = issue_copy(&w, "TEST-022", parent);
    let before = |id| {
        let row = w.db.read(|c| crate::repo::certificate::get(c, id)).unwrap();
        (row.doc_hash.clone(), row.status.clone())
    };
    let snap = |id| {
        let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
        render::render(&doc, Mode::Issued(proof)).unwrap()
    };
    let (h1, h2) = (before(child), before(child2));
    let out2 = snap(child2);
    discard_now(&w, parent, &req("TEST-020")).unwrap();
    assert_eq!(before(child), h1, "취소된 자식도 doc_hash·상태 그대로");
    assert_eq!(before(child2), h2);
    assert_eq!(snap(child2), out2, "자식 출력 결과 그대로(지문 검증 통과)");
    let meta = w.db.read(|c| crate::repo::certificate::get_meta(c, child2)).unwrap();
    assert_eq!(meta.copied_from_certificate_id, None);
    assert_eq!(count(&w.db, "certificates"), 2);
    // 지운 뒤에는 그 칸을 아무도 고칠 수 없다 (다시 막힌다)
    assert!(sql(&w.db, &format!("UPDATE certificates SET copied_from_certificate_id = {child} WHERE id = {child2}")).is_err());
}

// ---------------- 감사 기록 ----------------

#[test]
fn 감사_기록에는_폐기_사실과_내부_식별자만_남는다() {
    let w = world();
    let id = issue_now(&w, "제2026-777호").unwrap();
    printed(&w, id);
    let uuid = w.db.read(|c| crate::repo::certificate::get(c, id)).unwrap().uuid;
    let reason = "주소 가상시 연습구 오기 900101-1000001"; // privacy:fake
    discard_now(&w, id, &DiscardRequest { reason: reason.into(), outputs_acknowledged: true, ..req("제2026-777호") }).unwrap();
    let log = audit_rows(&w);
    let e = log.iter().find(|e| e.action == "CERTIFICATE_DISCARDED").expect("폐기 기록");
    assert_eq!(e.target_type, "certificate");
    assert_eq!(e.target_id, Some(id));
    assert!(e.summary.starts_with("오발급 폐기 · 발급번호 재사용 가능"), "{}", e.summary);
    assert!(e.summary.contains(&uuid));
    assert!(e.summary.contains("출력 이력 1건"));
    for (k, bad) in [("발급번호", "2026-777"), ("성명", "김가람"), ("사유", "오기"), ("주소", "연습구"), ("주민번호", "900101"), ("주민번호2", &FAKE_RRN[..6]), ("주소2", &FAKE_ADDRESS[..6])] {
        assert!(log.iter().all(|e| !e.summary.contains(bad)), "{k} 가 기록에 있다");
    }
    // 발급 기록은 지워졌지만 '발급했다'·'폐기했다' 기록은 남는다
    assert!(log.iter().any(|e| e.action == "CERTIFICATE_ISSUE" && e.target_id == Some(id)));
}

// ---------------- 화면에 보이는 것 ----------------

#[test]
fn 폐기하면_발급이력과_대시보드_건수에서_바로_빠진다() {
    let w = world();
    let a = issue_now(&w, "TEST-030").unwrap();
    issue_now(&w, "TEST-031").unwrap();
    let r = w.db.read(|c| history::recent(c, 10)).unwrap();
    assert_eq!((r.issued_count, r.issued.len()), (2, 2));
    discard_now(&w, a, &req("TEST-030")).unwrap();
    let r = w.db.read(|c| history::recent(c, 10)).unwrap();
    assert_eq!((r.issued_count, r.voided_count), (1, 0));
    assert!(r.issued.iter().all(|m| m.id != a));
    let page = w.db.read(|c| history::search(c, &history::Filter::default())).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert!(page.rows.iter().all(|m| m.issue_no != "TEST-030"));
    assert_eq!(w.db.read(|c| history::detail(c, a)).unwrap_err().code, "NOT_FOUND");
}

#[test]
fn 폐기는_이동용_백업_뒤_변경으로_잡힌다() {
    let w = world();
    let id = issue_now(&w, "TEST-040").unwrap();
    w.db.write(|c| reminder::record_export(c, NOW)).unwrap();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    assert_eq!(w.db.read(|c| reminder::reminder(c, today)).unwrap().state, reminder::State::Current);
    discard_now(&w, id, &req("TEST-040")).unwrap();
    assert_eq!(w.db.read(|c| reminder::reminder(c, today)).unwrap().state, reminder::State::Changed);
}

// ---------------- 일반 경로는 계속 막힌다 ----------------

#[test]
fn 허가_없는_수정과_삭제는_여전히_거절된다() {
    let w = world();
    let id = issue_now(&w, "TEST-050").unwrap();
    printed(&w, id);
    let other = issue_copy(&w, "TEST-051", id);
    for s in [
        format!("DELETE FROM certificates WHERE id = {id}"),
        format!("DELETE FROM certificate_items WHERE certificate_id = {id}"),
        format!("DELETE FROM certificate_outputs WHERE certificate_id = {id}"),
        format!("UPDATE certificates SET copied_from_certificate_id = NULL WHERE id = {other}"),
        format!("UPDATE certificates SET holder_name = '다른이름' WHERE id = {id}"),
        format!("UPDATE certificate_items SET to_text = 'x' WHERE certificate_id = {id}"),
        format!("UPDATE certificate_outputs SET result = 'FAILED' WHERE certificate_id = {id}"),
        "DELETE FROM certificate_logos".to_string(),
    ] {
        let before = (count(&w.db, "certificates"), count(&w.db, "certificate_items"), count(&w.db, "certificate_outputs"));
        let r = sql(&w.db, &s);
        assert!(r.is_err() || r == Ok(0), "{s} → {r:?}");
        let after = (count(&w.db, "certificates"), count(&w.db, "certificate_items"), count(&w.db, "certificate_outputs"));
        assert_eq!(before, after, "{s}");
    }
    // 허가가 있어도 다른 발급 기록의 내용은 못 고친다 (copied_from 을 비우는 것만)
    let r = w.db.write(|c| {
        c.execute(&format!("INSERT INTO certificate_discard_gate VALUES ({id})"), [])?;
        let e = c.execute(&format!("UPDATE certificates SET purpose = '바꿈' WHERE id = {other}"), []).map_err(|e| e.to_string());
        let e2 = c
            .execute(&format!("UPDATE certificates SET copied_from_certificate_id = NULL, purpose = '바꿈' WHERE id = {other}"), [])
            .map_err(|e| e.to_string());
        c.execute("DELETE FROM certificate_discard_gate", [])?;
        Ok((e, e2))
    });
    let (e, e2) = r.unwrap();
    assert!(e.unwrap_err().contains("immutable"));
    assert!(e2.unwrap_err().contains("immutable"));
}

#[test]
fn 폐기_뒤에도_남은_발급본은_이동용_백업으로_검증된다() {
    use crate::service::portable::portable_tests::{package, pw, PW};
    let w = world();
    let a = issue_now(&w, "TEST-060").unwrap();
    let b = issue_copy(&w, "TEST-061", a);
    printed(&w, b);
    discard_now(&w, a, &req("TEST-060")).unwrap();
    w.db.write(|c| crate::service::recovery::set(c, &pw(PW), &pw(PW), NOW)).unwrap();
    let bytes = package(&w.db);
    let p = crate::service::portable::prepare(&bytes, &pw(PW)).unwrap();
    assert_eq!(p.verified_certificates, 1);
}
