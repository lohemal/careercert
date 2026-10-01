import { invoke } from './invoke'

/**
 * 설정 → 데이터 관리: 복구 비밀번호 · 이동용 백업 · 복원 · 엑셀 가져오기.
 *
 * 비밀번호를 보내는 요청은 **직접 부른다**(useMutation 캐시에 남기지 않는다). 파일 위치는 Rust 가 고르고
 * 경로는 돌려주지 않는다(파일 이름만).
 */

// ---------------- 복구 비밀번호 ----------------

/** 화면도 같은 규칙으로 먼저 알려 준다 (서버가 다시 검사한다) */
export const RECOVERY_MIN = 10
export const RECOVERY_MAX = 1024

export const recoveryApi = {
  set: (password: string, confirm: string) => invoke<void>('recovery_set', { password, confirm }),
  change: (old: string, password: string, confirm: string) => invoke<void>('recovery_change', { old, password, confirm }),
}

// ---------------- 이동용 백업 · 복원 ----------------

export interface ExportResult {
  saved: boolean
  fileName: string | null
}

export interface BackupHeader {
  fileName: string
  createdAtLabel: string
  schemaVersion: number
  appVersion: string
  formatVersion: number
}

export interface Counts {
  instructors: number
  careers: number
  certificatesIssued: number
  certificatesVoided: number
  imports: number
}

export interface RestorePreview {
  header: BackupHeader
  backup: Counts
  current: Counts
  migratedFrom: number
  latestSchema: number
  verifiedCertificates: number
  sameWindowsUser: boolean
}

export const backupApi = {
  exportPortable: (password: string) => invoke<ExportResult>('backup_export', { password }),
  pick: () => invoke<BackupHeader | null>('restore_pick'),
  unlock: (password: string) => invoke<RestorePreview>('restore_unlock', { password }),
  confirm: () => invoke<void>('restore_confirm'),
  cancel: () => invoke<void>('restore_cancel'),
}

// ---------------- 엑셀 가져오기 ----------------

export type ImportField = 'NAME' | 'PROGRAM' | 'POSITION' | 'START' | 'END' | 'DUTY' | 'PHONE'
export type RowStatus = 'OK' | 'WARN' | 'REVIEW' | 'ERROR' | 'DUPLICATE'
export type GroupAction = 'LINK' | 'NEW' | 'SKIP'
export type Outcome = 'ADD' | 'SKIP_ERROR' | 'SKIP_DUPLICATE_FILE' | 'SKIP_DUPLICATE_DB' | 'SKIP_REVIEW' | 'SKIP_EXCLUDED' | 'SKIP_GROUP' | 'PENDING_DECISION'

export interface ImportRow {
  rowNo: number
  name: string
  program: string
  position: string
  duty: string
  startText: string
  endText: string
  startLabel: string
  endLabel: string
  status: RowStatus
  messages: string[]
}

export interface ImportCandidate {
  id: number
  distinguisher: string
  phone: string
  careers: number
  programs: string[]
}

export interface ImportGroup {
  name: string
  rowNos: number[]
  phones: string[]
  candidates: ImportCandidate[]
}

export interface ImportAnalysis {
  token: string
  fileName: string
  kind: 'XLSX' | 'XLSM'
  sheet: string
  sheets: string[]
  headerRow: number
  columns: { field: ImportField; label: string; header: string; column: string }[]
  rows: ImportRow[]
  groups: ImportGroup[]
  formats: [string, number][]
}

export interface GroupDecision {
  name: string
  action: GroupAction | null
  instructorId: number | null
  distinguisher: string
}

export interface ImportDecisions {
  groups: GroupDecision[]
  reviewActive: number[]
  excludedRows: number[]
}

export interface ImportSummary {
  rowsTotal: number
  instructorsCreated: number
  instructorsLinked: number
  careersAdded: number
  rowsWarned: number
  rowsSkipped: number
}

export interface ImportPlan {
  summary: ImportSummary
  problems: string[]
  outcomes: [number, Outcome][]
  fingerprint: string
}

export interface ImportApplied {
  importId: number
  summary: ImportSummary
}

export const importApi = {
  /** sheet 없이 부르면 파일을 고른다 */
  open: (sheet: string | null = null) => invoke<ImportAnalysis | null>('import_open', { sheet }),
  plan: (token: string, decisions: ImportDecisions) => invoke<ImportPlan>('import_plan', { token, decisions }),
  apply: (token: string, decisions: ImportDecisions, fingerprint: string) =>
    invoke<ImportApplied>('import_apply', { token, decisions, fingerprint }),
  cancel: () => invoke<void>('import_cancel'),
}
