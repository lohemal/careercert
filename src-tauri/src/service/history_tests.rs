//! 발급이력 시험 — 찾기·상세·재출력·취소·"이 내용으로 새 증명서 작성"·복구 상태. 가상값만 쓴다.

use super::*;
use crate::db::Db;
use crate::domain::career::EndReason;
use crate::service::certificate as drafting;
use crate::service::issuance::issuance_tests::{
    issue_now, request, sql, world, World, FAKE_ADDRESS, FAKE_RRN, LATER, NOW,
};
use crate::service::issuance::{self, check_copies, for_output, record_output, review_token, IssueRequest, OutputKind, LOCAL_ACTOR};
use crate::service::{career, instructor};

const TODAY: &str = "2026-10-01";

/// 발급일·용도·가림을 골라 발급한다. 그 발급일에 넣을 수 있는 경력만 고른다.
fn issue_with(w: &World, issue_no: &str, issued_on: &str, purpose: &str, mask: bool, copied_from: Option<i64>) -> AppResult<i64> {
    let ids: Vec<i64> = w
        .db
        .read(|c| drafting::choices(c, w.who, issued_on))?
        .into_iter()
        .filter(|ch| ch.on_issue.is_ok())
        .map(|ch| ch.career.id)
        .collect();
    let mut req = request(w, issue_no);
    req.career_ids = ids;
    req.issue.issued_on = issued_on.into();
    req.issue.purpose = purpose.into();
    req.issue.mask_rrn = mask;
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW))?;
    let token = review_token(&built.doc);
    let acks = built.warnings.iter().map(|x| x.ack_key()).collect();
    w.db.write(|c| {
        issuance::issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from }, NOW, LOCAL_ACTOR)
    })
}

fn find(w: &World, f: Filter) -> Vec<String> {
    w.db.read(|c| search(c, &f)).unwrap().rows.into_iter().map(|m| m.issue_no).collect()
}

fn q(query: &str) -> Filter {
    Filter { query: query.into(), ..Default::default() }
}

fn range(from: Option<&str>, to: Option<&str>) -> Filter {
    Filter { from: from.map(String::from), to: to.map(String::from), ..Default::default() }
}

/// 발급 셋: 2025.06.01 · 2026.01.15 · 2026.10.01
fn three(w: &World) -> (i64, i64, i64) {
    let a = issue_with(w, "제2025-001호", "2025-06-01", "기관제출", false, None).unwrap();
    let b = issue_with(w, "제2026-010호", "2026-01-15", "기관제출", false, None).unwrap();
    let c = issue_with(w, "제2026-152호", "2026-10-01", "기관제출", false, None).unwrap();
    (a, b, c)
}

// ---------------- 찾기 ----------------

#[test]
fn 성명으로_찾는다() {
    let w = world();
    three(&w);
    assert_eq!(find(&w, q("가람")).len(), 3);
    assert_eq!(find(&w, q("김가람")).len(), 3);
    assert!(find(&w, q("이나래")).is_empty());
}

#[test]
fn 발급번호_일부로_찾고_공백만_다른_번호도_찾는다() {
    let w = world();
    three(&w);
    assert_eq!(find(&w, q("152")), vec!["제2026-152호"]);
    assert_eq!(find(&w, q("2026-")), vec!["제2026-152호", "제2026-010호"]);
    let spaced = issue_with(&w, "제2026- 200호", "2026-10-01", "기관제출", false, None).unwrap();
    assert!(spaced > 0);
    assert_eq!(find(&w, q("2026-200")), vec!["제2026- 200호"], "공백 없이 쳐도 찾는다");
    assert_eq!(find(&w, q(" 152 ")), vec!["제2026-152호"], "앞뒤 공백은 정리한다");
}

