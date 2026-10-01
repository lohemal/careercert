#!/usr/bin/env node
// 저장소에 개인정보·자료 파일·비밀 키가 들어가는 것을 막는다 (설계안 §15.4).
//
//   node scripts/check-privacy.mjs          커밋하려는(스테이지된) 파일만 검사 — pre-commit 훅이 부른다
//   node scripts/check-privacy.mjs --all    저장소에 올라가 있는 파일 전체 검사 — 릴리스 전 필수
//
// 무엇을 보는가
//   1) 파일 종류: DB·백업·PDF·엑셀·로그·키 파일은 이름만 보고 거절한다.
//   2) 내용: 주민등록번호 모양(YYMMDD-[1-8]NNNNNN, 하이픈 없는 13자리 포함)과 비밀 키 모양.
//      시험용 가짜 값이 꼭 필요하면 **같은 줄에 `privacy:fake`** 를 적어야 통과한다.
//
// 걸린 값은 화면에 **다시 찍지 않는다** — 터미널 기록에 개인정보가 남지 않게 위치와 종류만 알린다.
//
// 검사 대상은 작업 폴더가 아니라 **git 색인(index)의 내용**이다(`git show :경로`).
// 커밋되는 것은 색인이므로, 파일을 고쳐 놓고 다시 add 하지 않은 경우도 정확히 본다.
// 경로는 `-z` 로 받는다 — 한글 경로를 git 이 따옴표·8진수로 바꿔 내보내는 것을 피한다.

import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

/** 이름만으로 거절하는 확장자 */
export const BLOCKED_EXTENSIONS = [
  '.db', '.db-shm', '.db-wal', '.db-journal', '.sqlite', '.sqlite3',
  '.restore-pending', '.part',
  '.pdf', '.xlsx', '.xlsm', '.xls', '.csv', '.hwp', '.hwpx',
  '.log', '.dmp',
  '.key', '.pem', '.dpapi',
]

/** 이름만으로 거절하는 파일 이름(소문자 비교) */
const BLOCKED_NAMES = ['.env']

/** 이 폴더 아래는 무엇이든 거절 */
const BLOCKED_DIRS = ['backups/', 'exports/', 'scratch/', 'samples/', 'private/', 'logs/']

/** 가짜 값임을 사람이 확인했다는 표시 */
export const FAKE_MARK = 'privacy:fake'

// 주민등록번호 모양: 생년월일(월 01~12, 일 01~31) + 성별자리 1~8 + 6자리.
// 하이픈·공백은 있어도 없어도 된다. 앞뒤가 숫자이면 더 긴 숫자의 일부이므로 보지 않는다.
const RRN = /(?<![0-9])[0-9]{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12][0-9]|3[01])[ \t]?-?[ \t]?[1-8][0-9]{6}(?![0-9])/

// 비밀 키 모양. 찾는 글자를 그대로 적으면 이 파일이 자기 자신에게 걸리므로 조각으로 잇는다.
const SECRET_PATTERNS = [
  { kind: '개인 키(PEM)', re: new RegExp('-----BEGIN [A-Z ]*' + 'PRIVATE KEY-----') },
  { kind: '업데이터 서명 키', re: new RegExp('rsign encrypted ' + 'secret key') },
  { kind: 'GitHub 토큰', re: new RegExp('\\b(?:gh' + 'p|gh' + 'o|gh' + 's|github' + '_pat)_[A-Za-z0-9_]{20,}') },
  { kind: 'OpenAI 키', re: new RegExp('\\bs' + 'k-(?:proj-)?[A-Za-z0-9_-]{20,}') },
]

/**
 * 경로가 이름만으로 거절 대상인가. 거절이면 까닭을, 아니면 null.
 * @param {string} path 저장소 기준 경로 (`/` 구분)
 */
export function checkPath(path) {
  const p = path.replace(/\\/g, '/')
  const lower = p.toLowerCase()
  const base = lower.split('/').pop() ?? lower
  for (const d of BLOCKED_DIRS) {
    if (lower.startsWith(d) || lower.includes('/' + d)) return `${d} 폴더의 파일`
  }
  if (BLOCKED_NAMES.includes(base) || base.startsWith('.env.')) return '환경변수 파일'
  if (/^password.*\.txt$/.test(base)) return '비밀번호 파일'
  for (const ext of BLOCKED_EXTENSIONS) {
    if (base.endsWith(ext)) return `${ext} 파일`
  }
  return null
}

/**
 * 글 내용을 훑어 걸린 줄을 돌려준다. 값 자체는 담지 않는다.
 * @param {string} text
 * @returns {{ line: number, kind: string }[]}
 */
export function scanText(text) {
  const out = []
  const lines = text.split(/\r?\n/)
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    if (RRN.test(line) && !line.includes(FAKE_MARK)) {
      out.push({ line: i + 1, kind: '주민등록번호 모양' })
    }
    for (const s of SECRET_PATTERNS) {
      if (s.re.test(line)) out.push({ line: i + 1, kind: s.kind })
    }
  }
  return out
}

/** NUL 바이트가 있으면 이진 파일로 본다 (그림·글꼴 등). */
function isBinary(buf) {
  const n = Math.min(buf.length, 8000)
  for (let i = 0; i < n; i++) if (buf[i] === 0) return true
  return false
}

function git(args, opts = {}) {
  return execFileSync('git', args, { maxBuffer: 256 * 1024 * 1024, ...opts })
}

function listFiles(all) {
  const raw = all
    ? git(['ls-files', '-z'])
    : git(['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'])
  return raw.toString('utf8').split('\0').filter(Boolean)
}

function main() {
  const all = process.argv.includes('--all')
  const files = listFiles(all)
  const problems = []

  for (const f of files) {
    const why = checkPath(f)
    if (why) {
      problems.push(`${f}  — 올리면 안 되는 파일(${why})`)
      continue
    }
    let buf
    try {
      buf = git(['show', `:${f}`])
    } catch {
      continue // 색인에 없는 파일(지운 것 등)
    }
    if (isBinary(buf)) continue
    for (const hit of scanText(buf.toString('utf8'))) {
      problems.push(`${f}:${hit.line}  — ${hit.kind}`)
    }
  }

  const scope = all ? '저장소 전체' : '커밋할 파일'
  if (problems.length === 0) {
    console.log(`[check-privacy] ${scope} ${files.length}개 — 문제 없음`)
    return
  }
  console.error(`[check-privacy] ${scope}에서 ${problems.length}건을 찾아 멈췄습니다.`)
  for (const p of problems) console.error('  ' + p)
  console.error('')
  console.error('  · 자료·출력 파일이면 git rm --cached <파일> 로 빼 주세요.')
  console.error(`  · 시험용 가짜 주민번호라면 같은 줄에 ${FAKE_MARK} 를 적어 주세요.`)
  process.exit(1)
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main()
}
