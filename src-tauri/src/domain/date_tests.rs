use super::*;

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

// ---------------- 표시 ----------------

#[test]
fn 재직중_기간은_현재로_표시한다() {
    assert_eq!(display_period(ymd(2026, 3, 4), None), "2026.03.04 ~ 현재");
}

#[test]
fn 종료된_기간은_종료일로_표시한다() {
    assert_eq!(
        display_period(ymd(2025, 3, 5), Some(ymd(2026, 2, 6))),
        "2025.03.05 ~ 2026.02.06"
    );
}

#[test]
fn 기간의_두_쪽은_한_줄_표시와_같다() {
    let (from, to) = display_period_parts(ymd(2026, 3, 4), None);
    assert_eq!((from.as_str(), to.as_str()), ("2026.03.04", "현재"));
    assert_eq!(format!("{from} ~ {to}"), display_period(ymd(2026, 3, 4), None));
}

#[test]
fn 날짜는_0을_채워_점으로_표시한다() {
    assert_eq!(display(ymd(2022, 3, 4)), "2022.03.04");
    assert_eq!(display_end(Some(ymd(2023, 12, 31))), "2023.12.31");
    assert_eq!(display_end(None), CURRENT_LABEL);
}

// ---------------- 저장 형식 ----------------

#[test]
fn 저장_형식은_yyyy_mm_dd() {
    assert_eq!(to_iso(ymd(2022, 3, 4)), "2022-03-04");
    assert_eq!(parse_iso("2022-03-04"), Ok(ymd(2022, 3, 4)));
    assert_eq!(parse_iso(" 2022-03-04 "), Ok(ymd(2022, 3, 4)));
}

#[test]
fn 저장_형식은_엄격하다() {
    for s in ["2022-3-4", "2022.03.04", "20220304", "2022-03-04T00:00:00", "현재"] {
        assert_eq!(parse_iso(s), Err(DateError::Format), "{s}");
    }
    assert_eq!(parse_iso(""), Err(DateError::Empty));
    assert_eq!(parse_iso("2022-02-30"), Err(DateError::NotExist));
}

// ---------------- 기존 엑셀 글자 ----------------

#[test]
fn 기존_엑셀_날짜_글자를_읽는다() {
    let want = ymd(2022, 3, 4);
    for s in [
        "2022.3.4.",
        "2022. 3. 4",
        "2022. 3. 4.",
        "2022-03-04",
        "2022-3-4",
        "2022/3/4",
        "2022/03/04",
        "2022.03.04",
        "  2022.3.4.  ",
        "\t2022-03-04\n",
        "2022년 3월 4일",
        "2022년3월4일",
        "20220304",
        "22.3.4",
        "22.03.04.",
    ] {
        assert_eq!(parse_loose(s), Ok(want), "{s:?}");
    }
}

#[test]
fn 샘플_명단의_날짜가_모두_읽힌다() {
    // 기존 강사명단 시트에 실제로 있던 모양
    for (s, want) in [
        ("2022.3.4.", ymd(2022, 3, 4)),
        ("2023.2.10.", ymd(2023, 2, 10)),
        ("2023.3.5.", ymd(2023, 3, 5)),
        ("2024.2.8.", ymd(2024, 2, 8)),
        ("2024.3.8.", ymd(2024, 3, 8)),
        ("2025.2.7.", ymd(2025, 2, 7)),
    ] {
        assert_eq!(parse_loose(s), Ok(want), "{s}");
    }
}

#[test]
fn 두_자리_연도는_2000년대다() {
    assert_eq!(parse_loose("99.3.4"), Ok(ymd(2099, 3, 4)));
    assert_eq!(parse_loose("05.3.4"), Ok(ymd(2005, 3, 4)));
}

#[test]
fn 없는_날짜는_거부한다() {
    for s in ["2022.02.30", "2022.13.04", "2022.0.4", "2022.3.0", "2023.2.29", "2022-04-31", "20221304"] {
        assert_eq!(parse_loose(s), Err(DateError::NotExist), "{s}");
    }
    assert_eq!(parse_loose("2024.2.29"), Ok(ymd(2024, 2, 29)), "윤년은 받는다");
}

#[test]
fn 날짜_모양이_아니면_거부한다() {
    for s in [
        "현재",
        "2022.3",
        "2022.3.4.5",
        "2022.3-4",
        "2022.3.4..",
        "3월초",
        "2022.3.4 부터",
        "2022.003.04",
        "202.3.4",
        "1234567",
    ] {
        assert_eq!(parse_loose(s), Err(DateError::Format), "{s:?}");
    }
    assert_eq!(parse_loose("   "), Err(DateError::Empty));
}

