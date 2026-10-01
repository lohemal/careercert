/**
 * 공개 저장소에 올려도 되는지 검사한다 (Phase 9 — 소스 저장소를 공개하기로 함).
 *
 *   npm run check:public              # Git 이 추적하는 파일
 *   npm run check:public -- --history # Git 이력의 **모든 블롭** + 한 번이라도 추가된 경로
 *
 * `.gitignore`·커밋 훅(check-privacy)만 믿지 않는다. 실제로 올라갈 내용을 읽는다.
 * 시험용 가짜 주민번호처럼 일부러 넣은 줄은 같은 줄에 `privacy:fake` 가 있어야 한다(커밋 훅과 같은 규칙).
 *
 * 막음(block) — 하나라도 있으면 실패한다.
 */
import { execSync } from 'node:child_process'
import { readFileSync, statSync } from 'node:fs'

const HISTORY = process.argv.includes('--history')
const BS = String.fromCharCode(92)

/** 이 프로그램이 쓰는 가상 표기 — 막지 않는다 */
const FAKE_SCHOOL = /^(○○|△△|가나|다라)/

const BLOCK = [
  { name: '주민등록번호', re: /\b\d{6}-?[1-8]\d{6}\b/g },
  { name: '전화번호', re: /\b0\d{1,2}-\d{3,4}-\d{4}\b/g, allow: (m) => /^0+-0+-0+$/.test(m) },
  { name: '학교 이름', re: /([가-힣○△]{2,})(초등학교|중학교|고등학교)/g, allow: (m) => FAKE_SCHOOL.test(m) || /^(초등학교|중학교|고등학교)/.test(m) },
  // 찾을 글자를 조각으로 이어 둔다 — 이 파일 자신이 커밋 훅의 키 검사에 걸리지 않게
  { name: '개인 키', re: new RegExp(['BEGIN [A-Z ]*PRIVATE ' + 'KEY', 'minisign encrypted ' + 'secret key', 'rsign encrypted ' + 'secret key'].join('|'), 'g') },
  { name: 'GitHub 토큰', re: /\bgh[pousr]_[A-Za-z0-9]{16,}|\bgithub_pat_[A-Za-z0-9_]{20,}/g },
  { name: '개인 이메일', re: /[A-Za-z0-9._%+-]+@(gmail\.com|naver\.com|daum\.net|korea\.kr|hanmail\.net)/gi },
  { name: '개인 컴퓨터 경로', re: new RegExp('[A-Za-z]:' + BS + BS + '+Users' + BS + BS + '+(?!<)[A-Za-z0-9._-]+', 'g') },
  { name: '실제 샘플의 값', re: new RegExp(['한빛' + '초', '044-' + '903'].join('|'), 'g') },
]

/** 내용을 보지 않고 있다는 것만으로 막는 형식 */
const BLOCK_EXT = ['.db', '.sqlite', '.sqlite3', '.db-wal', '.db-shm', '.xlsx', '.xlsm', '.xls', '.csv', '.hwp', '.hwpx', '.pdf',
  '.careercert-backup', '.key', '.pem', '.pfx', '.p12', '.env', '.log', '.restore-pending', '.part', '.exe', '.sig']
/** 내용을 읽지 않는 형식 (그림·글꼴) */
const SKIP_READ = ['.png', '.ico', '.icns', '.jpg', '.jpeg', '.woff', '.woff2', '.ttf', '.otf']

function ext(path) {
  const l = path.toLowerCase()
  const name = l.slice(l.lastIndexOf('/') + 1)
  if (name === '.env' || name.startsWith('.env.')) return '.env'
  return name.includes('.') ? name.slice(name.lastIndexOf('.')) : ''
}

let blocked = 0
const report = (where, what) => {
  console.error(`✗ 막음  ${where}\n        ${what}`)
  blocked++
}

