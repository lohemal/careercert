import type { QueryClient } from '@tanstack/react-query'

/** 강사·경력이 바뀌면 이것을 보는 화면을 모두 새로 읽는다 (목록·상세·대시보드·일괄 종료 후보·증명서 작성의 경력 선택). */
export function invalidateCareerData(qc: QueryClient) {
  for (const key of ['instructors', 'instructor', 'careers', 'dashboard', 'bulk-candidates', 'cert-choices']) {
    qc.invalidateQueries({ queryKey: [key] })
  }
}
