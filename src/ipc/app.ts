import { invoke } from './invoke'

/** 프로그램을 켤 때 있었던 일. 개인정보는 담기지 않는다 */
export interface StartupNote {
  /** INTEGRITY · BACKUP_FAILED */
  kind: string
  tone: 'success' | 'warn' | 'error' | 'info'
  message: string
}

/** Rust `commands::app::AppInfo` 와 1:1 */
export interface AppInfo {
  appVersion: string
  schemaVersion: number
  latestSchemaVersion: number
  dbPath: string
  backupDir: string
  /** 연습용 실행 — 실제 자료와 따로 저장된다 */
  sandbox: boolean
  notes: StartupNote[]
}

export const appApi = {
  info: () => invoke<AppInfo>('app_info'),
}
