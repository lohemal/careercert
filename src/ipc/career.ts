import { invoke } from './invoke'

export type CareerStatus = 'ACTIVE' | 'ENDED'
export type EndReason = 'CONTRACT_END' | 'TERMINATED'

/** 종료 사유 — 고르는 칸에 쓴다. 이름은 Rust `EndReason::label` 과 같다 */
export const END_REASONS: { value: EndReason; label: string }[] = [
  { value: 'CONTRACT_END', label: '계약만료' },
  { value: 'TERMINATED', label: '중도해지' },
]

/**
 * Rust `commands::career::CareerView`.
 * 표시 글자(`period`, `statusLabel`, `endReasonLabel`)는 Rust domain 이 만든다 — 화면은 날짜 표시를 만들지 않는다.
 * `startDate`·`endDate`(YYYY-MM-DD)는 입력 칸을 채울 때만 쓴다.
 */
export interface Career {
  id: number
  instructorId: number
  programName: string
  position: string
  duty: string
  startDate: string
  endDate: string | null
  status: CareerStatus
  statusLabel: string
  endReason: EndReason | null
  endReasonLabel: string | null
  /** `2026.03.04 ~ 현재` */
  period: string
  /** 기간의 두 쪽 — `period` 와 같은 글자를 나눈 것 (Rust 가 만든다) */
  periodFrom: string
  periodTo: string
  futureEnd: boolean
  /** 예정 종료일 YYYY-MM-DD — 관리용, 증명서 기간과 무관 */
  plannedEndDate: string | null
  /** `2027.02.05` */
  plannedEndLabel: string | null
  /** 재직중인데 예정 종료일이 지났다 */
  plannedEndPassed: boolean
  memo: string
  archived: boolean
  archivedAt: string | null
  updatedAt: string
}

export interface CareerInput {
  programName: string
  position: string
  duty: string
  startDate: string
  status: CareerStatus
  endDate: string | null
  endReason: EndReason | null
  plannedEndDate: string | null
  memo: string
}

export interface CareerHints {
  defaultPosition: string
  /** `방과후학교 {프로그램명}` */
  dutySuggestion: string | null
  /** 오늘 YYYY-MM-DD */
  today: string
}

export const careerApi = {
  list: (instructorId: number, includeArchived: boolean) =>
    invoke<Career[]>('career_list', { instructorId, includeArchived }),
  create: (instructorId: number, input: CareerInput, confirmFuture: boolean) =>
    invoke<Career>('career_create', { instructorId, input, confirmFuture }),
  update: (id: number, input: CareerInput, confirmFuture: boolean) =>
    invoke<Career>('career_update', { id, input, confirmFuture }),
  end: (id: number, endDate: string, reason: EndReason, confirmFuture: boolean) =>
    invoke<Career>('career_end', { id, endDate, reason, confirmFuture }),
  archive: (id: number) => invoke<Career>('career_archive', { id }),
  unarchive: (id: number) => invoke<Career>('career_unarchive', { id }),
  hints: (programName: string) => invoke<CareerHints>('career_hints', { programName }),
}
