import { invoke } from './invoke'

export interface CheckItem {
  instructorId: number
  instructorName: string
  distinguisher: string
  detail: string
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
}
