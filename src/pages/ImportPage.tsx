import { useMemo, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { ArrowLeft, Eye, FileSpreadsheet, Upload } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Input, Notice, Page, Select } from '@/components/ui'
import { invalidateCareerData } from '@/features/instructors/invalidate'
import {
  importApi,
  type GroupDecision,
  type ImportAnalysis,
  type ImportApplied,
  type ImportDecisions,
  type ImportGroup,
  type ImportPlan,
  type Outcome,
  type RowStatus,
} from '@/ipc/data'
import s from './ImportPage.module.css'

const STATUS: Record<RowStatus, { label: string; tone: 'success' | 'warn' | 'info' | 'error' | 'neutral' }> = {
  OK: { label: '정상', tone: 'success' },
  WARN: { label: '경고', tone: 'warn' },
  REVIEW: { label: '검토 필요', tone: 'info' },
  ERROR: { label: '오류', tone: 'error' },
  DUPLICATE: { label: '파일 안 중복', tone: 'neutral' },
}

const OUTCOME: Record<Outcome, string> = {
  ADD: '가져옴',
  SKIP_ERROR: '오류 — 건너뜀',
  SKIP_DUPLICATE_FILE: '파일 안 중복 — 건너뜀',
  SKIP_DUPLICATE_DB: '이미 있는 경력 — 건너뜀',
  SKIP_REVIEW: '검토 줄 — 고르지 않음',
  SKIP_EXCLUDED: '뺀 줄',
  SKIP_GROUP: '가져오지 않는 강사',
  PENDING_DECISION: '강사 선택 필요',
}

/** 처음 결정: 같은 이름의 기존 강사가 없으면 새 강사, 있으면 **고르지 않은 상태**(자동 연결하지 않는다) */
function initialDecisions(a: ImportAnalysis): ImportDecisions {
  return {
    groups: a.groups.map((g) => ({
      name: g.name,
      action: g.candidates.length === 0 ? 'NEW' : null,
      instructorId: null,
      distinguisher: '',
    })),
    reviewActive: [],
    excludedRows: [],
  }
}

/**
 * 엑셀 가져오기 — 파일 고르기 → 읽기 전용 분석(미리보기) → 강사 연결·검토 줄 선택 → 반영 미리보기 → 가져오기.
 * 강사와 경력만 가져온다(과거 발급 기록은 가져오지 않는다). 엑셀 매크로는 실행하지 않는다.
 */
