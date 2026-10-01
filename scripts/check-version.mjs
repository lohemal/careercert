/**
 * 버전이 적힌 자리가 모두 같은지 본다. 어긋나면 실패한다.
 *
 *   npm run check:version
 *
 * 버전이 어긋난 채로 배포하면 업데이터가 새 버전을 알아보지 못하거나
 * 설치 파일 이름과 latest.json 이 맞지 않아 업데이트가 조용히 깨진다.
 *
 * 어디를 보는지는 `version-files.mjs` 에 있다 (`set-version.mjs` 와 같은 목록).
 */
import { readAll } from './version-files.mjs'

const all = readAll()
for (const [where, v] of Object.entries(all)) console.log(`  ${where.padEnd(26)} ${v ?? '(못 읽음)'}`)

const values = [...new Set(Object.values(all))]
if (values.length !== 1 || !values[0]) {
  console.error('\n✗ 버전이 서로 다릅니다. `npm run version:set <버전>` 으로 맞춰 주세요.')
  process.exit(1)
}
console.log(`\ncheck-version: 통과 (v${values[0]})`)
