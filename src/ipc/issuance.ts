import type { PrepareRequest } from './certificate'
import { invoke } from './invoke'

export interface OutputRecord {
  /** PRINT · PDF */
  outputType: string
  /** SUCCESS · FAILED */
  result: string
  printerName: string | null
  copies: number | null
  at: string
}

/** Rust `commands::issuance::IssuedView` — 주민번호는 가린 값만 */
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
export const issuanceApi = {
  issue: (request: IssueRequest) => invoke<Issued>('certificate_issue', { request }),
  get: (id: number) => invoke<Issued>('certificate_issued', { id }),
  void: (id: number, reason: string) => invoke<Issued>('certificate_void', { id, reason }),
  savePdf: (id: number) => invoke<SaveResult>('certificate_save_pdf', { id }),
  print: (id: number, printer: string, copies: number) => invoke<PrintResult>('certificate_print', { id, printer, copies }),
  printers: () => invoke<Printers>('printers_list'),
}
