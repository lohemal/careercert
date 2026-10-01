# 방과후 강사 경력증명서 발급 시스템

방과후학교 강사의 경력을 관리하고 경력증명서(활동 확인서)를 발급·보관·재출력하는 Windows 데스크톱 프로그램.
모든 자료는 이 컴퓨터에만 저장된다. 주민등록번호·주소는 강사 원장에 두지 않고 발급할 때만 받아 암호화해 발급 기록에 남긴다.

- **사용 안내서**: [`docs/02-사용안내서.md`](docs/02-사용안내서.md) · 실사용 점검표: [`docs/03-실사용-점검표.md`](docs/03-실사용-점검표.md)
- 기준 문서: [`docs/01-설계안.md`](docs/01-설계안.md) — 확정된 결정은 §15, 진행 기록은 §16
- 구성: Tauri 2 · React + TypeScript · Rust · SQLite

## 설치 (사용자)

[Releases](https://github.com/lohemal/careercert/releases) 에서 `careercert_<버전>_x64-setup.exe` 를 받아 실행한다(관리자 권한 불필요).
새 버전은 프로그램 안에서 알려 주고 [업데이트] 로 설치한다(서명 확인 후 설치).

## 개발

```bash
npm install
npm run hooks:install      # 커밋 전 개인정보 검사 훅 (처음 한 번)
npm run app                # 실제 자료 폴더로 실행
npm run app:sandbox        # 연습용 자료 폴더로 실행 (실제 자료와 섞이지 않음)
npm run seed               # 연습용 자료에 가상 강사·경력 넣기 (-- --reset 로 갈아엎기, 실제 자료에는 쓰지 않음)
```

| 검사 | 명령 |
|---|---|
| 화면 타입 검사·빌드 | `npm run build` |
| Rust 시험 | `npm run test:rust` (시험 폴더는 끝나면 지운다 · 남기려면 `CAREERCERT_KEEP_TEST_DIRS=1`) |
| 규모 시험 (강사 500 · 발급 5,000) | `cargo test --release --lib perf -- --ignored --nocapture` (src-tauri 에서) |
| 개인정보 검사 규칙 · 하드코딩 검사 | `npm run test:scripts` |
| 화면 규칙 (작성 값 · 증명서 2판 쪽 나누기) | `npm run test:ui` |
| 양식 눈 시험용 HTML (1판·2판, 가상 자료) | `CAREERCERT_DUMP_HTML=<폴더> cargo test --lib 눈시험용` (src-tauri 에서) |
| 전부 | `npm test` |
| 저장소 전체 개인정보 검사 (릴리스 전 필수) | `npm run check:privacy -- --all` |
| 공개 저장소 검사 (추적 파일 / 이력 전체) | `npm run check:public` / `npm run check:public -- --history` |

## 릴리스

```bash
npm run version:set 0.1.1         # 다섯 곳의 버전을 맞춘다
npm run check:version
git commit -am "v0.1.1" && git tag -a v0.1.1 -m "v0.1.1"
git push origin main --tags       # Actions 가 시험 → 빌드 → 서명 → 서명 검증 → Release
```

업데이트 서명 키는 저장소 밖(`%USERPROFILE%\.careercert-release\`)에 있고, 저장소 secret
`TAURI_SIGNING_PRIVATE_KEY`(키 파일 **내용**) · `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 로만 CI 에 들어간다.
**키를 잃으면 이미 설치된 프로그램에 업데이트를 보낼 수 없다.**

자료 위치

| 실행 | 폴더 |
|---|---|
| 실제 | `%APPDATA%\kr.school.careercert\careercert.db` |
| 연습용 | `%APPDATA%\kr.school.careercert.sandbox\careercert.db` |
| 백업 | 각 폴더의 `backups\` |

## 개인정보 원칙

- 실제 강사 명단·주민번호·주소·DB·백업·PDF·엑셀 원본을 저장소에 넣지 않는다.
  `.gitignore` 로 1차 차단하고, 커밋 때 `scripts/check-privacy.mjs` 가 파일 종류와 내용을 다시 검사한다.
- 시험 자료는 코드가 만드는 가상 자료만 쓴다. 주민번호 모양 가짜 값이 꼭 필요하면 같은 줄에 `privacy:fake` 를 적는다.
