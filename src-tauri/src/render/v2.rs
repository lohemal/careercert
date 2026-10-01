//! 증명서 양식 2판 (v0.1.2). **배포 뒤에는 고치지 않는다** — 바꿀 것은 v3 으로.
//!
//! 1판(`v1.rs`, v0.1.0·v0.1.1 발급본)을 그대로 옮기고 두 가지만 바꿨다.
//!   1. `경력 사항` 글자를 경력표 왼쪽 큰 칸(머리글 2줄 + 경력 줄)의 **가로·세로 가운데**에 둔다.
//!      1판은 머리글 칸에 넣어 위쪽에 붙어 보였다. 줄을 묶은 칸(rowspan)은 쓰지 않는다 — 묶으면 줄이
//!      쪽을 넘지 못해 경력표 전체가 다음 쪽으로 밀린다. 줄마다의 왼쪽 칸(위·아래 선 없음, 1판과 같다) 위에
//!      글자 상자(`.side-label`)를 겹쳐 경력 영역 덩어리의 높이 가운데에 둔다.
//!   2. 발급 당시 학교 로고(`doc.logo`)를 경력표 뒤에 **연한 흑백 워터마크**로 깐다.
//!      그림은 `domain::logo::watermark_png` 가 미리 흑백·옅게 만든 것(CSS filter·opacity 에 기대지 않음 —
//!      미리보기·PDF·프린터가 같다). 머리글·경력 사항 칸을 뺀 경력 줄 영역 가운데, 표 글자 뒤(z-index -1)에
//!      `<img>` 로 두고 max-width/max-height 로 종횡비를 지킨 채 영역 안에만 그린다.
//!
//! 여러 쪽: WebView2 는 쪽이 갈라진 상자에 겹친 그림을 쪽마다 다시 그리지 않는다. 그래서 출력 창
//! (`src/print-split.ts`)이 실제 글꼴·쪽 크기로 줄 높이를 재서 경력 영역(`.career-zone[data-split]`)을
//! **쪽마다 한 덩어리**로 나눈다 — 덩어리마다 머리글·`경력 사항`·로고가 있고, 다음 덩어리는 새 쪽에서 시작한다.
//! 쪽마다 들어가는 줄 수는 원래 쪽 나눔(1판)과 같다. 줄이 아주 적은 덩어리에는 로고를 넣지 않는다.
//! 마지막 덩어리(용도·학교장·담당)만 있는 쪽에는 경력 영역이 없으므로 로고도 없다. 로고가 없으면 글자만.
//!
//! 기존 엑셀 양식(경력증명서 시트 A1:G26)의 배치를 따른다.
//!
//! ```text
//!   ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
//!   ┃ 발급번호: 제2026-152호                    ┃
//!   ┃        {문서 제목}                         ┃
//!   ┃ 인적 ┃ 성 명 ┃ 김으뜸 ┃ 주민번호 ┃ …       ┃   ← '한 글' 칸은 뺐다(D1)
//!   ┃ 사항 ┃ 주 소 ┃ …                          ┃
//!   ┃ 경력 ┃ 기간(부터·까지) 프로그램명 직위 지도사항 ┃ ← 머리글은 쪽마다 되풀이
//!   ┃ 사항 ┃ … 최소 5줄(빈 줄로 채움), 넘치면 다음 쪽 ┃
//!   ┃ 용도 ┃ 기관제출                            ┃ ┐
//!   ┃        위와 같이 증명합니다.               ┃ │ 쪼개지지 않는 마지막 덩어리
//!   ┃        2026. 10. 1.                        ┃ │
//!   ┃        ○○초등학교장 (직인)                ┃ │
//!   ┃ 담당부서 / 담당자 (인) / 전화번호          ┃ ┘ ← '전학번호' 를 바로잡았다(D12)
//!   ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
//!                       발급번호 · 성명 · 1 / 2  ← 쪽 아래 작은 글씨(모든 쪽)
//! ```
//!
//! * 표의 열 6개(라벨 · 부터 · 까지 · 프로그램명 · 직위 · 지도사항)를 인적사항·경력·용도가 함께 쓴다.
//! * 직인·도장은 이미지로 넣지 않는다(D8) — `(직인)`·`(인)` 글자만.
//! * 글꼴은 앱에 넣은 Noto Sans KR(Regular·Bold)만 쓴다. 다른 글꼴로 대신하지 않는다.

