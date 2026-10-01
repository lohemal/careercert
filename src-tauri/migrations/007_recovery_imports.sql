-- 007_recovery_imports — Phase 8
--
-- 1) key_store: 복구 비밀번호로 감싼 사본(recovery_blob)을 언제 넣었는지.
--    recovery_blob 은 006 에 이미 있다(JSON — 형식 버전·KDF 이름·인자·salt·nonce·암호문).
--    비밀번호 자체는 어디에도 저장하지 않는다.
-- 2) imports: 엑셀 가져오기 한 묶음의 요약. **원본 파일 이름·경로는 저장하지 않는다**
--    (파일 이름에 개인정보가 들어 있을 수 있다). 대신 파일 내용의 SHA-256 — 같은 파일을 두 번
--    가져오는지 알아볼 수만 있고, 내용을 되살릴 수는 없다. 원본 파일은 DB 에 넣지 않는다.
-- 3) instructors.import_id: 가져오기로 새로 만든 강사가 어느 묶음에서 왔는지.
--
-- SQLite 의 ADD COLUMN 은 기본값이 있어야 NOT NULL 을 붙일 수 있다. 옛 행은 기본값을 받는다.

ALTER TABLE key_store ADD COLUMN recovery_set_at TEXT;

ALTER TABLE imports ADD COLUMN label TEXT NOT NULL DEFAULT '' CHECK (length(label) <= 100);
-- XLSX · XLSM
ALTER TABLE imports ADD COLUMN source_kind TEXT NOT NULL DEFAULT '' CHECK (source_kind IN ('', 'XLSX', 'XLSM'));
ALTER TABLE imports ADD COLUMN source_sha256 TEXT NOT NULL DEFAULT '' CHECK (source_sha256 = '' OR length(source_sha256) = 64);
ALTER TABLE imports ADD COLUMN sheet_name TEXT NOT NULL DEFAULT '' CHECK (length(sheet_name) <= 100);
ALTER TABLE imports ADD COLUMN rows_total INTEGER NOT NULL DEFAULT 0 CHECK (rows_total >= 0);
ALTER TABLE imports ADD COLUMN instructors_created INTEGER NOT NULL DEFAULT 0 CHECK (instructors_created >= 0);
ALTER TABLE imports ADD COLUMN instructors_linked INTEGER NOT NULL DEFAULT 0 CHECK (instructors_linked >= 0);
ALTER TABLE imports ADD COLUMN careers_added INTEGER NOT NULL DEFAULT 0 CHECK (careers_added >= 0);
-- 경고가 있었지만 가져온 줄
ALTER TABLE imports ADD COLUMN rows_warned INTEGER NOT NULL DEFAULT 0 CHECK (rows_warned >= 0);
-- 오류·중복·검토 미선택·담당자 제외로 가져오지 않은 줄
ALTER TABLE imports ADD COLUMN rows_skipped INTEGER NOT NULL DEFAULT 0 CHECK (rows_skipped >= 0);
ALTER TABLE imports ADD COLUMN created_by TEXT NOT NULL DEFAULT 'local';

ALTER TABLE instructors ADD COLUMN import_id INTEGER REFERENCES imports (id);

CREATE INDEX idx_careers_import ON careers (import_id);
CREATE INDEX idx_instructors_import ON instructors (import_id);

-- 가져오기 기록은 고치거나 지우지 않는다(되짚을 수 있게)
CREATE TRIGGER imports_no_update BEFORE UPDATE ON imports
BEGIN
    SELECT RAISE(ABORT, 'imports is append-only');
END;

CREATE TRIGGER imports_no_delete BEFORE DELETE ON imports
BEGIN
    SELECT RAISE(ABORT, 'imports is append-only');
END;
