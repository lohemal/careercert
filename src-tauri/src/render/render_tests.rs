//! 양식 HTML 시험. 가상값만 쓴다(주민번호 모양 줄에는 `privacy:fake`).
//! 쪽 수·A4·글꼴처럼 실제 PDF 를 봐야 하는 것은 `scripts/print-check.mjs`(앱을 띄워 확인)가 본다.

use chrono::NaiveDate;

use super::*;
use crate::domain::career::{Career, CareerFields, EndReason, Term};
use crate::domain::certificate::{build, IssueInput};
use crate::domain::instructor::Instructor;
use crate::domain::settings::{SchoolSettings, DEFAULT_PURPOSE, DEFAULT_TITLE};

const FAKE_RRN: &str = "880808-2000002"; // privacy:fake

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn who(name: &str) -> Instructor {
    Instructor {
        id: 1,
        uuid: "u".repeat(36),
        name: name.into(),
        distinguisher: "구분메모는증명서에안나온다".into(),
        phone: "".into(),
        memo: "".into(),
        archived_at: None,
        created_at: "".into(),
        updated_at: "".into(),
    }
}

fn school() -> SchoolSettings {
    SchoolSettings {
        school_name: "○○초등학교".into(),
        certificate_title: DEFAULT_TITLE.into(),
        issuer_title: "○○초등학교장".into(),
        department: "방과후학교".into(),
        manager_name: "가상담당".into(),
        phone: "000-0000-0000".into(),
        default_purpose: DEFAULT_PURPOSE.into(),
        updated_at: None,
    }
}

fn careers(n: usize) -> Vec<Career> {
    (0..n)
        .map(|i| {
            let start = ymd(2000 + i as i32 % 26, 3, 4);
            Career {
                id: i as i64 + 1,
                uuid: "c".repeat(36),
                instructor_id: 1,
                fields: CareerFields {
                    program_name: format!("프로그램{}", i + 1),
                    position: "강사".into(),
                    duty: format!("방과후학교 프로그램{}", i + 1),
                    start_date: start,
                    term: Term::Ended {
                        end_date: start + chrono::Duration::days(300),
                        reason: EndReason::ContractEnd,
                    },
                    planned_end_date: Some(ymd(2030, 1, 1)),
                    memo: "".into(),
                },
                import_id: None,
                archived_at: None,
                created_at: "".into(),
                updated_at: "".into(),
            }
        })
        .collect()
}

fn doc_with(name: &str, n: usize, mask: bool, s: &SchoolSettings) -> CertificateDoc {
    build(
        &who(name),
        &careers(n),
        IssueInput {
            rrn: FAKE_RRN.into(),
            address: "가상시 연습구 시험로 1".into(),
            issue_no: "제2026-152호".into(),
            purpose: "기관제출".into(),
            issued_on: "2026-10-01".into(),
            mask_rrn: mask,
        },
        s,
        ymd(2026, 10, 1),
    )
    .unwrap()
    .doc
}

fn doc(n: usize) -> CertificateDoc {
    doc_with("김가람", n, false, &school())
}

fn issued() -> Mode {
    Mode::Issued(IssuedProof::from_issued_record(1))
}

fn html(d: &CertificateDoc, mode: Mode) -> String {
    render(d, mode).unwrap()
}

fn count(h: &str, needle: &str) -> usize {
    h.matches(needle).count()
}

