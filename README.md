# 방과후 강사 경력증명서 발급 시스템

방과후학교 강사의 경력을 관리하고 경력증명서(활동 확인서)를 발급·보관·재출력하는 Windows 데스크톱 프로그램.
모든 자료는 이 컴퓨터에만 저장된다.

- 기준 문서: [`docs/01-설계안.md`](docs/01-설계안.md) — 확정된 결정은 §15, 진행 기록은 §16
- 구성: Tauri 2 · React + TypeScript · Rust · SQLite

## 개발

```bash
npm install
npm run hooks:install      # 커밋 전 개인정보 검사 훅 (처음 한 번)
npm run app                # 실제 자료 폴더로 실행
npm run app:sandbox        # 연습용 자료 폴더로 실행 (실제 자료와 섞이지 않음)
```

| 검사 | 명령 |
|---|---|
| 화면 타입 검사·빌드 | `npm run build` |
| Rust 시험 | `npm run test:rust` |
| 개인정보 검사 규칙 시험 | `npm run test:privacy` |
| 저장소 전체 개인정보 검사 (릴리스 전 필수) | `npm run check:privacy -- --all` |

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
