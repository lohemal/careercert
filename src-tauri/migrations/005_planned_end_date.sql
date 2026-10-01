-- 005_planned_end_date — Phase 4
--
-- 예정 종료일: 계약서 등에 적힌 "이 날 끝날 예정" 인 날. **관리용 정보**다.
--
--   * 선택값. 비어 있으면 NULL.
--   * 있으면 실제로 있는 'YYYY-MM-DD' 이고 시작일보다 빠를 수 없다.
--   * 증명서의 경력 기간 계산에는 **쓰지 않는다.** 실제 종료일은 언제나 end_date 다.
--   * 재직중(ACTIVE) 은 end_date 가 NULL 이라는 003 의 규칙은 그대로다 — 예정 종료일이 있어도
--     실제로 끝나기 전까지는 재직중이다. 프로그램이 이 날짜로 저절로 종료하지 않는다.
--
-- SQLite 의 ADD COLUMN 은 다른 열을 보는 CHECK 도 받는다(기존 행은 NULL 이라 통과).

ALTER TABLE careers ADD COLUMN planned_end_date TEXT
    CHECK (planned_end_date IS NULL
           OR (date(planned_end_date) IS planned_end_date AND planned_end_date >= start_date));
