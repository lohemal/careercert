import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { ImagePlus, Trash2 } from 'lucide-react'

import { Button, ErrorNotice } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { logoApi } from '@/ipc/settings'
import s from './SchoolLogoField.module.css'

/**
 * 학교 로고 (v0.1.2) — 고르면 바로 검사해 프로그램 안에 보관한다(원본 파일 위치는 기억하지 않는다).
 * 앞으로 새로 발급하는 증명서에만 쓰인다. 증명서에는 경력표 뒤 연한 흑백 워터마크로 나온다.
 */
export function SchoolLogoField() {
  const qc = useQueryClient()
  const [confirm, confirmDialog] = useConfirm()
  const logo = useQuery({ queryKey: ['school-logo'], queryFn: logoApi.get })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const l = logo.data

  const run = async (f: () => Promise<unknown>) => {
    setBusy(true)
    setError(null)
    try {
      const r = await f()
      if (r) qc.setQueryData(['school-logo'], r)
      void qc.invalidateQueries({ queryKey: ['backup-reminder'] })
    } catch (e) {
      setError(e)
    } finally {
      setBusy(false)
    }
  }

  const remove = async () => {
    const ok = await confirm({
      title: '학교 로고 삭제',
      tone: 'danger',
      confirmLabel: '삭제',
      message: '로고를 지우면 앞으로 새로 발급하는 증명서에는 로고가 들어가지 않습니다. 이미 발급한 증명서의 로고는 그대로입니다.',
    })
    if (ok) await run(logoApi.remove)
  }

  return (
    <div className={s.box}>
      <span className={s.label}>학교 로고 (선택)</span>
      <div className={s.row}>
        <div className={s.preview} aria-label="학교 로고 미리보기">
          {l?.preview ? <img src={l.preview} alt="" /> : <span className={s.empty}>없음</span>}
        </div>
        <div className={s.side}>
          <div className={s.actions}>
            <Button size="sm" icon={ImagePlus} onClick={() => run(logoApi.pick)} disabled={busy}>
              {l?.present ? '로고 변경' : '로고 선택'}
            </Button>
            {l?.present && (
              <Button size="sm" variant="ghost" icon={Trash2} onClick={remove} disabled={busy}>
                로고 삭제
              </Button>
            )}
          </div>
          <p className={s.help}>
            PNG·JPG·WebP (10MB · 가로·세로 8000픽셀 이하). 컬러 이미지를 등록해도 증명서에는 연한 흑백 워터마크로 출력됩니다.
            고르면 바로 저장됩니다.
          </p>
          <p className={s.note}>
            학교 로고 변경은 앞으로 새로 발급하는 증명서에 적용됩니다. 이미 발급한 증명서의 로고는 변경되지 않습니다.
          </p>
          <ErrorNotice error={logo.error ?? error} />
        </div>
      </div>
      {confirmDialog}
    </div>
  )
}
