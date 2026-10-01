/**
 * 증명서 작성 중인 값 — **화면 메모리에만** 있다. 저장하지 않는다(DB·localStorage·sessionStorage 모두 아님).
 *
 * 규칙 (Phase 4 결정)
 *   * 강사를 바꾸면 주민번호·주소·발급번호를 **비운다**. 용도·발급일은 학교 기본값·오늘로, 뒷자리 가림은 꺼짐으로 되돌린다.
 *     앞 사람의 주민번호가 다음 사람 증명서로 넘어가는 일을 구조적으로 막는다(기존 엑셀의 E1 문제).
 *   * 경력은 기본으로 **전부 선택**이다. 사용자가 끈 것만 기억한다(`excluded`).
 *     발급일 때문에 넣을 수 없는 경력은 선택에서 빠지고, 발급일을 바꿔 넣을 수 있게 되면 다시 선택된다.
 *     넣을 수 있는지는 화면이 판단하지 않는다 — Rust 가 준 `eligible` 을 따른다.
 *
 *   * "이 내용으로 새 증명서 작성"(Phase 7): 원래 발급 기록에서 용도·주민번호 표시 방식·경력 선택만 가져온다.
 *     주민번호·주소·발급번호는 **가져오지 않는다**(빈칸). 발급일은 오늘. 원본 번호는 `copiedFrom` 으로만 들고 있다가
 *     발급 확정 때 함께 보낸다. 다른 강사로 바꾸면 원본과의 연결도 끊는다(다른 사람의 새 증명서다).
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
  /** 증명서에 주민번호 뒷자리를 가려 찍는가 (기본 꺼짐 = 전체) */
  maskRrn: boolean
}

export interface Draft {
  instructorId: number | null
  /** 사용자가 체크를 끈 경력 번호 */
  excluded: number[]
  fields: IssueFields
  /** "이 내용으로 새 증명서 작성" 의 원래 발급 기록 번호 */
  copiedFrom: number | null
}

/** Rust `CopyPlanView` 중 초안에 쓰는 것 — 주민번호·주소·발급번호는 들어 있지 않다 */
export interface CopyPlanLike {
  fromId: number
  instructorId: number | null
  issuedOn: string
  purpose: string
  maskRrn: boolean
  excludedCareerIds: number[]
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
  return { rrn: '', address: '', issueNo: '', purpose: d.purpose, issuedOn: d.today, maskRrn: false }
}

export function initialDraft(d: Defaults): Draft {
  return { instructorId: null, excluded: [], fields: emptyFields(d), copiedFrom: null }
}

/**
 * 강사를 고른다(바꾼다). 개인정보·발급번호는 비우고 선택은 기본(전부)으로.
 * 단, "새 증명서 작성" 초안에서 원래 강사를 쓸 수 없어 **처음** 강사를 고르는 경우에는
 * 가져온 용도·표시 방식·원본 연결을 남긴다(주민번호·주소·발급번호는 어차피 빈칸이다).
 */
export function selectInstructor(prev: Draft, instructorId: number | null, d: Defaults): Draft {
  if (prev.copiedFrom != null && prev.instructorId == null && instructorId != null) {
    const f = emptyFields(d)
    return {
      instructorId,
      excluded: [],
      copiedFrom: prev.copiedFrom,
      fields: { ...f, purpose: prev.fields.purpose, maskRrn: prev.fields.maskRrn },
    }
  }
  return { instructorId, excluded: [], fields: emptyFields(d), copiedFrom: null }
}

/** "이 내용으로 새 증명서 작성" 의 첫 초안. 주민번호·주소·발급번호는 빈칸, 발급일은 오늘. */
export function fromCopyPlan(plan: CopyPlanLike, d: Defaults): Draft {
  return {
    instructorId: plan.instructorId,
    excluded: plan.instructorId == null ? [] : [...plan.excludedCareerIds],
    copiedFrom: plan.fromId,
    fields: { ...emptyFields(d), issuedOn: plan.issuedOn || d.today, purpose: plan.purpose, maskRrn: plan.maskRrn },
  }
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
