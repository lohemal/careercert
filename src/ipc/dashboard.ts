import { invoke } from './invoke'

export interface CheckItem {
  instructorId: number
  instructorName: string
  distinguisher: string
  detail: string
  /** 기간 겹침 항목만 — [정상 경력으로 확인] 에 보낼 두 경력과 지금 지문 */
  overlap: OverlapRef | null
}

export interface OverlapRef {
  careerAId: number
  careerBId: number
  fingerprint: string
}

export interface Check {
  /** DUPLICATE_NAME · ACTIVE_STARTS_LATER · ENDS_LATER · OVERLAP */
  kind: string
  title: string
  description: string
  items: CheckItem[]
}

export interface Overview {
  instructors: number
  activeCareers: number
  /** 걸린 것이 있는 항목만 */
  checks: Check[]
}

export const dashboardApi = {
  overview: () => invoke<Overview>('dashboard_overview'),
  /** 같은 강사의 두 경력 기간 겹침을 '정상 경력' 으로 확인 (그 한 쌍 · 지금 지문). 기간이 바뀌면 다시 나온다 */
  acknowledgeOverlap: (o: OverlapRef) =>
    invoke<void>('career_overlap_acknowledge', { careerAId: o.careerAId, careerBId: o.careerBId, fingerprint: o.fingerprint }),
}
