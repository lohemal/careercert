/**
 * 증명서 작성 중인 값 — **화면 메모리에만** 있다. 저장하지 않는다(DB·localStorage·sessionStorage 모두 아님).
 *
 * 규칙 (Phase 4 결정)
 *   * 강사를 바꾸면 주민번호·주소·발급번호를 **비운다**. 용도·발급일은 학교 기본값·오늘로 되돌린다.
 *     앞 사람의 주민번호가 다음 사람 증명서로 넘어가는 일을 구조적으로 막는다(기존 엑셀의 E1 문제).
 *   * 경력은 기본으로 **전부 선택**이다. 사용자가 끈 것만 기억한다(`excluded`).
 *     발급일 때문에 넣을 수 없는 경력은 선택에서 빠지고, 발급일을 바꿔 넣을 수 있게 되면 다시 선택된다.
 *     넣을 수 있는지는 화면이 판단하지 않는다 — Rust 가 준 `eligible` 을 따른다.
 *
 * 이 파일은 다른 모듈을 import 하지 않는다 — `node --test` 로 바로 시험한다.
 */

export interface IssueFields {
  rrn: string
  address: string
  issueNo: string
  purpose: string
  /** YYYY-MM-DD */
  issuedOn: string
}

export interface Draft {
  instructorId: number | null
  /** 사용자가 체크를 끈 경력 번호 */
  excluded: number[]
  fields: IssueFields
}

export interface Defaults {
  /** 학교 설정의 기본 용도 */
  purpose: string
  /** 오늘 YYYY-MM-DD */
  today: string
}

/** 고를 수 있는지는 Rust 가 정한다 */
export interface ChoiceLike {
  id: number
  eligible: boolean
}

export function emptyFields(d: Defaults): IssueFields {
  return { rrn: '', address: '', issueNo: '', purpose: d.purpose, issuedOn: d.today }
}

export function initialDraft(d: Defaults): Draft {
  return { instructorId: null, excluded: [], fields: emptyFields(d) }
}

/** 강사를 고른다(바꾼다). 개인정보·발급번호는 비우고 선택은 기본(전부)으로. */
export function selectInstructor(_prev: Draft, instructorId: number | null, d: Defaults): Draft {
  return { instructorId, excluded: [], fields: emptyFields(d) }
}

export function setField<K extends keyof IssueFields>(draft: Draft, key: K, value: IssueFields[K]): Draft {
  return { ...draft, fields: { ...draft.fields, [key]: value } }
}

/** 체크를 켜거나 끈다. 넣을 수 없는 경력은 바꾸지 않는다. */
export function toggle(draft: Draft, choice: ChoiceLike): Draft {
  if (!choice.eligible) return draft
  const off = draft.excluded.includes(choice.id)
  return {
    ...draft,
    excluded: off ? draft.excluded.filter((id) => id !== choice.id) : [...draft.excluded, choice.id],
  }
}

/** 지금 선택된 경력 = 넣을 수 있는 것 − 사용자가 끈 것 */
export function selectedIds(draft: Draft, choices: ChoiceLike[]): number[] {
  return choices.filter((c) => c.eligible && !draft.excluded.includes(c.id)).map((c) => c.id)
}

export function isSelected(draft: Draft, choice: ChoiceLike): boolean {
  return choice.eligible && !draft.excluded.includes(choice.id)
}

/** [증명서 내용 확인] 을 누를 수 없는 까닭. 비어 있으면 누를 수 있다. 형식 검사는 Rust 가 한다. */
export function blockers(draft: Draft, choices: ChoiceLike[]): string[] {
  const out: string[] = []
  if (draft.instructorId == null) return ['강사를 고르세요.']
  if (selectedIds(draft, choices).length === 0) out.push('증명서에 넣을 경력을 하나 이상 고르세요.')
  const f = draft.fields
  if (!f.rrn.trim()) out.push('주민등록번호를 입력하세요.')
  if (!f.address.trim()) out.push('주소를 입력하세요.')
  if (!f.issueNo.trim()) out.push('발급번호를 입력하세요.')
  if (!f.purpose.trim()) out.push('용도를 입력하세요.')
  if (!f.issuedOn) out.push('발급일을 고르세요.')
  return out
}
