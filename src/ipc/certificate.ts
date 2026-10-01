import type { Career } from './career'
import { invoke } from './invoke'

/** 작성 화면 2단계의 한 줄 — Rust `commands::certificate::ChoiceView` */
export interface Choice extends Career {
  /** 이 발급일의 증명서에 넣을 수 있는가 (Rust domain 이 판단) */
  eligible: boolean
  /** 넣을 수 없는 까닭 */
  reason: string | null
  /** 발급일 기준 '부터' · '까지' — Rust 가 만든 글자 */
  onIssueFrom: string | null
  onIssueTo: string | null
  /** 예정 종료일 `2027.02.05` (관리 참고용) */
  plannedEndLabel: string | null
}

/** 발급 정보. 주민번호·주소가 들어 있다 — 화면 메모리에만 두고 저장하지 않는다 */
export interface IssueFields {
  rrn: string
  address: string
  issueNo: string
  purpose: string
  /** YYYY-MM-DD */
  issuedOn: string
  /** 주민번호 뒷자리 가림 */
  maskRrn: boolean
}

export interface PrepareRequest {
  instructorId: number
  careerIds: number[]
  issue: IssueFields
}

export interface DocItem {
  careerId: number
  from: string
  to: string
  programName: string
  position: string
  duty: string
}

/** Rust `CertificateDoc` 을 화면용으로 펼친 것 */
export interface Doc {
  templateVersion: number
  title: string
  issueNo: string
  issuedOnLabel: string
  purpose: string
  holderName: string
  holderRrn: string
  /** 증명서에 찍힐 모양 (Rust render 가 만든다) */
  holderRrnShown: string
  rrnMasked: boolean
  holderAddress: string
  items: DocItem[]
  issuerTitle: string
  department: string
  managerName: string
  phone: string
}

export interface DocWarning {
  /** ENDS_AFTER_ISSUE · ISSUE_FAR_PAST · RRN_BIRTH_DATE */
  code: string
  message: string
  careerId: number | null
  /** 발급 확정 때 '확인함' 으로 돌려보낼 이름 */
  ackKey: string
}

export interface Prepared {
  doc: Doc
  warnings: DocWarning[]
  /** 내용 확인 표 (개인정보 없음) — 발급 확정 요청에 그대로 */
  reviewToken: string
}

export const certificateApi = {
  choices: (instructorId: number, issuedOn: string) =>
    invoke<Choice[]>('certificate_choices', { instructorId, issuedOn }),
  /**
   * 증명서 내용 만들기. **React Query 의 useMutation 으로 부르지 않는다** — mutation 캐시가
   * 요청 값(주민번호·주소)을 화면을 떠난 뒤에도 몇 분 들고 있기 때문이다. 화면 state 로만 다룬다.
   */
  prepare: (request: PrepareRequest) => invoke<Prepared>('certificate_prepare', { request }),
  /** 실제 출력 미리보기 — '발급 전 미리보기' 워터마크가 들어간 PDF (ArrayBuffer). 이것도 직접 부른다 */
  preview: (request: PrepareRequest) => invoke<ArrayBuffer>('certificate_preview', { request }),
}
