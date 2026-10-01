//! 만든 PDF 를 가볍게 들여다본다 — 쪽 수, 쪽 크기, 실린 글꼴 이름.
//!
//! 개발 검증(`dev_print_spike`)과 시험이 쓴다. Chromium 이 만든 PDF 는 쪽·글꼴 사전이 압축되지 않은
//! 객체로 들어 있어 바이트를 훑는 것으로 충분하다. 본문 글자(압축된 내용 스트림)는 보지 않는다.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfFacts {
    pub bytes: usize,
    pub pages: usize,
    /// 쪽마다 [너비, 높이] (mm, 소수 첫째 자리)
    pub page_sizes_mm: Vec<[f64; 2]>,
    /// `/BaseFont` 이름 (서브셋 접두어 `ABCDEF+` 는 뗀다), 겹치지 않게
    pub fonts: Vec<String>,
}

fn find_all<'a>(hay: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    (0..hay.len().saturating_sub(needle.len()) + 1).filter(move |&i| &hay[i..i + needle.len()] == needle)
}

/// `/Type /Page`(뒤가 `s` 가 아닌 것) 를 센다.
pub fn facts(pdf: &[u8]) -> PdfFacts {
    let mut pages = 0;
    let mut sizes = Vec::new();
    for pat in [&b"/Type /Page"[..], &b"/Type/Page"[..]] {
        for i in find_all(pdf, pat) {
            let next = pdf.get(i + pat.len()).copied().unwrap_or(b' ');
            if next == b's' {
                continue;
            }
            pages += 1;
            // 같은 사전 안의 /MediaBox [0 0 w h]
            let end = (i + 600).min(pdf.len());
            let start = i.saturating_sub(600);
            let window = &pdf[start..end];
            if let Some(m) = find_all(window, b"/MediaBox").next() {
                let tail = &window[m..];
                if let (Some(a), Some(b)) = (tail.iter().position(|&c| c == b'['), tail.iter().position(|&c| c == b']')) {
                    let nums: Vec<f64> = String::from_utf8_lossy(&tail[a + 1..b])
                        .split_whitespace()
                        .filter_map(|x| x.parse().ok())
                        .collect();
                    if nums.len() == 4 {
                        let mm = |pt: f64| (pt / 72.0 * 25.4 * 10.0).round() / 10.0;
                        sizes.push([mm(nums[2] - nums[0]), mm(nums[3] - nums[1])]);
                    }
                }
            }
        }
    }
    let mut fonts: Vec<String> = find_all(pdf, b"/BaseFont /")
        .filter_map(|i| {
            let rest = &pdf[i + 11..(i + 120).min(pdf.len())];
            let end = rest.iter().position(|c| matches!(c, b' ' | b'/' | b'\n' | b'\r' | b'>' | b'['))?;
            let name = String::from_utf8_lossy(&rest[..end]).to_string();
            Some(match name.split_once('+') {
                Some((_, n)) => n.to_string(),
                None => name,
            })
        })
        .collect();
    fonts.sort();
    fonts.dedup();
    PdfFacts {
        bytes: pdf.len(),
        pages,
        page_sizes_mm: sizes,
        fonts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 쪽과_글꼴을_센다() {
        let pdf = b"%PDF-1.4\n1 0 obj << /Type /Pages /Count 2 >>\n2 0 obj << /Type /Page /MediaBox [0 0 595.92 841.92] >>\n\
                    3 0 obj << /Type /Page /MediaBox [0 0 595.92 841.92] >>\n4 0 obj << /BaseFont /AAAAAA+NotoSansKR-Regular /Subtype /CIDFontType2 >>\n\
                    5 0 obj << /BaseFont /BAAAAA+NotoSansKR-Bold >>\n6 0 obj << /BaseFont /CAAAAA+NotoSansKR-Regular >>";
        let f = facts(pdf);
        assert_eq!(f.pages, 2);
        assert_eq!(f.page_sizes_mm, vec![[210.2, 297.0], [210.2, 297.0]]);
        assert_eq!(f.fonts, vec!["NotoSansKR-Bold", "NotoSansKR-Regular"]);
    }
}
