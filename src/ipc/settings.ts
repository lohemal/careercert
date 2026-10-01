import { invoke } from './invoke'

/** Rust `domain::settings::SchoolSettings` 와 1:1 */
export interface SchoolSettings {
  schoolName: string
  certificateTitle: string
  issuerTitle: string
  department: string
  managerName: string
  phone: string
  defaultPurpose: string
  /** 한 번도 저장하지 않았으면 null */
  updatedAt: string | null
}

/** Rust `SettingsInput` — 저장할 때 보내는 값 */
export type SettingsInput = Omit<SchoolSettings, 'updatedAt'>

export type FieldKey = keyof SettingsInput

export interface SettingsView {
  settings: SchoolSettings
  /** 증명서에 찍히는데 비어 있는 칸. 비어 있으면 설정 완료 */
  missing: { key: FieldKey; label: string }[]
  /** 저장하면서 발급자 표기를 학교명으로 채웠는가 */
  issuerFilled: boolean
}

/** Rust `commands::settings::LogoView` — 미리보기는 보관본(색 그대로), 경로는 없다 */
export interface SchoolLogo {
  present: boolean
  preview: string | null
  width: number | null
  height: number | null
  updatedAt: string | null
}

export const logoApi = {
  get: () => invoke<SchoolLogo>('school_logo_get'),
  /** 파일을 고르면 바로 검사·보관한다. 취소하면 null */
  pick: () => invoke<SchoolLogo | null>('school_logo_pick'),
  remove: () => invoke<SchoolLogo>('school_logo_remove'),
}

export const settingsApi = {
  get: () => invoke<SettingsView>('settings_get'),
  save: (input: SettingsInput) => invoke<SettingsView>('settings_save', { input }),
}

/** 기본값 — Rust `DEFAULT_TITLE` · `DEFAULT_PURPOSE` 와 같다(마이그레이션 002 의 기본값) */
export const DEFAULT_TITLE = '방과후학교 개인위탁 외부강사 활동 확인서'
export const DEFAULT_PURPOSE = '기관제출'

/**
 * 학교명으로 만든 발급자 표기 제안 — Rust `domain::settings::suggest_issuer` 와 같은 규칙.
 * 화면은 **보여 주기만** 하고, 값을 바꾸는 것은 사용자가 버튼을 눌렀을 때뿐이다.
 */
export function suggestIssuer(schoolName: string): string | null {
  const name = schoolName.trim()
  return name ? `${name}장` : null
}
