// seed 안전장치 시험. 실제 파일은 건드리지 않는다.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertSandboxPath, plan, sandboxDbPath } from './seed-sandbox.mjs'

test('연습용 자료 파일 경로만 받는다', () => {
  const p = sandboxDbPath({ APPDATA: 'C:\\Users\\가상\\AppData\\Roaming' })
  assert.equal(p.replace(/\\/g, '/'), 'C:/Users/가상/AppData/Roaming/kr.school.careercert.sandbox/careercert.db')
  assert.doesNotThrow(() => assertSandboxPath(p))
})

test('실제 자료 파일과 그 밖의 경로는 거절한다', () => {
  for (const p of [
    'C:\\Users\\가상\\AppData\\Roaming\\kr.school.careercert\\careercert.db',
    'C:\\Users\\가상\\AppData\\Roaming\\kr.school.careercert.sandbox\\backups\\auto_20261001_090000.db',
    'C:\\Users\\가상\\AppData\\Roaming\\kr.school.careercert.sandbox\\other.db',
    'C:\\kr.school.careercert.sandbox.evil\\careercert.db',
    'careercert.db',
  ]) {
    assert.throws(() => assertSandboxPath(p), /연습용 자료 파일이 아니어서/, p)
  }
})

test('APPDATA 가 없으면 멈춘다', () => {
  assert.throws(() => sandboxDbPath({}), /APPDATA/)
})

test('가상 자료는 요구한 상황을 모두 담는다', () => {
  const y = 2026
  const people = plan(y)
  const careers = people.flatMap((p) => p.careers.map((c) => ({ p, c })))
  const active = ({ c }) => c[2] === null
  assert.ok(people.some((p) => p.careers.length >= 4), '여러 해 경력')
  assert.ok(careers.some(active), '지금 재직중')
  assert.ok(people.some((p) => p.careers.every((c) => c[2] !== null)), '모두 끝난 강사')
  assert.ok(careers.some(({ c }) => c[3] === 'TERMINATED'), '중도해지')
  const names = people.map((p) => p.name)
  assert.ok(names.some((n, i) => names.indexOf(n) !== i), '동명이인')
  assert.ok(people.some((p) => p.archived), '보관한 강사')
  assert.ok(careers.some(({ c }) => c[4] === true), '보관한 경력')
  // DB 규칙과 어긋나는 값이 없다: 재직중은 사유 없음, 종료는 사유 있음, 종료 ≥ 시작
  for (const { c } of careers) {
    const [, start, end, reason] = c
    if (end === null) assert.equal(reason, null)
    else {
      assert.ok(reason === 'CONTRACT_END' || reason === 'TERMINATED')
      assert.ok(end >= start, `${start} ~ ${end}`)
    }
    assert.match(start, /^\d{4}-\d{2}-\d{2}$/)
  }
})