#[test]
fn 퍼센트와_밑줄은_와일드카드가_아니다() {
    let w = world();
    issue_with(&w, "A-1", "2026-10-01", "기관제출", false, None).unwrap();
    issue_with(&w, "A%1", "2026-10-01", "기관제출", false, None).unwrap();
    issue_with(&w, "A_1", "2026-10-01", "기관제출", false, None).unwrap();
    assert_eq!(find(&w, q("%")), vec!["A%1"]);
    assert_eq!(find(&w, q("_")), vec!["A_1"]);
    assert_eq!(find(&w, q("A_")), vec!["A_1"]);
    assert_eq!(find(&w, q("A%")), vec!["A%1"]);
}

#[test]
fn 발급일_시작만_끝만_범위로_거른다() {
    let w = world();
    three(&w);
    assert_eq!(find(&w, range(Some("2026-01-15"), None)), vec!["제2026-152호", "제2026-010호"], "시작일 포함");
    assert_eq!(find(&w, range(None, Some("2026-01-15"))), vec!["제2026-010호", "제2025-001호"], "끝 날짜 포함");
    assert_eq!(find(&w, range(Some("2025-07-01"), Some("2026-09-30"))), vec!["제2026-010호"]);
    assert_eq!(find(&w, range(Some("2026-01-15"), Some("2026-01-15"))), vec!["제2026-010호"], "같은 날도 된다");
    assert_eq!(find(&w, range(Some(""), Some(" "))).len(), 3, "빈칸은 조건 없음");
}

#[test]
fn 잘못된_기간은_거절한다() {
    let w = world();
    three(&w);
    let e = w.db.read(|c| search(c, &range(Some("2026-02-01"), Some("2026-01-31")))).unwrap_err();
    assert_eq!(e.code, "HISTORY_RANGE");
    assert!(w.db.read(|c| search(c, &range(Some("2026-02-30"), None))).is_err(), "없는 날짜");
    assert!(w.db.read(|c| search(c, &range(None, Some("2026/01/01")))).is_err(), "형식이 다르다");
    let bad = Filter { status: Some("DELETED".into()), ..Default::default() };
    assert!(w.db.read(|c| search(c, &bad)).is_err());
}

#[test]
fn 상태로_거른다() {
    let w = world();
    let (a, _, _) = three(&w);
    w.db.write(|c| issuance::void(c, a, "잘못 발급", LATER)).unwrap();
    let by = |s: &str| find(&w, Filter { status: Some(s.into()), ..Default::default() });
    assert_eq!(by("ALL").len(), 3);
    assert_eq!(by("ISSUED"), vec!["제2026-152호", "제2026-010호"]);
    assert_eq!(by("VOIDED"), vec!["제2025-001호"]);
}

#[test]
fn 최근_발급순이다_같은_날은_나중에_확정한_것이_먼저() {
    let w = world();
    // 확정 차례와 발급일 차례를 엇갈리게
    issue_with(&w, "N1", "2026-01-15", "기관제출", false, None).unwrap();
    issue_with(&w, "N2", "2026-10-01", "기관제출", false, None).unwrap();
    issue_with(&w, "N3", "2025-06-01", "기관제출", false, None).unwrap();
    issue_with(&w, "N4", "2026-10-01", "기관제출", false, None).unwrap();
    assert_eq!(find(&w, Filter::default()), vec!["N4", "N2", "N1", "N3"]);
}

// ---------------- 복호화하지 않는다 ----------------

/// 암호문과 키를 일부러 망가뜨린다 — 목록·상세·새 초안이 복호화를 한다면 여기서 실패한다.
fn break_crypto(w: &World) {
    sql(&w.db, "DROP TRIGGER certificates_only_void").unwrap();
    sql(&w.db, "UPDATE certificates SET sensitive_cipher = X'00', sensitive_nonce = X'000000000000000000000000'").unwrap();
    sql(&w.db, "UPDATE key_store SET dpapi_blob = X'00'").unwrap();
}

