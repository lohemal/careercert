-- 001_init — Phase 0
--
-- 이 파일이 **이 프로그램의 자료**임을 표시하는 표 하나만 만든다.
-- 업무 표(학교 설정·강사·경력·발급 기록)는 각 Phase 에서 002, 003 … 으로 더한다.
--
-- 규칙
--   * 배포(v0.1.0)된 뒤에는 이미 나간 마이그레이션 파일을 고치지 않는다. 바꿀 것은 새 번호로.
--   * 날짜는 TEXT 'YYYY-MM-DD', 시각은 TEXT 'YYYY-MM-DDTHH:MM:SS'(현지 시각).
--   * 표는 STRICT — 열 형식이 다른 값이 조용히 들어가지 않게 한다.

CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- 백업·복원 검사가 이 값으로 "다른 프로그램의 SQLite 파일"을 가려낸다.
INSERT INTO app_meta (key, value) VALUES
    ('app_id', 'kr.school.careercert'),
    ('created_at', strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime'));
