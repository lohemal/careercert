// 작성 중 값 규칙 시험 (`npm run test:ui`). 가상값만 쓴다.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { blockers, fromCopyPlan, initialDraft, isSelected, selectedIds, selectInstructor, setField, toggle, type Draft } from './draft.ts'

const D = { purpose: '기관제출', today: '2026-10-01' }
const FAKE_RRN = '900101-1000001' // privacy:fake

function filled(): Draft {
  let d = selectInstructor(initialDraft(D), 1, D)
  d = setField(d, 'rrn', FAKE_RRN)
  d = setField(d, 'address', '가상시 연습로 1')
  d = setField(d, 'issueNo', '제2026-152호')
  d = setField(d, 'purpose', '취업용')
  d = setField(d, 'issuedOn', '2026-09-30')
  d = setField(d, 'maskRrn', true)
  return d
}

test('강사를 바꾸면 주민번호·주소·발급번호를 비우고 용도·발급일은 기본값으로', () => {
  const before = toggle(filled(), { id: 10, eligible: true })
  const after = selectInstructor(before, 2, D)
  assert.equal(after.instructorId, 2)
  assert.equal(after.fields.rrn, '')
  assert.equal(after.fields.address, '')
  assert.equal(after.fields.issueNo, '')
  assert.equal(after.fields.purpose, '기관제출')
  assert.equal(after.fields.issuedOn, '2026-10-01')
  assert.equal(after.fields.maskRrn, false, '뒷자리 가림도 기본값(꺼짐)으로')
  assert.deepEqual(after.excluded, [], '경력 선택도 기본(전부)으로')
})

test('같은 강사를 다시 골라도 비운다 (이전 입력을 남기지 않는다)', () => {
  const again = selectInstructor(filled(), 1, D)
  assert.equal(again.fields.rrn, '')
})

test('경력은 기본으로 전부 선택이고 끈 것만 빠진다', () => {
  const choices = [
    { id: 1, eligible: true },
    { id: 2, eligible: true },
    { id: 3, eligible: false },
  ]
  let d = selectInstructor(initialDraft(D), 1, D)
  assert.deepEqual(selectedIds(d, choices), [1, 2], '넣을 수 없는 경력은 처음부터 빠진다')
  d = toggle(d, choices[0])
  assert.deepEqual(selectedIds(d, choices), [2])
  d = toggle(d, choices[0])
  assert.deepEqual(selectedIds(d, choices), [1, 2])
})

test('넣을 수 없는 경력은 켤 수 없고, 발급일이 바뀌어 넣을 수 있게 되면 다시 선택된다', () => {
  let d = selectInstructor(initialDraft(D), 1, D)
  const notYet = { id: 3, eligible: false }
  d = toggle(d, notYet)
  assert.equal(isSelected(d, notYet), false)
  assert.deepEqual(d.excluded, [], '켜려는 시도는 무시된다')
  assert.equal(isSelected(d, { id: 3, eligible: true }), true)
})

test('선택 0건이나 빈 칸이 있으면 내용 확인을 할 수 없다', () => {
  assert.deepEqual(blockers(initialDraft(D), []), ['강사를 고르세요.'])
  const d = filled()
  assert.deepEqual(blockers(d, [{ id: 1, eligible: true }]), [])
  assert.deepEqual(blockers(d, [{ id: 1, eligible: false }]), ['증명서에 넣을 경력을 하나 이상 고르세요.'])
  const blank = setField(setField(d, 'issueNo', '   '), 'rrn', '')
  assert.deepEqual(blockers(blank, [{ id: 1, eligible: true }]), ['주민등록번호를 입력하세요.', '발급번호를 입력하세요.'])
})

test('setField 는 원래 값을 바꾸지 않는다', () => {
  const d = filled()
  const e = setField(d, 'issueNo', '다른 번호')
  assert.equal(d.fields.issueNo, '제2026-152호')
  assert.equal(e.fields.issueNo, '다른 번호')
})

// ---------------- 이 내용으로 새 증명서 작성 ----------------

const PLAN = {
  fromId: 7,
  instructorId: 1,
  issuedOn: '2026-10-01',
  purpose: '취업용',
  maskRrn: true,
  excludedCareerIds: [12],
}

test('새 증명서 초안은 발급번호·주민번호·주소가 빈칸이고 발급일은 오늘, 용도·표시 방식은 가져온다', () => {
  const d = fromCopyPlan(PLAN, D)
  assert.equal(d.fields.issueNo, '')
  assert.equal(d.fields.rrn, '')
  assert.equal(d.fields.address, '')
  assert.equal(d.fields.issuedOn, '2026-10-01')
  assert.equal(d.fields.purpose, '취업용')
  assert.equal(d.fields.maskRrn, true)
  assert.equal(d.copiedFrom, 7)
  assert.equal(d.instructorId, 1)
})

test('새 증명서 초안은 원래 없던 경력만 끄고 나머지는 선택한다', () => {
  const d = fromCopyPlan(PLAN, D)
  const choices = [
    { id: 11, eligible: true },
    { id: 12, eligible: true },
    { id: 13, eligible: false },
  ]
  assert.deepEqual(selectedIds(d, choices), [11])
})

test('새 증명서 초안에서 다른 강사로 바꾸면 원본 연결도 끊는다', () => {
  const d = selectInstructor(fromCopyPlan(PLAN, D), 2, D)
  assert.equal(d.copiedFrom, null)
  assert.equal(d.fields.purpose, '기관제출')
})

test('원래 강사를 쓸 수 없는 초안에서 처음 강사를 고르면 가져온 값과 원본 연결을 남긴다', () => {
  const d0 = fromCopyPlan({ ...PLAN, instructorId: null }, D)
  assert.equal(d0.instructorId, null)
  assert.deepEqual(d0.excluded, [])
  const d = selectInstructor(d0, 3, D)
  assert.equal(d.copiedFrom, 7)
  assert.equal(d.fields.purpose, '취업용')
  assert.equal(d.fields.maskRrn, true)
  assert.equal(d.fields.rrn, '')
})

test('일반 초안은 원본 연결이 없다', () => {
  assert.equal(initialDraft(D).copiedFrom, null)
  assert.equal(selectInstructor(initialDraft(D), 1, D).copiedFrom, null)
})