#[test]
fn 연도_범위_밖은_오타로_본다() {
    assert_eq!(parse_loose("0222.3.4"), Err(DateError::YearOutOfRange));
    assert_eq!(parse_loose("2202.3.4"), Err(DateError::YearOutOfRange));
}

#[test]
fn 현재라는_말은_날짜가_아니라_재직중_표시다() {
    for s in ["현재", " 현재 ", "현 재", "재직중", "재직", "근무중"] {
        assert!(is_current_word(s), "{s}");
        assert!(parse_loose(s).is_err(), "{s} 는 날짜로 읽히면 안 된다");
    }
    for s in ["", "2022.3.4.", "현재까지", "퇴직"] {
        assert!(!is_current_word(s), "{s}");
    }
}

#[test]
fn 공문서_날짜는_0을_채우지_않고_온점으로_쓴다() {
    assert_eq!(display_official(ymd(2026, 10, 1)), "2026. 10. 1.");
    assert_eq!(display_official(ymd(2027, 2, 12)), "2027. 2. 12.");
}

#[test]
fn 시각에서_오늘을_뽑는다() {
    assert_eq!(today_of("2026-10-01T09:00:00"), Ok(ymd(2026, 10, 1)));
    assert!(today_of("").is_err());
}

// ---------------- 엑셀 날짜 숫자 ----------------

#[test]
fn 엑셀_1900_체계_날짜_숫자를_읽는다() {
    let d = |n: f64| from_excel_serial(n, false);
    assert_eq!(d(45000.0).unwrap(), NaiveDate::from_ymd_opt(2023, 3, 15).unwrap());
    assert_eq!(d(44624.0).unwrap(), NaiveDate::from_ymd_opt(2022, 3, 4).unwrap());
    assert_eq!(d(44624.75).unwrap(), NaiveDate::from_ymd_opt(2022, 3, 4).unwrap(), "시각은 버린다");
    assert_eq!(d(46085.0).unwrap(), NaiveDate::from_ymd_opt(2026, 3, 4).unwrap());
}

#[test]
fn 엑셀의_1900년_윤년_오류를_고려한다() {
    // 1 = 1900-01-01, 59 = 1900-02-28, 60 = 엑셀만의 1900-02-29(없는 날), 61 = 1900-03-01
    let day = |y, m, d| NaiveDate::from_ymd_opt(y, m, d);
    assert_eq!(excel_serial_day(1, false), day(1900, 1, 1));
    assert_eq!(excel_serial_day(59, false), day(1900, 2, 28));
    assert_eq!(excel_serial_day(60, false), None);
    assert_eq!(excel_serial_day(61, false), day(1900, 3, 1));
    assert_eq!(excel_serial_day(0, true), day(1904, 1, 1));
    // 1900 년대는 연도 범위 밖이라 결과로는 거부한다
    assert_eq!(from_excel_serial(60.0, false).unwrap_err(), DateError::NotExist);
    assert_eq!(from_excel_serial(59.0, false).unwrap_err(), DateError::YearOutOfRange);
    assert_eq!(from_excel_serial(61.0, false).unwrap_err(), DateError::YearOutOfRange);
    // 범위 안에서는 60 이후 규칙(1899-12-30 기준)이 실제 엑셀과 같다: 18264 = 1950-01-01
    assert_eq!(from_excel_serial(18264.0, false).unwrap(), NaiveDate::from_ymd_opt(1950, 1, 1).unwrap());
}

#[test]
fn 엑셀_1904_체계도_읽는다() {
    // 1904 체계는 1900 체계보다 1462 작다
    assert_eq!(from_excel_serial(45000.0 - 1462.0, true).unwrap(), NaiveDate::from_ymd_opt(2023, 3, 15).unwrap());
}

#[test]
fn 엑셀_날짜_숫자의_범위_밖은_거부한다() {
    assert!(from_excel_serial(0.0, false).is_err());
    assert!(from_excel_serial(-5.0, false).is_err());
    assert!(from_excel_serial(f64::NAN, false).is_err());
    assert!(from_excel_serial(3_000_000.0, false).is_err());
    assert!(from_excel_serial(73051.0, false).is_err(), "2100-01-01 은 범위 밖");
}
