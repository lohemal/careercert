#!/usr/bin/env node
// 연습용(sandbox) 자료 폴더에 **가상** 강사·경력을 넣는다.  `npm run seed` (`-- --reset` 로 갈아엎기)
//
// 안전장치 — 실제 자료에는 절대 쓰지 않는다
//   1. 쓰는 파일은 `%APPDATA%\kr.school.careercert.sandbox\careercert.db` **하나뿐**이다.
//      다른 경로를 받는 옵션이 없다. 경로가 sandbox 폴더가 아니면 멈춘다(assertSandboxPath).
//   2. 그 파일이 이 프로그램의 자료(app_meta.app_id)이고 자료 구조가 v4 이상인지 확인한다.
//      파일이 없으면 만들지 않는다 — 연습용 앱을 한 번 켜서 만들게 한다.
//   3. 이미 강사가 있으면 `--reset` 없이는 넣지 않는다.
//
// 이름은 모두 지어낸 것이고, 메모에 "연습용 가상 자료" 를 적는다. 주민번호·주소는 없다(원장에 칸이 없다).
// 날짜는 실행하는 해를 기준으로 만든다 — 언제 돌려도 "올해 재직중" 이 나온다.

import { randomUUID } from 'node:crypto'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const SANDBOX_DIR = 'kr.school.careercert.sandbox'
export const DB_FILE = 'careercert.db'
const APP_ID = 'kr.school.careercert'
const MIN_SCHEMA = 5

/** `%APPDATA%\kr.school.careercert.sandbox\careercert.db` */
export function sandboxDbPath(env = process.env) {
  if (!env.APPDATA) throw new Error('APPDATA 환경변수가 없습니다 (Windows 에서만 실행합니다).')
  return join(env.APPDATA, SANDBOX_DIR, DB_FILE)
}

/** sandbox 자료 파일이 아니면 던진다. 실제 자료 폴더(`kr.school.careercert`)는 물론 그 밖의 어떤 경로도 거절. */
export function assertSandboxPath(p) {
  const parts = p.replace(/\\/g, '/').toLowerCase().split('/').filter(Boolean)
  const [dir, file] = parts.slice(-2)
  if (dir !== SANDBOX_DIR || file !== DB_FILE) {
    throw new Error(`연습용 자료 파일이 아니어서 멈췄습니다: ${p}`)
  }
}

const pad = (n) => String(n).padStart(2, '0')
const iso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const stamp = (d) => `${iso(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`

/**
 * 가상 자료. y = 올해.
 * 경력: [프로그램, 시작, 종료|null(재직중), 사유|null, 보관?, 예정 종료일?]
 */
export function plan(y) {
  const year = (from, to, program, reason = 'CONTRACT_END') => [program, `${from}-03-04`, `${to}-02-10`, reason]
  return [
    {
      name: '김가람', distinguisher: '', note: '여러 해 경력 · 지금 재직중',
      careers: [
        year(y - 4, y - 3, '마술'), year(y - 3, y - 2, '마술'), year(y - 2, y - 1, '마술'),
        ['마술', `${y - 1}-03-05`, `${y}-02-06`, 'CONTRACT_END'],
        ['마술', `${y}-03-04`, null, null, false, `${y + 1}-02-05`],
      ],
    },
    { name: '이나래', distinguisher: '', note: '일반 강사', careers: [['바둑', `${y}-03-04`, null, null, false, `${y + 1}-02-05`]] },
    {
      name: '박다솜', distinguisher: '', note: '계약이 모두 끝난 강사',
      careers: [year(y - 2, y - 1, '생명과학'), year(y - 1, y, '생명과학')],
    },
    {
      name: '최라온', distinguisher: '', note: '중도해지',
      careers: [year(y - 1, y, '배드민턴'), ['배드민턴', `${y}-03-04`, `${y}-06-30`, 'TERMINATED', false, `${y + 1}-02-05`]],
    },
    { name: '정하늘', distinguisher: '1985년생', note: '동명이인(구분 메모 있음)', careers: [['미술', `${y}-03-04`, null, null]] },
    { name: '정하늘', distinguisher: '음악 강사', note: '동명이인(구분 메모 있음)', careers: [['음악', `${y - 1}-03-05`, null, null]] },
    { name: '임가온', distinguisher: '', note: '동명이인(구분 메모 없음 → 대시보드 확인 필요)', careers: [['요리', `${y}-03-04`, null, null]] },
    { name: '임가온', distinguisher: '', note: '동명이인(구분 메모 없음 → 대시보드 확인 필요)', careers: [['과학실험', `${y}-03-04`, null, null]] },
    {
      name: '윤슬기', distinguisher: '', note: '잘못 넣은 경력 하나를 보관함 · 예정 종료일이 지났는데 재직중',
      careers: [['로봇과학', `${y}-03-04`, null, null, false, `${y}-08-31`], ['로봇과학', `${y}-03-04`, null, null, true]],
    },
    {
      name: '강다온', distinguisher: '', note: '종료일을 미리 넣어 둔 경력(올해 12월 31일)',
      careers: [['코딩', `${y}-03-04`, `${y}-12-31`, 'CONTRACT_END']],
    },
    {
      name: '한보람', distinguisher: '이전 계약자', note: '보관한 강사', archived: true,
      careers: [year(y - 3, y - 2, '서예')],
    },
  ]
}

