import { lazy, Suspense, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { Eye, FileCheck2, Search, Settings, UserRoundSearch } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Field, Input, Notice, Page } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { blockers, initialDraft, isSelected, selectedIds, selectInstructor, setField, toggle, type Defaults, type Draft, type IssueFields } from '@/features/issue/draft'
import { whoLabel } from '@/features/instructors/label'
import { careerApi } from '@/ipc/career'
import { certificateApi, type Prepared } from '@/ipc/certificate'
import { instructorApi, type InstructorRow } from '@/ipc/instructor'
import { settingsApi } from '@/ipc/settings'
import s from './IssuePage.module.css'

// pdf.js 는 미리보기를 열 때만 읽는다 (첫 화면을 무겁게 하지 않으려고)
const PdfPreview = lazy(() => import('@/features/issue/PdfPreview').then((m) => ({ default: m.PdfPreview })))

/**
 * 증명서 작성 (Phase 4) — 강사 선택 → 경력 선택 → 발급 정보 → [증명서 내용 확인].
 *
 * 개인정보
 *   * 작성 중인 값(주민번호·주소 포함)은 이 화면의 React state 에만 있다. 화면을 떠나면 사라진다.
 *     DB·localStorage·sessionStorage·React Query 캐시 어디에도 두지 않는다
 *     (내용 확인 요청은 useMutation 이 아니라 직접 부른다 — mutation 캐시가 요청 값을 들고 있으므로).
 *   * 강사를 바꾸면 주민번호·주소·발급번호를 비운다(`draft.selectInstructor`).
 *
 * 발급 확정(기록 저장)은 Phase 6, 출력(미리보기·인쇄·PDF)은 Phase 5.
 */
export function IssuePage() {
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const hints = useQuery({ queryKey: ['career-hints'], queryFn: () => careerApi.hints(''), staleTime: Infinity })
  if (settings.error || hints.error) {
    return (
      <Page title="증명서 발급">
        <ErrorNotice error={settings.error ?? hints.error} />
      </Page>
    )
  }
  if (!settings.data || !hints.data) return <Page title="증명서 발급">{null}</Page>
  const defaults: Defaults = { purpose: settings.data.settings.defaultPurpose, today: hints.data.today }
  return <IssueForm defaults={defaults} missingSettings={settings.data.missing.map((m) => m.label)} />
}

