import { useQuery } from '@tanstack/react-query'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { AppShell } from '@/components/AppShell'
import { appApi } from '@/ipc/app'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { DashboardPage } from '@/pages/DashboardPage'
import { PlaceholderPage } from '@/pages/PlaceholderPage'
import s from './App.module.css'

export function App() {
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })

  if (info.isLoading) {
    return (
      <div className={s.splash}>
        <div className={s.splashMark} />
        <p className={s.splashText}>경력증명서 발급 시스템을 준비하는 중…</p>
      </div>
    )
  }

  if (info.error || !info.data) {
    return (
      <div className={s.splash}>
        <h1 className={s.fatalTitle}>프로그램을 시작하지 못했습니다</h1>
        <p className={s.fatalMsg}>{errorMessage(info.error)}</p>
        {errorDetail(info.error) && (
          <details className={s.fatalDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{errorDetail(info.error)}</pre>
          </details>
        )}
      </div>
    )
  }

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell appVersion={info.data.appVersion} sandbox={info.data.sandbox} />}>
          <Route path="/dashboard" element={<DashboardPage info={info.data} />} />
          <Route
            path="/instructors"
            element={
              <PlaceholderPage
                title="강사관리"
                phase="Phase 2~3"
                description="강사 검색, 경력 등록·수정·종료 처리, 학년도 말 일괄 종료(선택 → 미리보기 → 승인)."
              />
            }
          />
          <Route
            path="/issue"
            element={
              <PlaceholderPage
                title="증명서 발급"
                phase="Phase 4~6"
                description="강사 선택 → 경력 선택 → 발급정보 입력 → 미리보기 → 발급 확정."
              />
            }
          />
          <Route
            path="/history"
            element={
              <PlaceholderPage
                title="발급이력"
                phase="Phase 7"
                description="발급된 증명서 검색·열람, 발급 당시 내용 그대로 재출력·PDF 저장."
              />
            }
          />
          <Route
            path="/settings"
            element={
              <PlaceholderPage
                title="설정"
                phase="Phase 1 · 8"
                description="학교 기본정보(Phase 1), 엑셀 가져오기·백업·복원(Phase 8)."
              />
            }
          />
        </Route>
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </HashRouter>
  )
}
