# 함께 배포하는 제3자 자료

| 자료 | 앱 안의 위치 | 라이선스 | 출처 |
|---|---|---|---|
| Noto Sans KR Regular · Bold | `public/fonts/NotoSansKR-Regular.ttf`, `NotoSansKR-Bold.ttf` | SIL Open Font License 1.1 — `NotoSansKR-OFL.txt` | Google Fonts 저장소 `google/fonts` 의 `ofl/notosanskr/NotoSansKR[wght].ttf` (버전 2.004) |
| pdf.js (`pdfjs-dist` 6.3) | 화면 번들 (발급 전 미리보기를 열 때 읽음) | Apache License 2.0 — `pdfjs-LICENSE.txt` | Mozilla `pdfjs-dist` (npm) |

## Noto Sans KR

- 증명서 양식(`render::v1`)이 이 글꼴만 쓴다. PC 에 설치된 글꼴과 상관없이 출력 모양이 같도록 앱에 넣었다.
- 배포본의 가변 글꼴(굵기 100–900)에서 **굵기 축만 고정**해 Regular(400)·Bold(700) 두 파일로 만들었다
  (fontTools `varLib.instancer`). **글자를 잘라 내지 않았다** — 두 파일 모두 원본과 같은 23,174자
  (한글 음절 11,172자 전부, 한자 8,138자), 글리프 24,964개.
  - 고정 굵기로 만든 까닭: Chromium 은 가변 글꼴을 PDF 에 Type3 로 넣는데, Type3 는 프린터·PDF 뷰어에
    따라 품질·속도가 떨어질 수 있다. 고정 굵기는 TrueType(Type0)으로 들어간다(실측).
- OFL 은 글꼴을 프로그램과 함께 배포하는 것을 허용한다. 글꼴 파일만 따로 팔 수 없고, 고친 판은
  예약 글꼴 이름('Source' 등)을 쓸 수 없다. 굵기만 고정한 이 판은 원래 이름(Noto Sans KR)을 그대로 쓴다 —
  배포처(Google Fonts)가 내려받기 묶음에 넣어 주는 고정 굵기 판과 같은 방식이다.
- sha256
  - 원본 가변 글꼴 `NotoSansKR[wght].ttf`: `194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252`
  - `NotoSansKR-Regular.ttf`: `c8439a64f890e6d534e4205efa00c3761ada3d6d152e332a06c303d971884cd9`
  - `NotoSansKR-Bold.ttf`: `208f97f58b81160b00d0467ebdabb5059a6f6825fb674a28de04f547b24ac71f`

## pdf.js

- **발급 전 미리보기**를 그림(canvas)으로 보여 주는 데만 쓴다. WebView2 내장 PDF 뷰어는 저장·인쇄 버튼을
  숨기는 설정이 듣지 않아(Phase 5 기술 검증) 발급 전 문서를 보여 주는 데 쓰지 않는다.