#[test]
fn 사용자_값은_모두_이스케이프한다() {
    let mut s = school();
    s.certificate_title = "<script>alert(1)</script>".into();
    s.issuer_title = "\"><img src=x onerror=alert(1)>".into();
    s.department = "부서 & 팀".into();
    let mut d = doc_with("김<b>가람</b>", 1, false, &s);
    d.issue_no = "</style><script>x</script>".into();
    d.items[0].duty = "지도 '사항' <i>".into();
    let h = html(&d, Mode::PreviewDraft);
    assert!(!h.contains("<script>"), "스크립트가 그대로 들어가면 안 된다");
    assert!(!h.contains("<img"), "속성 밖으로 빠져나가면 안 된다");
    assert!(!h.contains("<b>가람"));
    assert!(!h.contains("<i>"));
    assert!(h.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(h.contains("부서 &amp; 팀"));
    assert!(h.contains("지도 &#39;사항&#39; &lt;i&gt;"));
    // 쪽 아래 글자(CSS 문자열)로도 <style> 을 닫고 나갈 수 없다
    assert_eq!(count(&h, "</style>"), 1, "style 은 하나만 닫힌다");
    assert!(h.contains(r"\3C /style\3E "));
}

#[test]
fn 제목과_학교_정보가_들어간다() {
    let mut s = school();
    s.certificate_title = "방과후학교 강사 경력증명서".into();
    let h = html(&doc_with("김가람", 2, false, &s), issued());
    assert!(h.contains("<h1 class=\"title\">방과후학교 강사 경력증명서</h1>"));
    assert!(h.contains("<title>방과후학교 강사 경력증명서</title>"), "PDF 제목 정보에는 문서 제목만");
    assert!(h.contains("○○초등학교장<span class=\"seal\">(직인)</span>"));
    assert!(h.contains("<th>담당부서</th><td>방과후학교</td>"));
    assert!(h.contains("<th>담당자</th><td>가상담당 (인)</td>"));
    assert!(h.contains("<th>전화번호</th><td>000-0000-0000</td>"));
    assert!(!h.contains("전학번호"), "오타를 바로잡았다(D12)");
    assert!(!h.contains("한 글") && !h.contains(">한글<"), "'한 글' 칸은 없다(D1)");
    assert!(h.contains("<p class=\"issued\">2026. 10. 1.</p>"));
    assert!(h.contains("발급번호:<b>제2026-152호</b>"));
    assert!(h.contains("<p class=\"statement\">위와 같이 증명합니다.</p>"));
    assert!(!h.contains("구분메모는증명서에안나온다"), "구분 메모는 증명서에 들어가지 않는다");
    assert!(!h.contains("<img"), "직인 이미지를 넣지 않는다(D8)");
}

#[test]
fn 주민번호는_표시_방식대로_찍는다() {
    let full = html(&doc_with("김가람", 1, false, &school()), Mode::PreviewDraft);
    assert!(full.contains(">880808-2000002<")); // privacy:fake
    let masked_doc = doc_with("김가람", 1, true, &school());
    let masked = html(&masked_doc, Mode::PreviewDraft);
    assert!(masked.contains(">880808-2******<"));
    assert!(!masked.contains("2000002"), "가림이면 뒷자리가 어디에도 없다");
    assert_eq!(masked_doc.holder.rrn.as_str(), FAKE_RRN, "원본은 그대로");
    assert_eq!(rrn_text(&masked_doc), "880808-2******");
}

#[test]
fn 주민번호와_주소는_본문에만_있고_title_이나_쪽_글자에는_없다() {
    let h = html(&doc(1), Mode::PreviewDraft);
    let head = &h[..h.find("</head>").unwrap()];
    assert!(!head.contains("880808"), "head(제목·쪽 글자)에 주민번호가 없다"); // privacy:fake
    assert!(!head.contains("시험로"), "head 에 주소가 없다");
    assert!(head.contains(r#""제2026-152호""#) && head.contains(r#""김가람""#), "쪽 글자에는 발급번호·성명");
}

#[test]
fn 예정_종료일은_증명서에_나오지_않는다() {
    // careers() 는 모두 예정 종료일 2030-01-01 을 갖는다
    let h = html(&doc(3), issued());
    assert!(!h.contains("2030"), "{h}");
    assert!(!h.contains("예정"));
}

#[test]
fn 미리보기에는_워터마크가_있고_발급본에는_없다() {
    let d = doc(2);
    let draft = html(&d, Mode::PreviewDraft);
    assert!(draft.contains("class=\"draft-mark\""));
    assert!(draft.contains(DRAFT_MARK));
    assert!(draft.contains("@top-center"), "쪽 위에도 알림");
    let real = html(&d, issued());
    assert!(!real.contains("<div class=\"draft-mark\""));
    assert!(!real.contains(DRAFT_MARK));
    assert!(!real.contains("@top-center"));
}

#[test]
fn 경력이_적으면_빈_줄로_5줄을_채운다() {
    for (n, items, blanks) in [(1, 1, 4), (5, 5, 0), (6, 6, 0), (15, 15, 0), (40, 40, 0)] {
        let h = html(&doc(n), issued());
        assert_eq!(count(&h, "<tr class=\"item\">"), items, "{n}건");
        assert_eq!(count(&h, "<tr class=\"blank\">"), blanks, "{n}건");
    }
}

#[test]
fn 경력_머리글은_쪽마다_되풀이되는_thead_다() {
    let h = html(&doc(40), issued());
    assert_eq!(count(&h, "<thead>"), 1);
    assert!(h.contains(".career thead { display: table-header-group; }"));
    assert!(h.contains("<th>부 터</th><th>까 지</th>"));
    assert!(h.contains(".career tr { break-inside: avoid; }"), "한 줄이 두 쪽에 걸쳐 잘리지 않는다");
}

#[test]
fn 쪽마다_발급번호_성명_쪽번호를_찍는다() {
    let h = html(&doc(15), issued());
    assert!(h.contains("@bottom-right"));
    assert!(h.contains(r#"content: "발급번호 " "제2026-152호" " · " "김가람" " · " counter(page) " / " counter(pages)"#));
}

#[test]
fn 마지막_덩어리는_쪼개지지_않는다() {
    let h = html(&doc(6), issued());
    assert!(h.contains(".closing { break-inside: avoid; page-break-inside: avoid; }"));
    let closing = &h[h.find("<div class=\"closing\">").unwrap()..];
    for part in ["용도", "위와 같이 증명합니다.", "2026. 10. 1.", "○○초등학교장", "담당부서", "담당자", "전화번호"] {
        assert!(closing.contains(part), "{part} 가 마지막 덩어리 안에 있어야 한다");
    }
}

#[test]
fn a4_와_내장_글꼴을_쓴다() {
    let h = html(&doc(1), issued());
    assert!(h.contains("size: A4 portrait"));
    assert!(h.contains("url('/fonts/NotoSansKR-Regular.ttf')") && h.contains("url('/fonts/NotoSansKR-Bold.ttf')"));
    assert!(h.contains("font-family: CertSans;"));
    assert!(!h.contains("Malgun") && !h.contains("맑은"), "시스템 글꼴로 대신하지 않는다");
}

#[test]
fn 경력_줄은_발급일_기준_부터_까지를_쓴다() {
    let h = html(&doc(1), issued());
    assert!(h.contains("<td class=\"period\">2000.03.04</td><td class=\"period\">2000.12.29</td>"), "{h}");
}

#[test]
fn 모르는_양식_판은_그리지_않는다() {
    let mut d = doc(1);
    d.template_version = 99;
    assert_eq!(render(&d, issued()).unwrap_err().code, "TEMPLATE_UNKNOWN");
}

#[test]
fn css_문자열은_따옴표와_역슬래시도_안전하다() {
    assert_eq!(css_string(r#"a"b\c"#), r#""a\22 b\5C c""#);
}
