import { invoke } from './invoke'

/** Rust `commands::instructor::InstructorView` — 주민번호·주소는 없다 */
export interface Instructor {
  id: number
  name: string
  distinguisher: string
  phone: string
  memo: string
  archived: boolean
  archivedAt: string | null
  createdAt: string
  updatedAt: string
}

export interface InstructorRow extends Instructor {
  careers: number
  activeCareers: number
  /** 보관하지 않은 경력의 프로그램명 (가나다순, 겹치지 않게) */
  programs: string[]
  /** 지금 목록에 같은 이름이 또 있다 */
  sameName: boolean
}

/** 화면 필터 [재직중 · 전체 · 보관] */
export type Scope = 'ACTIVE' | 'ALL' | 'ARCHIVED'

export interface InstructorInput {
  name: string
  distinguisher: string
  phone: string
  memo: string
}

export const instructorApi = {
  search: (query: string, scope: Scope) => invoke<InstructorRow[]>('instructor_search', { query, scope }),
  get: (id: number) => invoke<Instructor>('instructor_get', { id }),
  create: (input: InstructorInput) => invoke<Instructor>('instructor_create', { input }),
  update: (id: number, input: InstructorInput) => invoke<Instructor>('instructor_update', { id, input }),
  archive: (id: number) => invoke<Instructor>('instructor_archive', { id }),
  unarchive: (id: number) => invoke<Instructor>('instructor_unarchive', { id }),
}