use super::{css_string, esc, rrn_text, Mode, DRAFT_MARK};
use crate::domain::certificate::CertificateDoc;
use crate::domain::date;
use crate::domain::logo;
use crate::error::AppResult;

/// 첫 쪽에 늘 보이는 경력 줄 수 (기존 양식 10~14행)
pub const MIN_ROWS: usize = 5;

const STYLE: &str = r#"
@font-face { font-family: CertSans; src: url('/fonts/NotoSansKR-Regular.ttf') format('truetype'); font-weight: 400; font-style: normal; }
@font-face { font-family: CertSans; src: url('/fonts/NotoSansKR-Bold.ttf') format('truetype'); font-weight: 700; font-style: normal; }
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; background: #fff; color: #000; }
body { font-family: CertSans; font-size: 10.5pt; line-height: 1.35; -webkit-print-color-adjust: exact; print-color-adjust: exact; }

/* 양식 테두리 — 쪽이 나뉘면 쪽마다 닫는다 */
.sheet { border: 1.6pt solid #000; box-decoration-break: clone; -webkit-box-decoration-break: clone; padding: 0; }

.no { padding: 8pt 9pt 0; font-size: 10pt; letter-spacing: 0.02em; }
.no b { font-weight: 400; margin-left: 6pt; }
.title { margin: 0; padding: 20pt 8pt 24pt; text-align: center; font-size: 17pt; font-weight: 700; letter-spacing: 0.04em; border-bottom: 1.2pt solid #000; }

table { width: 100%; border-collapse: collapse; table-layout: fixed; }
col.c-label { width: 11%; } col.c-from { width: 15%; } col.c-to { width: 15%; }
col.c-prog { width: 17%; } col.c-pos { width: 10%; } col.c-duty { width: 32%; }
th, td { border: 0.6pt solid #000; padding: 4pt 5pt; vertical-align: middle; word-break: keep-all; overflow-wrap: anywhere; }
th { font-weight: 400; text-align: center; }
.sheet table tr > :first-child { border-left: none; }
.sheet table tr > :last-child { border-right: none; }
.label { text-align: center; letter-spacing: 0.3em; }
.label.tight { letter-spacing: 0.05em; white-space: nowrap; }

.person td, .person th { height: 36pt; }
.person .val { text-align: center; }
.person .addr { text-align: left; padding-left: 8pt; }
.person tr:first-child > * { border-top: none; }

.career thead { display: table-header-group; }
.career thead th { height: 20pt; font-size: 10.5pt; }
.career tr { break-inside: avoid; }
.career tbody td { height: 36pt; text-align: center; font-size: 10pt; }
.career tbody td.duty { text-align: left; padding-left: 7pt; }
.career .side { border-top: none; border-bottom: none; }
.career thead .side { border-bottom: none; }

/* 경력 영역 한 덩어리 — 출력 창이 쪽마다 한 덩어리로 나눈다(print-host.ts). 왼쪽 큰 칸 글자와 로고는 덩어리 위에 겹친다 */
.career-zone { position: relative; z-index: 0; }
.career-zone + .career-zone { break-before: page; page-break-before: always; }
/* 경력 사항 — 왼쪽 큰 칸(머리글 + 경력 줄)의 가로·세로 가운데 */
.career-zone .side-label { position: absolute; top: 0; bottom: 0; left: 0; width: 11%; display: flex; align-items: center; justify-content: center; text-align: center; line-height: 1.6; }
/* 학교 로고 워터마크 — 머리글과 경력 사항 칸을 뺀 경력 줄 영역의 가운데, 표 글자 뒤. 종횡비 그대로 영역 안에만 */
.career-zone .logo-mark { position: absolute; top: 45pt; bottom: 0; left: 11%; right: 0; z-index: -1; display: flex; align-items: center; justify-content: center; }
.career-zone .logo-mark img { display: block; max-width: min(70%, 85mm); max-height: min(calc(100% - 16pt), 85mm); }
.period { font-variant-numeric: tabular-nums; white-space: nowrap; }

/* 마지막 덩어리 — 중간에서 쪽이 갈라지지 않는다 */
.closing { break-inside: avoid; page-break-inside: avoid; }
.closing .use td, .closing .use th { height: 32pt; }
.closing .use .val { text-align: left; padding-left: 8pt; }
.closing .use tr > * { border-top: 1.2pt solid #000; border-bottom: 1.2pt solid #000; }
.statement { text-align: center; font-size: 12pt; margin: 34pt 0 0; }
.issued { text-align: center; font-size: 12pt; margin: 28pt 0 0; letter-spacing: 0.05em; }
.issuer { text-align: center; font-size: 13.5pt; font-weight: 700; margin: 30pt 0 0; letter-spacing: 0.08em; }
.issuer .seal { font-weight: 400; font-size: 11pt; letter-spacing: 0; margin-left: 4pt; }
.contact { width: 44%; margin: 30pt 0 0; border-top: 1pt solid #000; border-right: 1pt solid #000; }
.contact col.k { width: 34%; } .contact col.v { width: 66%; }
.contact th, .contact td { font-size: 9.5pt; height: 19pt; padding: 2pt 6pt; text-align: center; }
.contact tr:last-child > * { border-bottom: none; }

/* 작성 중 워터마크 — 쪽마다 비스듬히 크게 (position: fixed 는 인쇄할 때 쪽마다 되풀이된다) */
.draft-mark { position: fixed; top: 0; left: 0; width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; pointer-events: none; z-index: 10; }
.draft-mark span { transform: rotate(-32deg); font-size: 66pt; font-weight: 700; color: rgba(200, 30, 30, 0.13); white-space: nowrap; letter-spacing: 0.06em; border: 5pt solid rgba(200, 30, 30, 0.13); padding: 6pt 28pt; border-radius: 10pt; }
"#;

pub fn render(doc: &CertificateDoc, mode: Mode) -> AppResult<String> {
    let mut h = String::with_capacity(16 * 1024);

    // ---- head: 글꼴·양식·쪽 여백 칸(쪽 번호) ----
    // <title> 은 PDF 의 제목 정보가 된다 — 개인정보를 넣지 않는다(문서 제목만).
    h.push_str("<head><meta charset=\"utf-8\"><title>");
    h.push_str(&esc(&doc.title));
    h.push_str("</title><style>");
    h.push_str(STYLE);
    h.push_str(&page_rule(doc, mode));
    h.push_str("</style></head><body>");

    if mode.is_draft() {
        h.push_str(&format!("<div class=\"draft-mark\" aria-hidden=\"true\"><span>{DRAFT_MARK}</span></div>"));
    }

    h.push_str("<div class=\"sheet\">");

    // ---- 발급번호 · 제목 ----
    h.push_str(&format!("<div class=\"no\">발급번호:<b>{}</b></div>", esc(&doc.issue_no)));
    h.push_str(&format!("<h1 class=\"title\">{}</h1>", esc(&doc.title)));

    // ---- 인적사항 ----
    h.push_str("<table class=\"person\">");
    h.push_str(COLS);
    h.push_str(&format!(
        "<tr><th class=\"label\" rowspan=\"2\">인적<br>사항</th><th class=\"label\">성명</th>\
         <td class=\"val\" colspan=\"2\">{}</td><th class=\"label tight\">주민번호</th><td class=\"val\">{}</td></tr>",
        esc(&doc.holder.name),
        esc(&rrn_text(doc)),
    ));
    h.push_str(&format!(
        "<tr><th class=\"label\">주소</th><td class=\"addr\" colspan=\"4\">{}</td></tr>",
        esc(&doc.holder.address)
    ));
    h.push_str("</table>");

    // ---- 경력사항 ----
    // 왼쪽 큰 칸: 머리글 쪽은 빈 칸(아래 테두리 없음), 경력 줄 전체를 묶은 칸에 '경력 사항'
    h.push_str("<div class=\"career-zone\" data-split=\"page\"><div class=\"side-label\">경력<br>사항</div>");
    if let Some(l) = &doc.logo {
        h.push_str(&format!(
            "<div class=\"logo-mark\"><img alt=\"\" src=\"{}\"></div>",
            logo::data_url(&logo::watermark_png(l)?)
        ));
    }
    h.push_str("<table class=\"career\">");
    h.push_str(COLS);
    h.push_str(
        "<thead><tr><th class=\"label side\" rowspan=\"2\"></th>\
         <th colspan=\"2\">기 간</th><th rowspan=\"2\">프로그램명</th><th rowspan=\"2\">직 위</th>\
         <th rowspan=\"2\">지 도 사 항</th></tr><tr><th>부 터</th><th>까 지</th></tr></thead><tbody>",
    );
    for item in &doc.items {
        h.push_str(&format!(
            "<tr class=\"item\"><td class=\"side\"></td><td class=\"period\">{}</td><td class=\"period\">{}</td>\
             <td>{}</td><td>{}</td><td class=\"duty\">{}</td></tr>",
            esc(&item.from_text),
            esc(&item.to_text),
            esc(&item.program_name),
            esc(&item.position),
            esc(&item.duty),
        ));
    }
    for _ in doc.items.len()..MIN_ROWS {
        h.push_str("<tr class=\"blank\"><td class=\"side\"></td><td></td><td></td><td></td><td></td><td></td></tr>");
    }
    h.push_str("</tbody></table></div>");

    // ---- 마지막 덩어리 ----
    h.push_str("<div class=\"closing\">");
    h.push_str("<table class=\"use\">");
    h.push_str(COLS);
    h.push_str(&format!(
        "<tr><th class=\"label\">용도</th><td class=\"val\" colspan=\"5\">{}</td></tr></table>",
        esc(&doc.purpose)
    ));
    h.push_str("<p class=\"statement\">위와 같이 증명합니다.</p>");
    h.push_str(&format!("<p class=\"issued\">{}</p>", esc(&date::display_official(doc.issued_on))));
    h.push_str(&format!(
        "<p class=\"issuer\">{}<span class=\"seal\">(직인)</span></p>",
        esc(&doc.school.issuer_title)
    ));
    h.push_str("<table class=\"contact\"><col class=\"k\"><col class=\"v\">");
    h.push_str(&format!("<tr><th>담당부서</th><td>{}</td></tr>", esc(&doc.school.department)));
    h.push_str(&format!("<tr><th>담당자</th><td>{} (인)</td></tr>", esc(&doc.school.manager_name)));
    h.push_str(&format!("<tr><th>전화번호</th><td>{}</td></tr>", esc(&doc.school.phone)));
    h.push_str("</table></div>");

    h.push_str("</div></body>");
    Ok(h)
}

const COLS: &str = "<colgroup><col class=\"c-label\"><col class=\"c-from\"><col class=\"c-to\">\
                    <col class=\"c-prog\"><col class=\"c-pos\"><col class=\"c-duty\"></colgroup>";

/// A4 · 여백 · 쪽 아래 식별 정보(발급번호 · 성명 · 쪽/전체). 미리보기면 쪽 위에도 알림.
fn page_rule(doc: &CertificateDoc, mode: Mode) -> String {
    let footer = format!(
        "content: \"발급번호 \" {} \" · \" {} \" · \" counter(page) \" / \" counter(pages); font-family: CertSans; font-size: 7.5pt; color: #555;",
        css_string(&doc.issue_no),
        css_string(&doc.holder.name),
    );
    let top = if mode.is_draft() {
        format!(
            "@top-center {{ content: {}; font-family: CertSans; font-weight: 700; font-size: 9pt; color: #b42318; }}",
            css_string(&format!("{DRAFT_MARK} — 발급 확정 전 문서로, 증명서로 쓸 수 없습니다"))
        )
    } else {
        String::new()
    };
    format!("@page {{ size: A4 portrait; margin: 16mm 14mm 15mm; {top} @bottom-right {{ {footer} }} }}")
}
