import type { PrepareRequest } from './certificate'
import { invoke } from './invoke'

export interface OutputRecord {
  /** PRINT · PDF */
  outputType: string
  /** SUCCESS · FAILED */
  result: string
  printerName: string | null
  copies: number | null
  /** 실패했을 때 까닭 코드 (개인정보·경로 없음) */
  detail: string | null
  at: string
}

/** 경력 사본 한 줄 — 발급 당시 글자 그대로 */
export interface IssuedItem {
  seq: number
  fromText: string
  toText: string
  programName: string
  position: string
  duty: string
}

/** Rust `commands::issuance::IssuedView` — 발급 당시 스냅샷. 주민번호는 가린 값만, 주소는 없다(복호화하지 않고 만든다) */
export interface Issued {
  id: number
  issueNo: string
  issuedOnLabel: string
  holderName: string
  maskedRrn: string
  /** FULL · MASK_BACK */
  rrnDisplay: string
  purpose: string
  itemCount: number
  issuerTitle: string
  department: string
  managerName: string
  phone: string
  items: IssuedItem[]
  /** "이 내용으로 새 증명서 작성" 으로 만든 경우 그 원본 */
  copiedFrom: { id: number; issueNo: string } | null
  /** ISSUED · VOIDED */
  status: string
  voidedAt: string | null
  voidReason: string | null
  canOutput: boolean
  message: string | null
  createdAt: string
  outputs: OutputRecord[]
}

export interface IssueRequest {
  /** 내용 확인 때와 같은 요청 */
  prepare: PrepareRequest
  /** 내용 확인 때 받은 표 */
  reviewToken: string
  /** 확인한 경고의 ackKey */
  acknowledged: string[]
  copiedFrom: number | null
}

export interface PrinterInfo {
  name: string
  status: string
  ready: boolean
  jobs: number
}

export interface Printers {
  names: string[]
  default: string | null
  list: PrinterInfo[]
}

export interface SaveResult {
  saved: boolean
  fileName: string | null
  issued: Issued
}

export interface PrintResult {
  /** SUCCEEDED · PRINTER_UNAVAILABLE · OTHER_ERROR */
  status: string
  issued: Issued
}

/**
 * 발급 확정·정식 출력. 발급 확정 요청에는 주민번호·주소가 들어 있으므로 **직접 부른다**(useMutation 캐시에 남기지 않는다).
 * 정식 출력은 발급 번호(id)만 보낸다 — 출력할 내용은 Rust 가 발급 기록에서 다시 만든다.
 */
/** 한 번에 인쇄할 수 있는 매수 (서버도 같은 범위만 받는다) */
export const MAX_COPIES = 5

export interface DiscardRequest {
  reason: string
  /** 담당자가 다시 입력한 발급번호 (앞뒤 공백만 정리해 비교) */
  issueNoConfirm: string
  /** 정식 출력 성공 기록이 있을 때 '외부 교부가 아님' 을 따로 확인했는가 */
  outputsAcknowledged: boolean
}

export const issuanceApi = {
  issue: (request: IssueRequest) => invoke<Issued>('certificate_issue', { request }),
  get: (id: number) => invoke<Issued>('certificate_issued', { id }),
  void: (id: number, reason: string) => invoke<Issued>('certificate_void', { id, reason }),
  /** 오발급 폐기 — 기록을 지우고 발급번호를 다시 쓸 수 있게 한다(발급 취소와 다르다). 사유는 저장하지 않는다 */
  discard: (id: number, request: DiscardRequest) => invoke<void>('certificate_discard', { id, request }),
  savePdf: (id: number) => invoke<SaveResult>('certificate_save_pdf', { id }),
  print: (id: number, printer: string, copies: number) => invoke<PrintResult>('certificate_print', { id, printer, copies }),
  printers: () => invoke<Printers>('printers_list'),
}

// ---------------------------------------------------------------
// 발급이력
// ---------------------------------------------------------------

export type HistoryStatus = 'ALL' | 'ISSUED' | 'VOIDED'

export interface HistoryFilter {
  query: string
  from: string | null
  to: string | null
  status: HistoryStatus
}

/** 목록 한 줄 — 주민번호·주소 없음 */
export interface HistoryRow {
  id: number
  issueNo: string
  issuedOn: string
  issuedOnLabel: string
  holderName: string
  purpose: string
  itemCount: number
  rrnDisplay: string
  status: string
  voidedAt: string | null
  createdAt: string
}

export interface HistoryPage {
  rows: HistoryRow[]
  truncated: boolean
  limit: number
}

/** "이 내용으로 새 증명서 작성" 의 출발점 — 주민번호·주소·발급번호는 없다 */
export interface CopyPlan {
  fromId: number
  fromIssueNo: string
  fromStatus: string
  instructorId: number | null
  issuedOn: string
  purpose: string
  maskRrn: boolean
  selectedCareerIds: number[]
  excludedCareerIds: number[]
  originalCount: number
  unlinkedCount: number
  notices: string[]
}

export interface Recent {
  issuedCount: number
  voidedCount: number
  issued: HistoryRow[]
  voided: HistoryRow[]
}

export interface Recovery {
  /** NO_DATA · NOT_SET · READY */
  state: string
  ready: boolean
  message: string
  certificates: number
}

/** 발급이력 — 어느 것도 주민번호·주소를 돌려주지 않는다. 목록 조건에도 없다. */
export const historyApi = {
  list: (filter: HistoryFilter) => invoke<HistoryPage>('certificate_history', { filter }),
  copyPlan: (id: number) => invoke<CopyPlan>('certificate_copy_plan', { id }),
  recent: () => invoke<Recent>('certificate_recent'),
  recovery: () => invoke<Recovery>('recovery_status'),
}