async function main() {
  const reset = process.argv.includes('--reset')
  const path = sandboxDbPath()
  assertSandboxPath(path)
  if (!existsSync(path)) {
    console.error(`[seed] 연습용 자료 파일이 없습니다: ${path}`)
    console.error('       먼저 `npm run app:sandbox` 로 연습용 앱을 한 번 켜 주세요.')
    process.exit(1)
  }

  const { DatabaseSync } = await import('node:sqlite')
  const db = new DatabaseSync(path)
  db.exec('PRAGMA foreign_keys = ON')

  const appId = db.prepare("SELECT value FROM app_meta WHERE key = 'app_id'").get()?.value
  const version = db.prepare('PRAGMA user_version').get().user_version
  if (appId !== APP_ID) throw new Error('이 프로그램의 자료 파일이 아닙니다.')
  if (version < MIN_SCHEMA) {
    throw new Error(`자료 구조가 v${version} 입니다. 연습용 앱을 새 버전으로 한 번 켜서 v${MIN_SCHEMA} 이상으로 올려 주세요.`)
  }

  const existing = db.prepare('SELECT COUNT(*) AS n FROM instructors').get().n
  if (existing > 0 && !reset) {
    console.error(`[seed] 이미 강사 ${existing}명이 있어 넣지 않았습니다. 지우고 다시 넣으려면: npm run seed -- --reset`)
    process.exit(1)
  }

  const now = new Date()
  const at = stamp(now)
  const y = now.getFullYear()
  const people = plan(y)

  const insI = db.prepare(
    `INSERT INTO instructors (uuid, name, distinguisher, phone, memo, archived_at, created_at, updated_at)
     VALUES (?, ?, ?, '', ?, ?, ?, ?)`,
  )
  const insC = db.prepare(
    `INSERT INTO careers (uuid, instructor_id, program_name, position, duty, start_date, end_date,
                          status, end_reason, memo, archived_at, created_at, updated_at, planned_end_date)
     VALUES (?, ?, ?, '강사', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  )
  const log = db.prepare(
    "INSERT INTO audit_log (at, action, target_type, target_id, summary) VALUES (?, ?, 'seed', NULL, ?)",
  )

  let careers = 0
  db.exec('BEGIN')
  try {
    if (reset) {
      db.exec('DELETE FROM careers; DELETE FROM instructors;')
      log.run(at, 'SEED_RESET', '연습용 강사·경력을 모두 지움')
    }
    for (const p of people) {
      const memo = `연습용 가상 자료 — ${p.note}`
      const id = insI.run(randomUUID(), p.name, p.distinguisher, memo, p.archived ? at : null, at, at).lastInsertRowid
      for (const [program, start, end, reason, archived, planned] of p.careers) {
        insC.run(
          randomUUID(), id, program, `방과후학교 ${program}`, start, end,
          end ? 'ENDED' : 'ACTIVE', end ? reason : null,
          archived ? '연습용 — 잘못 넣어 보관한 경력' : '', archived ? at : null, at, at, planned ?? null,
        )
        careers++
      }
    }
    log.run(at, 'SEED', `연습용 가상 강사 ${people.length}명 · 경력 ${careers}건`)
    db.exec('COMMIT')
  } catch (e) {
    db.exec('ROLLBACK')
    throw e
  }
  db.close()
  console.log(`[seed] ${path}`)
  console.log(`[seed] 가상 강사 ${people.length}명 · 경력 ${careers}건을 넣었습니다. 연습용 앱이 켜져 있으면 화면을 새로 고쳐 주세요.`)
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch((e) => {
    console.error(`[seed] ${e.message}`)
    process.exit(1)
  })
}
