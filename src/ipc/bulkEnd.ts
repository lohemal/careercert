import type { Career, EndReason } from './career'
import { invoke } from './invoke'

export interface Candidate extends Career {
  instructorName: string
  distinguisher: string
}

export interface BulkEndRequest {
  careerIds: number[]
  /** YYYY-MM-DD */
  endDate: string
  reason: EndReason
}

export interface PreviewItem {
  careerId: number
  instructorId: number | null
  instructorName: string
  distinguisher: string
  programName: string
  /** `2026.03.04 ~ 현재` */
  before: string
  /** `2026.03.04 ~ 2027.02.12` — 끝낼 수 없으면 null */
  after: string | null
  problem: string | null
}

export interface Preview {
  items: PreviewItem[]
  endDateLabel: string
  reasonLabel: string
  future: boolean
  ok: boolean
  problems: number
  /** 미리보기 당시 자료의 지문 — 적용할 때 그대로 돌려보낸다 */
  stateKey: string
}

export interface Applied {
  ended: number
  backupFile: string
}

export const bulkEndApi = {
  candidates: () => invoke<Candidate[]>('bulk_end_candidates'),
  preview: (request: BulkEndRequest) => invoke<Preview>('bulk_end_preview', { request }),
  apply: (request: BulkEndRequest, stateKey: string, confirmFuture: boolean) =>
    invoke<Applied>('bulk_end_apply', { request, stateKey, confirmFuture }),
}