#[test]
fn 목록_상세_새_초안_대시보드는_복호화하지_않는다() {
    let w = world();
    let (a, _, _) = three(&w);
    break_crypto(&w);
    assert!(w.db.read(|c| issuance::reconstruct(c, a)).is_err(), "정식 출력 경로는 정말로 복호화한다");
    assert_eq!(find(&w, Filter::default()).len(), 3);
    let d = w.db.read(|c| detail(c, a)).unwrap();
    assert_eq!(d.meta.masked_rrn, "880808-2******");
    assert_eq!(d.items.len() as i64, d.meta.item_count);
    assert!(w.db.read(|c| copy_plan(c, a, TODAY)).is_ok());
    assert_eq!(w.db.read(|c| recent(c, 5)).unwrap().issued.len(), 3);
    assert_eq!(w.db.read(recovery).unwrap().state, RecoveryState::NotSet);
}

#[test]
fn 상세와_화면_응답에_주민번호와_주소_원문이_없다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let d = w.db.read(|c| detail(c, id)).unwrap();
    let json = serde_json::to_string(&crate::commands::issuance::view(d.clone())).unwrap();
    let debug = format!("{d:?}");
    for text in [&json, &debug] {
        assert!(!text.contains(FAKE_RRN), "주민번호 원문");
        assert!(!text.contains(&FAKE_RRN.replace('-', "")), "주민번호 숫자");
        assert!(!text.contains(FAKE_ADDRESS), "주소");
    }
    assert!(json.contains("880808-2******"), "가린 주민번호는 있다");
    let list = serde_json::to_string(&find(&w, Filter::default())).unwrap();
    assert!(!list.contains(FAKE_RRN) && !list.contains(FAKE_ADDRESS));
}

// ---------------- 상세 · 재출력 · 취소 ----------------

#[test]
fn 상세는_발급_당시_스냅샷이다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| career::end(c, w.active, "2026-09-30", EndReason::ContractEnd, false, LATER)).unwrap();
    let d = w.db.read(|c| detail(c, id)).unwrap();
    let periods: Vec<String> = d.items.iter().map(|i| format!("{} ~ {}", i.from_text, i.to_text)).collect();
    assert_eq!(periods, vec!["2025.03.05 ~ 2026.02.06", "2026.03.04 ~ 현재"]);
    assert_eq!(d.meta.manager_name, "가상담당");
    assert_eq!(d.meta.department, "방과후학교");
    assert_eq!(d.meta.phone, "000-0000-0000");
    assert_eq!(d.meta.issuer_title, "○○초등학교장");
    assert_eq!(d.copied_from, None);
}

#[test]
fn 발급_건은_몇_번이고_같은_내용으로_재출력하고_이력이_쌓인다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let (first, _) = w.db.read(|c| for_output(c, id)).unwrap();
    // 원장이 바뀐 뒤에도
    w.db.write(|c| career::end(c, w.active, "2026-09-30", EndReason::ContractEnd, false, LATER)).unwrap();
    for n in 1..=3 {
        let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
        assert_eq!(doc, first, "발급번호·발급일·주민번호 표시·경력·학교 정보가 그대로");
        assert_eq!(proof.certificate_id, id);
        w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 1 }, Some("가상 프린터"), true, "", LATER)).unwrap();
        assert_eq!(w.db.read(|c| detail(c, id)).unwrap().outputs.len(), n, "출력 뒤 이력이 바로 늘어난다");
    }
    assert_eq!(find(&w, Filter::default()).len(), 1, "재출력은 새 발급 기록을 만들지 않는다");
}

#[test]
fn 실패_이력에는_까닭_코드만_남는다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 2 }, Some("가상 프린터"), false, "PrinterUnavailable", LATER))
        .unwrap();
    let o = &w.db.read(|c| detail(c, id)).unwrap().outputs[0];
    assert_eq!((o.result.as_str(), o.detail.as_str(), o.copies), ("FAILED", "PrinterUnavailable", Some(2)));
}

