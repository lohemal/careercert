import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { ArrowLeft, ArrowRight, CalendarCheck, Eye } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Field, Input, Notice, Page } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { invalidateCareerData } from '@/features/instructors/invalidate'
import { whoLabel } from '@/features/instructors/label'
import { bulkEndApi, type Applied, type BulkEndRequest, type Preview } from '@/ipc/bulkEnd'
import { END_REASONS, type EndReason } from '@/ipc/career'
import { isAppError } from '@/ipc/invoke'
import s from './BulkEndPage.module.css'

/**
 * 재직중 경력 일괄 종료 (설계안 §15.2).
 *
 *   1. 재직중 경력 목록 — **기본 선택 0건**, 담당자가 직접 고른다
 *   2. 종료일 · 종료 사유
 *   3. [변경내용 미리보기] — 줄마다 변경 전 → 변경 후, 끝낼 수 없는 줄은 이유
 *   4. [적용] — 확인 창(미래 종료일이면 그 경고 포함) → 서버가 다시 확인하고 백업 후 한 번에
 *
 * 고른 것·날짜·사유를 바꾸면 미리보기는 버려진다 — 본 것과 다른 것이 적용되지 않게.
 * 프로그램은 날짜를 보고 저절로 끝내지 않는다.
 */
export function BulkEndPage() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const [confirm, confirmDialog] = useConfirm()
  const candidates = useQuery({ queryKey: ['bulk-candidates'], queryFn: bulkEndApi.candidates })

  const [picked, setPicked] = useState<Set<number>>(new Set())
  const [endDate, setEndDate] = useState('')
  const [reason, setReason] = useState<EndReason>('CONTRACT_END')
  const [filter, setFilter] = useState('')
  const [preview, setPreview] = useState<Preview | null>(null)
  const [done, setDone] = useState<Applied | null>(null)

  const request: BulkEndRequest = { careerIds: [...picked], endDate, reason }

  /** 조건이 바뀌면 미리보기를 버린다 */
  const changed = () => {
    setPreview(null)
    setDone(null)
    previewRun.reset()
    applyRun.reset()
  }

  const previewRun = useMutation({
    mutationFn: () => bulkEndApi.preview(request),
    onSuccess: setPreview,
  })

  const applyRun = useMutation({
    mutationFn: async () => {
      const p = preview!
      const ok = await confirm({
        title: '일괄 종료 적용',
        tone: 'primary',
        confirmLabel: `${p.items.length}건 종료`,
        message: (
          <>
            {p.future && (
              <p className={s.futureLine}>
                종료일이 오늘 이후입니다. {p.endDateLabel}로 종료 처리하시겠습니까?
              </p>
            )}
            재직중 경력 <b>{p.items.length}건</b>을 종료일 <b>{p.endDateLabel}</b> · <b>{p.reasonLabel}</b>(으)로 종료합니다.
            <br />
            적용하기 직전에 자료를 백업합니다. 한 건이라도 처리할 수 없으면 아무것도 바꾸지 않습니다.
          </>
        ),
      })
      if (!ok) return null
      // 확인 창에서 미래 종료일까지 확인했으므로 confirmFuture 를 함께 보낸다
      return bulkEndApi.apply(request, p.stateKey, p.future)
    },
    onSuccess: (r) => {
      if (!r) return
      setDone(r)
      setPreview(null)
      setPicked(new Set())
      invalidateCareerData(qc)
    },
    onError: (e) => {
      // 미리보기 뒤에 자료가 바뀌었다 — 다시 미리보기를 받아야 한다
      if (isAppError(e) && e.code === 'BULK_STALE') setPreview(null)
    },
  })

  const rows = useMemo(() => {
    const q = filter.trim()
    return (candidates.data ?? []).filter(
      (c) => !q || c.instructorName.includes(q) || c.distinguisher.includes(q) || c.programName.includes(q),
    )
  }, [candidates.data, filter])

  const toggle = (id: number) => {
    const next = new Set(picked)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    setPicked(next)
    changed()
  }
  const allShownPicked = rows.length > 0 && rows.every((r) => picked.has(r.id))
  const toggleShown = () => {
    const next = new Set(picked)
    for (const r of rows) {
      if (allShownPicked) next.delete(r.id)
      else next.add(r.id)
    }
    setPicked(next)
    changed()
  }

  return (
    <Page
      title="재직중 경력 일괄 종료"
      description="학년도 말 계약 종료를 한꺼번에 처리합니다. 담당자가 고른 경력만 바뀝니다."
      actions={
        <Button icon={ArrowLeft} onClick={() => navigate('/instructors')}>
          강사관리로
        </Button>
      }
    >
      {done && (
        <Notice tone="success">
          재직중 경력 {done.ended}건을 종료했습니다. 적용 직전 자료는 ‘{done.backupFile}’ 로 백업해 두었습니다.
        </Notice>
      )}

      {/* ---------- 1. 대상 고르기 ---------- */}
      <Card
        title={`1. 종료할 경력 고르기 — ${picked.size}건 선택`}
        description="처음에는 아무것도 선택되어 있지 않습니다. 끝낼 경력만 직접 체크하세요."
        actions={
          <Input
            className={s.filter}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="이름 · 메모 · 프로그램으로 좁히기"
          />
        }
      >
        <ErrorNotice error={candidates.error} />
        {candidates.data?.length === 0 ? (
          <p className={s.muted}>재직중인 경력이 없습니다.</p>
        ) : (
          <div className={s.tableWrap}>
            <table className={s.table}>
              <thead>
                <tr>
                  <th className={s.colCheck}>
                    <input
                      type="checkbox"
                      checked={allShownPicked}
                      onChange={toggleShown}
                      aria-label="보이는 경력 모두 선택"
                      disabled={rows.length === 0}
                    />
                  </th>
                  <th>강사</th>
                  <th>프로그램명</th>
                  <th>기간</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((c) => (
                  <tr key={c.id} className={picked.has(c.id) ? s.rowOn : undefined} onClick={() => toggle(c.id)}>
                    <td className={s.colCheck}>
                      <input type="checkbox" checked={picked.has(c.id)} onChange={() => toggle(c.id)} onClick={(e) => e.stopPropagation()} />
                    </td>
                    <td>{whoLabel({ name: c.instructorName, distinguisher: c.distinguisher })}</td>
                    <td>{c.programName}</td>
                    <td className={s.period}>{c.period}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {/* ---------- 2. 종료일 · 사유 ---------- */}
      <Card title="2. 종료일과 종료 사유">
        <div className={s.settings}>
          <Field label="종료일">
            <Input
              type="date"
              value={endDate}
              onChange={(e) => {
                setEndDate(e.target.value)
                changed()
              }}
            />
          </Field>
          <Field label="종료 사유">
            <div className={s.reasons}>
              {END_REASONS.map((r) => (
                <label key={r.value} className={s.radio}>
                  <input
                    type="radio"
                    name="bulk-reason"
                    checked={reason === r.value}
                    onChange={() => {
                      setReason(r.value)
                      changed()
                    }}
                  />
                  {r.label}
                </label>
              ))}
            </div>
          </Field>
          <Button
            variant="primary"
            icon={Eye}
            onClick={() => previewRun.mutate()}
            disabled={picked.size === 0 || !endDate || previewRun.isPending}
          >
            변경내용 미리보기
          </Button>
        </div>
        <ErrorNotice error={previewRun.error} />
      </Card>

      {/* ---------- 3. 미리보기 · 적용 ---------- */}
      {preview && (
        <Card
          title={`3. 변경내용 미리보기 — ${preview.items.length}건`}
          description={`종료일 ${preview.endDateLabel} · ${preview.reasonLabel}`}
        >
          {preview.future && (
            <Notice tone="warn">종료일이 오늘 이후입니다({preview.endDateLabel}). 미리 정해진 계약 종료일이 맞는지 확인하세요.</Notice>
          )}
          {!preview.ok && (
            <Notice tone="error">
              처리할 수 없는 경력이 {preview.problems}건 있습니다. 이대로는 적용할 수 없습니다 — 해당 경력의 선택을 풀고 다시 미리보기 하세요.
            </Notice>
          )}
          <div className={s.tableWrap}>
            <table className={s.table}>
              <thead>
                <tr>
                  <th>강사</th>
                  <th>프로그램명</th>
                  <th>변경 전</th>
                  <th aria-label="→" />
                  <th>변경 후</th>
                </tr>
              </thead>
              <tbody>
                {preview.items.map((i) => (
                  <tr key={i.careerId} className={i.problem ? s.rowBad : undefined}>
                    <td>{i.instructorName ? whoLabel({ name: i.instructorName, distinguisher: i.distinguisher }) : '—'}</td>
                    <td>{i.programName}</td>
                    <td className={s.period}>{i.before}</td>
                    <td className={s.arrow}>
                      <ArrowRight size={14} />
                    </td>
                    <td className={s.period}>
                      {i.after ?? <Badge tone="error">{i.problem}</Badge>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <ErrorNotice error={applyRun.error} />
          <div className={s.applyRow}>
            <Button
              variant="primary"
              icon={CalendarCheck}
              onClick={() => applyRun.mutate()}
              disabled={!preview.ok || applyRun.isPending}
            >
              {applyRun.isPending ? '적용 중…' : `${preview.items.length}건 종료 적용`}
            </Button>
          </div>
        </Card>
      )}
      {applyRun.error && !preview ? <ErrorNotice error={applyRun.error} /> : null}
      {confirmDialog}
    </Page>
  )
}
