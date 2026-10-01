import type { QueryClient } from '@tanstack/react-query'

/** 강사·경력이 바뀌면 이것을 보는 화면을 모두 새로 읽는다 (목록·상세·대시보드·일괄 종료 후보). */
export function invalidateCareerData(qc: QueryClient) {
  for (const key of ['instructors', 'instructor', 'careers', 'dashboard', 'bulk-candidates']) {
    qc.invalidateQueries({ queryKey: [key] })
  }
}