#[test]
fn 취소하면_바로_취소_상태이고_재출력도_재취소도_안_된다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| issuance::void(c, id, "발급번호 오기", LATER)).unwrap();
    let d = w.db.read(|c| detail(c, id)).unwrap();
    assert_eq!(d.meta.status, "VOIDED");
    assert_eq!(d.meta.voided_at.as_deref(), Some(LATER));
    assert_eq!(d.meta.void_reason.as_deref(), Some("발급번호 오기"));
    assert_eq!(w.db.read(|c| for_output(c, id)).unwrap_err().code, "CERT_VOIDED");
    assert_eq!(w.db.write(|c| issuance::void(c, id, "다시", LATER)).unwrap_err().code, "CERT_NOT_ISSUED");
    assert_eq!(issue_now(&w, "제2026-152호").unwrap_err().code, "CERT_ISSUE_NO_TAKEN", "번호는 계속 못 쓴다");
}

#[test]
fn 인쇄_매수는_1부에서_5부까지() {
    for n in 1..=5 {
        assert!(check_copies(n).is_ok());
    }
    assert!(check_copies(0).is_err());
    assert!(check_copies(6).is_err());
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    assert!(w.db.write(|c| record_output(c, id, OutputKind::Print { copies: 6 }, Some("가상"), true, "", NOW)).is_err());
}

// ---------------- 이 내용으로 새 증명서 작성 ----------------

#[test]
fn 새_초안에는_발급번호_주민번호_주소가_없고_발급일은_오늘이다() {
    let w = world();
    let id = issue_with(&w, "제2026-010호", "2026-01-15", "취업용", true, None).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.issued_on, TODAY);
    assert_eq!(p.purpose, "취업용", "용도 복사");
    assert_eq!(p.rrn_display, "MASK_BACK", "주민번호 표시 방식 복사");
    assert_eq!(p.instructor_id, Some(w.who));
    // 계획에는 주민번호·주소·발급번호 칸 자체가 없다 — 원본 번호는 안내용(from_issue_no)뿐
    let all = format!("{p:?}");
    assert!(!all.contains(FAKE_RRN) && !all.contains(&FAKE_RRN.replace('-', "")) && !all.contains(FAKE_ADDRESS));
    assert!(!all.contains("880808"), "가린 주민번호도 옮기지 않는다");
    assert_eq!(p.from_issue_no, "제2026-010호");
}

#[test]
fn 원래_경력을_지금_원장에서_고르고_새_경력은_고르지_않는다() {
    let w = world();
    // 2026.01.15 발급 — 그때는 재직중 경력(2026.03.04~)이 없었다
    let id = issue_with(&w, "제2026-010호", "2026-01-15", "기관제출", false, None).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.selected_career_ids, vec![w.ended]);
    assert_eq!(p.excluded_career_ids, vec![w.active], "원래 없던 경력은 선택하지 않는다");
    assert_eq!((p.original_count, p.unlinked_count), (1, 0));
    assert!(p.notices.iter().any(|n| n.contains("기존 발급에 없던 경력 1건")));
}

#[test]
fn 발급_뒤_종료된_경력은_지금_원장_기준으로_다시_계산된다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap(); // 2026.03.04 ~ 현재
    w.db.write(|c| career::end(c, w.active, "2026-09-30", EndReason::ContractEnd, false, LATER)).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.selected_career_ids, vec![w.ended, w.active]);
    // 새 초안으로 내용을 만들면 지금 원장의 종료일이 나온다
    let mut req = request(&w, "제2026-300호");
    req.career_ids = p.selected_career_ids.clone();
    let built = w.db.read(|c| drafting::prepare(c, req, NOW)).unwrap();
    assert_eq!(built.doc.items[1].to_text, "2026.09.30");
    // 옛 발급 기록은 그대로
    assert_eq!(w.db.read(|c| detail(c, id)).unwrap().items[1].to_text, "현재");
}

