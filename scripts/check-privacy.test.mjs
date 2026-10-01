// check-privacy 규칙 시험. `npm run test:privacy`
//
// 주민번호 모양 값은 이 파일에도 그대로 적지 않는다(적으면 이 파일이 커밋 검사에 걸린다).
// 조각을 이어 붙여 만들고, 표시를 붙인 줄만 그대로 적는다.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { checkPath, scanText, FAKE_MARK } from './check-privacy.mjs'

const front = '850315'
const back = '1' + '234567'

test('하이픈 있는 주민번호 모양을 잡는다', () => {
  const hits = scanText(`성명: 가상\n번호: ${front}-${back}\n`)
  assert.deepEqual(hits, [{ line: 2, kind: '주민등록번호 모양' }])
})

test('하이픈 없는 13자리와 공백 낀 것도 잡는다', () => {
  assert.equal(scanText(front + back).length, 1)
  assert.equal(scanText(`${front} - ${back}`).length, 1)
})

test('외국인 등록번호(성별자리 5~8)도 잡는다', () => {
  assert.equal(scanText(`${front}-5${'234567'}`).length, 1)
})

test('가짜 표시가 있는 줄은 통과한다', () => {
  assert.equal(scanText(`const rrn = '${front}-${back}' // ${FAKE_MARK}`).length, 0)
})

test('주민번호가 아닌 숫자는 잡지 않는다', () => {
  for (const s of [
    '2026-10-01',
    '000-000-0000',
    '제2026-152호',
    '1727712345678901', // 더 긴 숫자의 일부
    '851332-1234567', // 13월 32일은 날짜가 아니다 // privacy:fake
    `${front}-9${'234567'}`, // 성별자리 9
    `${front}-${back}8`, // 뒷자리가 7자리보다 길다
  ]) {
    assert.equal(scanText(s).length, 0, s)
  }
})

test('비밀 키 모양을 잡는다', () => {
  const pem = '-----BEGIN ' + 'PRIVATE KEY-----'
  const token = 'gh' + 'p_' + 'a'.repeat(36)
  const hits = scanText(`${pem}\n${token}`)
  assert.deepEqual(
    hits.map((h) => h.kind),
    ['개인 키(PEM)', 'GitHub 토큰'],
  )
})

test('자료·출력 파일은 이름만으로 거절한다', () => {
  for (const p of [
    'careercert.db',
    'src-tauri/careercert.db-wal',
    'backups/auto_20261001_090000.db',
    'docs/경력증명서_김으뜸.pdf',
    '방과후 강사 경력증명서 샘플.xlsm',
    'fixtures/import.xlsx',
    'app.log',
    '.env',
    '.env.local',
    'PASSWORD.txt',
    'career-cert.key',
    'samples/readme.md',
    'scratch/note.txt',
  ]) {
    assert.notEqual(checkPath(p), null, p)
  }
})

test('일반 소스 파일은 통과한다', () => {
  for (const p of [
    'src/App.tsx',
    'src-tauri/src/db/backup.rs',
    'src-tauri/migrations/001_init.sql',
    'docs/01-설계안.md',
    'src-tauri/icons/icon.ico',
    'package.json',
  ]) {
    assert.equal(checkPath(p), null, p)
  }
})