function IssueForm({ defaults, missingSettings }: { defaults: Defaults; missingSettings: string[] }) {
  const navigate = useNavigate()
  const [confirm, confirmDialog] = useConfirm()
  const [draft, setDraft] = useState<Draft>(() => initialDraft(defaults))
  const [picking, setPicking] = useState(true)
  const [prepared, setPrepared] = useState<Prepared | null>(null)
  const [prepareError, setPrepareError] = useState<unknown>(null)
  const [preparing, setPreparing] = useState(false)
  // 실제 출력 미리보기 PDF — 화면 메모리에만. 닫거나 무엇이든 바꾸면 버린다
  const [previewPdf, setPreviewPdf] = useState<ArrayBuffer | null>(null)
  const [previewing, setPreviewing] = useState(false)
  const [previewError, setPreviewError] = useState<unknown>(null)

  const choices = useQuery({
    queryKey: ['cert-choices', draft.instructorId, draft.fields.issuedOn],
    queryFn: () => certificateApi.choices(draft.instructorId!, draft.fields.issuedOn),
    enabled: draft.instructorId != null && !!draft.fields.issuedOn,
  })
  const list = choices.data ?? []
  const chosen = selectedIds(draft, list)
  const blocked = blockers(draft, list)

  /** 무엇이든 바뀌면 이전 '내용 확인' 결과는 버린다 — 본 것과 다른 내용으로 출력되지 않게 */
  const update = (next: Draft) => {
    setDraft(next)
    setPrepared(null)
    setPrepareError(null)
    setPreviewPdf(null)
    setPreviewError(null)
  }

  const runPreview = async () => {
    setPreviewing(true)
    setPreviewError(null)
    try {
      setPreviewPdf(await certificateApi.preview({ instructorId: draft.instructorId!, careerIds: chosen, issue: draft.fields }))
    } catch (e) {
      setPreviewError(e)
    } finally {
      setPreviewing(false)
    }
  }
  const set = (k: Exclude<keyof IssueFields, 'maskRrn'>, v: string) => update(setField(draft, k, v))

  const hasTyped = !!(draft.fields.rrn || draft.fields.address || draft.fields.issueNo)

  const pick = (row: InstructorRow) => {
    update(selectInstructor(draft, row.id, defaults))
    setPicking(false)
  }

  const changeInstructor = async () => {
    if (hasTyped) {
      const ok = await confirm({
        title: '다른 강사 선택',
        message: '입력한 주민등록번호·주소·발급번호가 지워집니다. 다른 강사를 고를까요?',
        confirmLabel: '지우고 다른 강사 선택',
      })
      if (!ok) return
    }
    update(selectInstructor(draft, null, defaults))
    setPicking(true)
  }

  const runPrepare = async () => {
    setPreparing(true)
    setPrepareError(null)
    try {
      const r = await certificateApi.prepare({
        instructorId: draft.instructorId!,
        careerIds: chosen,
        issue: draft.fields,
      })
      setPrepared(r)
    } catch (e) {
      setPrepared(null)
      setPrepareError(e)
    } finally {
      setPreparing(false)
    }
  }

  return (
    <Page title="증명서 발급" description="강사 선택 → 경력 선택 → 발급 정보 → 내용 확인">
      {missingSettings.length > 0 && (
        <Notice tone="warn">
          <span className={s.inline}>
            학교 기본정보가 비어 있어 증명서를 만들 수 없습니다: {missingSettings.join(', ')}
            <Button size="sm" variant="primary" icon={Settings} onClick={() => navigate('/settings')}>
              설정으로 이동
            </Button>
          </span>
        </Notice>
      )}

      {/* ---------- 1. 강사 ---------- */}
      <Card title="1. 강사 선택">
        {picking || draft.instructorId == null ? (
          <InstructorPicker onPick={pick} />
        ) : (
          <SelectedInstructor id={draft.instructorId} onChange={changeInstructor} />
        )}
      </Card>

      {draft.instructorId != null && !picking && (
        <>
          {/* ---------- 2. 경력 ---------- */}
          <Card
            title={`2. 경력 선택 — ${chosen.length}건 선택`}
            description="보관하지 않은 경력이 모두 선택되어 있습니다. 증명서에 넣지 않을 경력만 체크를 푸세요. 기간은 발급일 기준으로 표시합니다."
          >
            <ErrorNotice error={choices.error} />
            {choices.data?.length === 0 && <p className={s.muted}>이 강사에게 등록된 경력이 없습니다. 강사관리에서 먼저 넣어 주세요.</p>}
            {choices.data && choices.data.length > 0 && (
              <div className={s.tableWrap}>
                <table className={s.table}>
                  <thead>
                    <tr>
                      <th className={s.colCheck} aria-label="선택" />
                      <th>증명서 기간</th>
                      <th>프로그램명</th>
                      <th>직위</th>
                      <th>지도사항</th>
                      <th>현재 상태</th>
                    </tr>
                  </thead>
                  <tbody>
                    {list.map((c) => {
                      const on = isSelected(draft, c)
                      return (
                        <tr
                          key={c.id}
                          className={!c.eligible ? s.rowOff : on ? s.rowOn : undefined}
                          onClick={() => c.eligible && update(toggle(draft, c))}
                        >
                          <td className={s.colCheck}>
                            <input
                              type="checkbox"
                              checked={on}
                              disabled={!c.eligible}
                              onChange={() => update(toggle(draft, c))}
                              onClick={(e) => e.stopPropagation()}
                              aria-label={`${c.programName} ${c.period} 선택`}
                            />
                          </td>
                          <td className={s.period}>
                            {c.eligible ? (
                              <>
                                <span className={s.nowrap}>{c.onIssueFrom} ~</span> <span className={s.nowrap}>{c.onIssueTo}</span>
                              </>
                            ) : (
                              <span className={s.reason}>{c.reason}</span>
                            )}
                          </td>
                          <td>{c.programName}</td>
                          <td className={s.nowrap}>{c.position}</td>
                          <td>{c.duty}</td>
                          <td className={s.state}>
                            <Badge tone={c.status === 'ACTIVE' ? 'success' : 'neutral'}>{c.statusLabel}</Badge>
                            <span className={s.sub}>{c.period}</span>
                            {c.plannedEndLabel && <span className={s.sub}>예정 종료일 {c.plannedEndLabel} (참고)</span>}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </Card>

          {/* ---------- 3. 발급 정보 ---------- */}
          <Card
            title="3. 발급 정보"
            description="주민등록번호·주소는 강사 정보에 저장되지 않습니다. 이 화면을 떠나거나 강사를 바꾸면 지워집니다."
          >
            <form className={s.grid} autoComplete="off" onSubmit={(e) => e.preventDefault()}>
              <Field label="주민등록번호" hint="숫자 13자리 (하이픈은 있어도 없어도 됩니다)">
                <Input
                  value={draft.fields.rrn}
                  onChange={(e) => set('rrn', e.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                  inputMode="numeric"
                  maxLength={16}
                  placeholder="000000-0000000"
                />
              </Field>
              <Field label="발급번호" hint="NEIS 민원 등록 후 받은 번호를 그대로 적습니다. 앞뒤 공백만 정리하고 바꾸지 않습니다.">
                <Input value={draft.fields.issueNo} onChange={(e) => set('issueNo', e.target.value)} autoComplete="off" spellCheck={false} />
              </Field>
              <Field label="주소" className={s.wide}>
                <Input value={draft.fields.address} onChange={(e) => set('address', e.target.value)} autoComplete="off" spellCheck={false} />
              </Field>
              <Field label="용도" hint={`기본값: ${defaults.purpose}`}>
                <Input value={draft.fields.purpose} onChange={(e) => set('purpose', e.target.value)} autoComplete="off" />
              </Field>
              <Field label="발급일" hint="기본값은 오늘입니다. 미래 날짜는 쓸 수 없습니다. 바꾸면 경력 기간과 선택할 수 있는 경력이 다시 계산됩니다.">
                <Input type="date" value={draft.fields.issuedOn} max={defaults.today} onChange={(e) => set('issuedOn', e.target.value)} />
              </Field>
              <label className={s.maskRow}>
                <input
                  type="checkbox"
                  checked={draft.fields.maskRrn}
                  onChange={(e) => update(setField(draft, 'maskRrn', e.target.checked))}
                />
                <span>
                  <b>주민등록번호 뒷자리 가림</b>
                  <span className={s.sub}>켜면 증명서에 000000-0****** 처럼 뒷자리 여섯 자리를 가려 찍습니다. 입력한 번호는 바뀌지 않습니다.</span>
                </span>
              </label>
            </form>
          </Card>

          {/* ---------- 내용 확인 ---------- */}
          <div className={s.actions}>
            {blocked.length > 0 && <span className={s.blocked}>{blocked.join(' ')}</span>}
            <Button variant="primary" icon={FileCheck2} onClick={runPrepare} disabled={blocked.length > 0 || preparing}>
              {preparing ? '확인 중…' : '증명서 내용 확인'}
            </Button>
          </div>
          <ErrorNotice error={prepareError} />
          {prepared && (
            <>
              <PreparedView prepared={prepared} />
              <div className={s.actions}>
                <span className={s.blocked}>인쇄·PDF 저장은 발급 확정 후에 할 수 있습니다.</span>
                <Button variant="primary" icon={Eye} onClick={runPreview} disabled={previewing}>
                  {previewing ? 'PDF 만드는 중…' : '실제 출력 미리보기'}
                </Button>
              </div>
              <ErrorNotice error={previewError} />
            </>
          )}
          {previewPdf && (
            <Suspense fallback={null}>
              <PdfPreview pdf={previewPdf} onClose={() => setPreviewPdf(null)} />
            </Suspense>
          )}
        </>
      )}
      {confirmDialog}
    </Page>
  )
}

function InstructorPicker({ onPick }: { onPick: (row: InstructorRow) => void }) {
  const [query, setQuery] = useState('')
  // 보관한 강사는 고를 수 없다 — 'ALL' 은 보관 제외 전체다
  const list = useQuery({
    queryKey: ['instructors', query.trim(), 'ALL'],
    queryFn: () => instructorApi.search(query.trim(), 'ALL'),
  })
  return (
    <div className={s.picker}>
      <div className={s.searchBox}>
        <Search size={16} className={s.searchIcon} />
        <Input className={s.searchInput} value={query} onChange={(e) => setQuery(e.target.value)} placeholder="이름 · 구분 메모로 찾기" autoFocus />
      </div>
      <ErrorNotice error={list.error} />
      <div className={s.pickList}>
        {list.data?.length === 0 && <p className={s.muted}>찾는 강사가 없습니다.</p>}
        {list.data?.map((r) => (
          <button key={r.id} type="button" className={s.pickRow} onClick={() => onPick(r)}>
            <span className={s.pickName}>{r.name}</span>
            {r.distinguisher ? (
              <span className={r.sameName ? s.distStrong : s.dist}>{r.distinguisher}</span>
            ) : (
              r.sameName && <Badge tone="warn">동명이인 · 구분 메모 없음</Badge>
            )}
            <span className={s.pickMeta}>
              {r.programs.length > 0 ? r.programs.join(', ') : '경력 없음'} · 경력 {r.careers}건
              {r.activeCareers > 0 ? ` · 재직중 ${r.activeCareers}건` : ''}
            </span>
          </button>
        ))}
      </div>
    </div>
  )
}

function SelectedInstructor({ id, onChange }: { id: number; onChange: () => void }) {
  const who = useQuery({ queryKey: ['instructor', id], queryFn: () => instructorApi.get(id) })
  return (
    <div className={s.selected}>
      <span className={s.selectedName}>{who.data ? whoLabel(who.data) : '…'}</span>
      {who.data?.archived && <Badge tone="error">보관된 강사</Badge>}
      <span className={s.spacer} />
      <Button icon={UserRoundSearch} onClick={onChange}>
        다른 강사 선택
      </Button>
    </div>
  )
}

/** 증명서에 들어갈 내용을 한 번 더 보여 준다. 실제 양식(A4) 미리보기·출력은 Phase 5. */
function PreparedView({ prepared }: { prepared: Prepared }) {
  const d = prepared.doc
  return (
    <Card title="증명서 내용 확인" description="아래 내용으로 증명서가 만들어집니다. 틀린 곳이 있으면 위에서 고친 뒤 다시 확인하세요.">
      {prepared.warnings.map((w, i) => (
        <Notice key={`${w.code}-${i}`} tone="warn">
          {w.message}
        </Notice>
      ))}
      <dl className={s.doc}>
        <dt>문서 제목</dt>
        <dd className={s.docTitle}>{d.title}</dd>
        <dt>발급번호</dt>
        <dd>{d.issueNo}</dd>
        <dt>성명</dt>
        <dd>{d.holderName}</dd>
        <dt>주민등록번호</dt>
        <dd className="selectable">
          {d.holderRrnShown}
          {d.rrnMasked && <span className={s.sub}>증명서에 뒷자리를 가려 찍습니다 (입력한 번호: {d.holderRrn})</span>}
        </dd>
        <dt>주소</dt>
        <dd className="selectable">{d.holderAddress}</dd>
      </dl>
      <div className={s.tableWrap}>
        <table className={s.table}>
          <thead>
            <tr>
              <th>부터</th>
              <th>까지</th>
              <th>프로그램명</th>
              <th>직위</th>
              <th>지도사항</th>
            </tr>
          </thead>
          <tbody>
            {d.items.map((i) => (
              <tr key={i.careerId}>
                <td className={s.period}>{i.from}</td>
                <td className={s.period}>{i.to}</td>
                <td>{i.programName}</td>
                <td>{i.position}</td>
                <td>{i.duty}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <dl className={s.doc}>
        <dt>용도</dt>
        <dd>{d.purpose}</dd>
        <dt>발급일</dt>
        <dd>{d.issuedOnLabel}</dd>
        <dt>발급자</dt>
        <dd>{d.issuerTitle} (직인)</dd>
        <dt>담당</dt>
        <dd>
          {d.department} · {d.managerName} (인) · {d.phone}
        </dd>
      </dl>
      <Notice tone="success">내용 확인을 마쳤습니다. [실제 출력 미리보기] 로 증명서 모양을 확인할 수 있습니다.</Notice>
    </Card>
  )
}