function scanText(where, text) {
  const lines = text.split('\n')
  lines.forEach((line, i) => {
    if (line.includes('privacy:fake')) return
    for (const { name, re, allow } of BLOCK) {
      for (const m of line.matchAll(re)) {
        if (allow && allow(m[0])) continue
        report(`${where}:${i + 1}`, `${name} — ${JSON.stringify(m[0].slice(0, 40))}`)
      }
    }
  })
}

const SELF = 'scripts/check-public-safe.mjs'

if (!HISTORY) {
  // -z: 한글 경로가 이스케이프되어 "없는 파일" 로 조용히 빠지는 것을 막는다
  const files = execSync('git ls-files -z', { encoding: 'utf8', maxBuffer: 64 << 20 }).split('\0').filter(Boolean)
  let read = 0
  let skipped = 0
  for (const f of files) {
    if (f === SELF) { skipped++; continue }
    const e = ext(f)
    if (BLOCK_EXT.includes(e)) { report(f, `이 형식은 공개 저장소에 두지 않는다 (${e})`); skipped++; continue }
    if (SKIP_READ.includes(e)) { skipped++; continue }
    let text
    try {
      if (statSync(f).size > 4_000_000) { report(f, '너무 커서 읽지 않았다 — 무엇인지 확인'); skipped++; continue }
      text = readFileSync(f, 'utf8')
    } catch (err) {
      report(f, `읽지 못해 검사하지 못했다 — ${err.code ?? err.message}`)
      skipped++
      continue
    }
    read++
    scanText(f, text)
  }
  console.log(`\n추적 파일 ${files.length}개 — 읽음 ${read} · 형식상 건너뜀 ${skipped}`)
  if (read + skipped !== files.length) {
    console.error('✗ 목록과 읽은 수가 맞지 않습니다 — 검사하지 못한 파일이 있습니다.')
    process.exit(1)
  }
} else {
  // 이력의 모든 블롭과 경로
  const objects = execSync('git rev-list --objects --all', { encoding: 'utf8', maxBuffer: 256 << 20 })
    .split('\n').filter(Boolean)
    .map((l) => { const sp = l.indexOf(' '); return sp < 0 ? { sha: l, path: '' } : { sha: l.slice(0, sp), path: l.slice(sp + 1) } })
  const paths = new Set()
  let blobs = 0
  for (const { sha, path } of objects) {
    if (!path) continue
    const type = execSync(`git cat-file -t ${sha}`, { encoding: 'utf8' }).trim()
    if (type !== 'blob') continue
    // 경로가 따옴표로 이스케이프되었을 수 있다 — 형식 판단에는 끝부분만 쓴다
    const p = path.replace(/^"|"$/g, '')
    paths.add(p)
    if (p.endsWith('check-public-safe.mjs')) continue
    const e = ext(p)
    if (BLOCK_EXT.includes(e)) { report(`${p} @${sha.slice(0, 8)}`, `이 형식이 이력에 있다 (${e})`); continue }
    if (SKIP_READ.includes(e)) continue
    const buf = execSync(`git cat-file -p ${sha}`, { maxBuffer: 64 << 20 })
    blobs++
    scanText(`${p} @${sha.slice(0, 8)}`, buf.toString('utf8'))
  }
  const authors = execSync('git log --all --format=%an%x09%ae%x09%cn%x09%ce', { encoding: 'utf8' }).split('\n').filter(Boolean)
  for (const a of new Set(authors)) {
    for (const v of a.split('\t')) if (/@(gmail\.com|naver\.com|daum\.net|korea\.kr|hanmail\.net)$/i.test(v)) report('커밋 작성자', `개인 이메일 — ${v}`)
  }
  console.log(`\n이력의 블롭 ${blobs}개를 읽었다 · 한 번이라도 있었던 경로 ${paths.size}개 · 작성자 ${new Set(authors).size}종`)
}

if (blocked > 0) {
  console.error(`\n막음 ${blocked}건 — 공개하면 안 되는 것이 있습니다.`)
  process.exit(1)
}
console.log('check-public: 통과')