export function ImportPage() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const [analysis, setAnalysis] = useState<ImportAnalysis | null>(null)
  const [decisions, setDecisions] = useState<ImportDecisions | null>(null)
  const [plan, setPlan] = useState<ImportPlan | null>(null)
  const [applied, setApplied] = useState<ImportApplied | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [onlyProblems, setOnlyProblems] = useState(false)

  const load = async (sheet: string | null) => {
    setBusy('파일을 읽는 중…')
    setError(null)
    try {
      const a = await importApi.open(sheet)
      if (a) {
        setAnalysis(a)
        setDecisions(initialDecisions(a))
        setPlan(null)
        setApplied(null)
      }
    } catch (e) {
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  const change = (next: ImportDecisions) => {
    setDecisions(next)
    setPlan(null) // 결정이 바뀌면 반영 미리보기를 다시
  }

  const runPlan = async () => {
    if (!analysis || !decisions) return
    setBusy('반영할 내용을 계산하는 중…')
    setError(null)
    try {
      setPlan(await importApi.plan(analysis.token, decisions))
    } catch (e) {
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  const runApply = async () => {
    if (!analysis || !decisions || !plan) return
    setBusy('가져오는 중… (먼저 지금 자료를 백업합니다)')
    setError(null)
    try {
      const r = await importApi.apply(analysis.token, decisions, plan.fingerprint)
      setApplied(r)
      setAnalysis(null)
      setDecisions(null)
      setPlan(null)
      invalidateCareerData(qc)
    } catch (e) {
      setError(e)
      setPlan(null)
    } finally {
      setBusy(null)
    }
  }

  const back = () => {
    void importApi.cancel()
    navigate('/instructors')
  }

  return (
    <Page
      title="엑셀 가져오기"
      description="기존 엑셀 강사명단에서 강사와 경력을 가져옵니다. 미리보기를 확인한 뒤에만 반영합니다."
      actions={
        <Button icon={ArrowLeft} onClick={back}>
          강사관리로
        </Button>
      }
    >
      {applied && (
        <Notice tone="success">
          가져왔습니다 — 새 강사 {applied.summary.instructorsCreated}명 · 기존 강사 연결 {applied.summary.instructorsLinked}명 · 경력{' '}
          {applied.summary.careersAdded}건 (경고 {applied.summary.rowsWarned}건 포함) · 건너뜀 {applied.summary.rowsSkipped}줄. 가져오기 직전 자료는 앱 안에
          백업(before_import)해 두었습니다.
        </Notice>
      )}
      {!analysis && (
        <Card title="파일 고르기">
          <ul className={s.notes}>
            <li>.xlsx · .xlsm 파일을 읽기만 합니다. 엑셀 매크로(VBA)는 실행하지 않습니다.</li>
            <li>
              <b>강사와 경력만</b> 가져옵니다. 엑셀에 남아 있는 과거 발급번호·발급 기록은 가져오지 않습니다.
            </li>
            <li>성명·프로그램명·직위·시작날짜·마감날짜·지도사항 열만 읽습니다. 주민등록번호·주소 같은 다른 열은 읽어 두지 않습니다.</li>
            <li>같은 이름의 강사가 이미 있으면 자동으로 합치지 않습니다. 한 명씩 고르게 합니다.</li>
          </ul>
          <div>
            <Button variant="primary" icon={FileSpreadsheet} onClick={() => load(null)} disabled={!!busy}>
              엑셀 파일 고르기
            </Button>
          </div>
        </Card>
      )}
      {busy && <Notice tone="info">{busy}</Notice>}
      <ErrorNotice error={error} />

      {analysis && decisions && (
        <>
          <Structure analysis={analysis} onSheet={(name) => load(name)} disabled={!!busy} />
          <Groups analysis={analysis} decisions={decisions} onChange={change} />
          <Rows
            analysis={analysis}
            decisions={decisions}
            plan={plan}
            onlyProblems={onlyProblems}
            setOnlyProblems={setOnlyProblems}
            onChange={change}
          />
          <Card title="반영">
            {!plan && <p className={s.muted}>[반영 미리보기] 로 실제로 넣을 건수를 확인한 뒤에 가져올 수 있습니다.</p>}
            {plan && (
              <>
                <dl className={s.summary}>
                  <dt>새 강사</dt>
                  <dd>{plan.summary.instructorsCreated}명</dd>
                  <dt>기존 강사에 연결</dt>
                  <dd>{plan.summary.instructorsLinked}명</dd>
                  <dt>가져올 경력</dt>
                  <dd>
                    {plan.summary.careersAdded}건 <span className={s.muted}>(그중 경고·검토 {plan.summary.rowsWarned}건)</span>
                  </dd>
                  <dt>건너뛸 줄</dt>
                  <dd>{plan.summary.rowsSkipped}줄</dd>
                </dl>
                {plan.problems.length > 0 && (
                  <Notice tone="warn">
                    <ul className={s.problems}>
                      {plan.problems.map((p) => (
                        <li key={p}>{p}</li>
                      ))}
                    </ul>
                  </Notice>
                )}
              </>
            )}
            <div className={s.actions}>
              <Button icon={Eye} onClick={runPlan} disabled={!!busy}>
                반영 미리보기
              </Button>
              <Button variant="primary" icon={Upload} onClick={runApply} disabled={!!busy || !plan || plan.problems.length > 0}>
                가져오기
              </Button>
            </div>
          </Card>
        </>
      )}
    </Page>
  )
}

function Structure({ analysis: a, onSheet, disabled }: { analysis: ImportAnalysis; onSheet: (name: string) => void; disabled: boolean }) {
  const counts = useMemo(() => {
    const c: Record<RowStatus, number> = { OK: 0, WARN: 0, REVIEW: 0, ERROR: 0, DUPLICATE: 0 }
    a.rows.forEach((r) => c[r.status]++)
    return c
  }, [a])
  return (
    <Card title={`분석 — ${a.fileName}`} description="파일을 읽기만 했습니다. 아직 아무것도 반영하지 않았습니다.">
      <dl className={s.summary}>
        <dt>시트</dt>
        <dd>
          <Select value={a.sheet} onChange={(e) => onSheet(e.target.value)} disabled={disabled} className={s.sheet}>
            {a.sheets.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </Select>{' '}
          <span className={s.muted}>머리글 {a.headerRow}행</span>
        </dd>
        <dt>읽은 열</dt>
        <dd>
          {a.columns.map((c) => (
            <span key={c.field} className={s.col}>
              {c.label} ← {c.column}열 "{c.header}"
            </span>
          ))}
        </dd>
        <dt>줄</dt>
        <dd>
          {a.rows.length}줄 —{' '}
          {(Object.keys(counts) as RowStatus[])
            .filter((k) => counts[k] > 0)
            .map((k) => `${STATUS[k].label} ${counts[k]}`)
            .join(' · ')}
        </dd>
        <dt>날짜 읽은 방식</dt>
        <dd>{a.formats.map(([k, n]) => `${k} ${n}`).join(' · ') || '—'}</dd>
      </dl>
    </Card>
  )
}

function Groups({ analysis: a, decisions: d, onChange }: { analysis: ImportAnalysis; decisions: ImportDecisions; onChange: (d: ImportDecisions) => void }) {
  const set = (name: string, patch: Partial<GroupDecision>) =>
    onChange({ ...d, groups: d.groups.map((g) => (g.name === name ? { ...g, ...patch } : g)) })
  const withCandidates = a.groups.filter((g) => g.candidates.length > 0).length
  return (
    <Card
      title={`강사 ${a.groups.length}명`}
      description={
        withCandidates > 0
          ? `같은 이름의 기존 강사가 있는 ${withCandidates}명은 자동으로 합치지 않았습니다. 기존 강사와 연결할지, 새 강사로 등록할지 골라 주세요.`
          : '기존 강사와 이름이 같은 사람이 없어 모두 새 강사로 등록합니다. 가져오지 않을 사람은 바꾸세요.'
      }
    >
      <div className={s.groups}>
        {a.groups.map((g) => (
          <GroupCard key={g.name} group={g} decision={d.groups.find((x) => x.name === g.name)!} onChange={(p) => set(g.name, p)} />
        ))}
      </div>
    </Card>
  )
}

function GroupCard({ group: g, decision: dec, onChange }: { group: ImportGroup; decision: GroupDecision; onChange: (p: Partial<GroupDecision>) => void }) {
  const radio = `g-${g.name}`
  const needsDist = dec.action === 'NEW' && g.candidates.length > 0
  return (
    <div className={dec.action == null ? s.groupTodo : s.group}>
      <div className={s.groupHead}>
        <b>{g.name}</b>
        <span className={s.muted}>
          엑셀 {g.rowNos.length}줄{g.phones.length > 0 ? ` · 엑셀 연락처 ${g.phones.join(', ')}` : ''}
        </span>
        {dec.action == null && <Badge tone="warn">선택 필요</Badge>}
      </div>
      {g.candidates.map((c) => (
        <label key={c.id} className={s.option}>
          <input
            type="radio"
            name={radio}
            checked={dec.action === 'LINK' && dec.instructorId === c.id}
            onChange={() => onChange({ action: 'LINK', instructorId: c.id })}
          />
          기존 강사와 연결 — {g.name}
          {c.distinguisher ? ` (${c.distinguisher})` : ' (구분 메모 없음)'}
          <span className={s.muted}>
            {c.phone ? ` · 연락처 ${c.phone}` : ''} · 경력 {c.careers}건{c.programs.length ? ` · ${c.programs.join(', ')}` : ''}
          </span>
        </label>
      ))}
      <label className={s.option}>
        <input type="radio" name={radio} checked={dec.action === 'NEW'} onChange={() => onChange({ action: 'NEW', instructorId: null })} />
        새 강사로 등록
        {dec.action === 'NEW' && (
          <Input
            className={s.dist}
            value={dec.distinguisher}
            placeholder={needsDist ? '구분 메모 (필수 — 예: 1985년생)' : '구분 메모 (선택)'}
            onChange={(e) => onChange({ distinguisher: e.target.value })}
          />
        )}
      </label>
      <label className={s.option}>
        <input type="radio" name={radio} checked={dec.action === 'SKIP'} onChange={() => onChange({ action: 'SKIP', instructorId: null })} />
        가져오지 않음
      </label>
    </div>
  )
}

function Rows({
  analysis: a,
  decisions: d,
  plan,
  onlyProblems,
  setOnlyProblems,
  onChange,
}: {
  analysis: ImportAnalysis
  decisions: ImportDecisions
  plan: ImportPlan | null
  onlyProblems: boolean
  setOnlyProblems: (v: boolean) => void
  onChange: (d: ImportDecisions) => void
}) {
  const outcomes = new Map(plan?.outcomes ?? [])
  const rows = onlyProblems ? a.rows.filter((r) => r.status !== 'OK') : a.rows
  const toggleList = (list: number[], no: number, on: boolean) => (on ? [...list.filter((x) => x !== no), no] : list.filter((x) => x !== no))
  return (
    <Card
      title="경력 줄"
      actions={
        <label className={s.filter}>
          <input type="checkbox" checked={onlyProblems} onChange={(e) => setOnlyProblems(e.target.checked)} /> 확인할 줄만
        </label>
      }
    >
      <div className={s.tableWrap}>
        <table className={s.table}>
          <thead>
            <tr>
              <th>행</th>
              <th>가져오기</th>
              <th>성명</th>
              <th>프로그램명</th>
              <th>직위</th>
              <th>시작</th>
              <th>마감</th>
              <th>지도사항</th>
              <th>판정</th>
              {plan && <th>반영</th>}
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const importable = r.status === 'OK' || r.status === 'WARN'
              return (
                <tr key={r.rowNo} className={r.status === 'ERROR' ? s.rowError : undefined}>
                  <td className={s.num}>{r.rowNo}</td>
                  <td>
                    {importable && (
                      <input
                        type="checkbox"
                        checked={!d.excludedRows.includes(r.rowNo)}
                        onChange={(e) => onChange({ ...d, excludedRows: toggleList(d.excludedRows, r.rowNo, !e.target.checked) })}
                        title="이 줄을 가져올지"
                      />
                    )}
                    {r.status === 'REVIEW' && (
                      <label className={s.reviewPick}>
                        <input
                          type="checkbox"
                          checked={d.reviewActive.includes(r.rowNo)}
                          onChange={(e) => onChange({ ...d, reviewActive: toggleList(d.reviewActive, r.rowNo, e.target.checked) })}
                        />
                        재직중으로
                      </label>
                    )}
                  </td>
                  <td>{r.name}</td>
                  <td>{r.program}</td>
                  <td>{r.position}</td>
                  <td className={s.date}>
                    {r.startLabel || r.startText}
                    {r.startText && r.startLabel && r.startText !== r.startLabel && <span className={s.raw}>{r.startText}</span>}
                  </td>
                  <td className={s.date}>
                    {r.endLabel || r.endText || <span className={s.muted}>(빈칸)</span>}
                    {r.endText && r.endLabel && r.endText !== r.endLabel && <span className={s.raw}>{r.endText}</span>}
                  </td>
                  <td>{r.duty}</td>
                  <td>
                    <Badge tone={STATUS[r.status].tone}>{STATUS[r.status].label}</Badge>
                    {r.messages.map((m) => (
                      <div key={m} className={s.msg}>
                        {m}
                      </div>
                    ))}
                  </td>
                  {plan && <td className={s.outcome}>{outcomes.has(r.rowNo) ? OUTCOME[outcomes.get(r.rowNo)!] : ''}</td>}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </Card>
  )
}
