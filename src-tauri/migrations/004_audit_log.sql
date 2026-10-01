-- 004_audit_log — Phase 3
--
-- 중요한 변경을 **누가 언제 무엇을** 했는지 남긴다. 무엇이 **어떻게** 바뀌었는지(값)는 남기지 않는다.
--
--   * 주민등록번호·주소·이름·연락처 같은 값을 적는 칸이 없다. summary 에는 바뀐 **항목 이름**,
--     건수, 종료일·종료 사유 같은 업무 추적에 필요한 최소 정보만 적는다.
--     (로그가 강사 자료의 복제본이 되면, 자료를 지워도 로그에 남는다.)
--   * 대상은 번호(target_id)로만 가리킨다. 무엇을 가리키는지는 지금 자료에서 찾아본다.
--   * 덧붙이기만 한다. 고치거나 지우는 코드는 없다.

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY,
    at          TEXT NOT NULL,
    -- INSTRUCTOR_CREATE · CAREER_END · BULK_END …  (domain::audit::Action)
    action      TEXT NOT NULL CHECK (action <> ''),
    -- instructor · career · bulk_end
    target_type TEXT NOT NULL CHECK (target_type <> ''),
    target_id   INTEGER,
    summary     TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 300)
) STRICT;

CREATE INDEX idx_audit_target ON audit_log (target_type, target_id);