#[test]
fn 연결할_수_없는_경력은_조용히_빼지_않고_알린다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| career::archive(c, w.ended, LATER)).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.selected_career_ids, vec![w.active]);
    assert_eq!((p.original_count, p.unlinked_count), (2, 1));
    assert!(
        p.notices.iter().any(|n| n
            == "기존 발급 경력 2건 중 현재 원장과 연결 가능한 1건을 선택했습니다. 1건은 보관되었거나 사용할 수 없습니다."),
        "{:?}",
        p.notices
    );
}

#[test]
fn 원래_강사가_보관되었으면_억지로_잇지_않는다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| instructor::archive(c, w.who, LATER)).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.instructor_id, None);
    assert!(p.selected_career_ids.is_empty() && p.excluded_career_ids.is_empty());
    assert!(p.notices.iter().any(|n| n.contains("원래 강사가 보관되어")));
}

#[test]
fn 취소된_건에서_시작하면_새_증명서라는_것을_알린다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| issuance::void(c, id, "오기", LATER)).unwrap();
    let p = w.db.read(|c| copy_plan(c, id, TODAY)).unwrap();
    assert_eq!(p.from_status, "VOIDED");
    assert!(p.notices[0].contains("되살리거나 재발급하는 것이 아닙니다"));
    assert_eq!(p.selected_career_ids, vec![w.ended, w.active]);
}

#[test]
fn 새_증명서를_확정하면_원본과_연결된다() {
    let w = world();
    let from = issue_now(&w, "제2026-152호").unwrap();
    let new_id = issue_with(&w, "제2026-153호", TODAY, "기관제출", false, Some(from)).unwrap();
    assert_ne!(new_id, from);
    let d = w.db.read(|c| detail(c, new_id)).unwrap();
    assert_eq!(d.meta.copied_from_certificate_id, Some(from));
    assert_eq!(d.copied_from, Some((from, "제2026-152호".to_string())));
    let e = issue_with(&w, "제2026-154호", TODAY, "기관제출", false, Some(9999)).unwrap_err();
    assert_eq!(e.code, "NOT_FOUND");
}

// ---------------- 대시보드 · 복구 상태 ----------------

#[test]
fn 대시보드는_최근_발급과_최근_취소를_보인다() {
    let w = world();
    let (a, b, c) = three(&w);
    w.db.write(|x| issuance::void(x, b, "오기", LATER)).unwrap();
    w.db.write(|x| issuance::void(x, a, "오기", "2026-11-03T09:00:00")).unwrap();
    let r = w.db.read(|x| recent(x, 5)).unwrap();
    assert_eq!((r.issued_count, r.voided_count), (1, 2));
    assert_eq!(r.issued.iter().map(|m| m.id).collect::<Vec<_>>(), vec![c]);
    assert_eq!(r.voided.iter().map(|m| m.id).collect::<Vec<_>>(), vec![a, b], "최근 취소 차례");
}

#[test]
fn 복구_설정_상태는_키가_없으면_자료_없음_있으면_미설정() {
    let w = world();
    assert_eq!(w.db.read(recovery).unwrap().state, RecoveryState::NoData);
    issue_now(&w, "제2026-152호").unwrap();
    let r = w.db.read(recovery).unwrap();
    assert_eq!((r.state, r.certificates), (RecoveryState::NotSet, 1));
    assert!(r.state.message().contains("이 PC의 현재 Windows 사용자 계정에서만 열 수 있습니다"));
    // Phase 8 이 복구 사본을 채우면 준비 완료로 바뀐다
    sql(&w.db, "UPDATE key_store SET recovery_blob = X'01'").unwrap();
    assert_eq!(w.db.read(recovery).unwrap().state, RecoveryState::Ready);
}

#[test]
fn 빈_db_에서도_목록은_비어_있다() {
    let db = Db::memory();
    assert!(db.read(|c| search(c, &Filter::default())).unwrap().rows.is_empty());
}
