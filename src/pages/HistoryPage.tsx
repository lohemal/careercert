import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { ArrowLeft, RotateCcw, Search } from 'lucide-react'

import { Badge, Button, ErrorNotice, Input, Notice, Page } from '@/components/ui'
import { IssuedPanel, rrnDisplayLabel } from '@/features/issued/IssuedPanel'
import { historyApi, issuanceApi, type HistoryFilter, type HistoryRow, type HistoryStatus, type Issued } from '@/ipc/issuance'
import s from './HistoryPage.module.css'

const STATUSES: { value: HistoryStatus; label: string }[] = [
  { value: 'ALL', label: '전체' },
  { value: 'ISSUED', label: '발급' },
  { value: 'VOIDED', label: '취소' },
]

const EMPTY: HistoryFilter = { query: '', from: null, to: null, status: 'ALL' }

/**
 * 발급이력 — 찾기(성명·발급번호, 발급일 기간, 상태) → 목록(최근 발급순) → 발급 기록 상세.
 *
 * 개인정보: 목록·상세 모두 발급 기록의 암호문을 풀지 않은 값만 받는다(주민번호는 상세에서 가린 값만, 주소 없음).
 * 찾기 조건에도 주민번호·주소는 없다. 재출력·PDF 저장 때만 Rust 안에서 복호화한다.
 */
export function HistoryPage() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const [params, setParams] = useSearchParams()
  const selectedId = Number(params.get('id')) || null
  const [filter, setFilter] = useState<HistoryFilter>(EMPTY)
  // 오발급 폐기 뒤 목록에서 한 번 보이는 알림
  const [discarded, setDiscarded] = useState<string | null>(null)
  const set = <K extends keyof HistoryFilter>(k: K, v: HistoryFilter[K]) => setFilter((f) => ({ ...f, [k]: v }))
  const badRange = !!(filter.from && filter.to && filter.from > filter.to)

  const query: HistoryFilter = { ...filter, query: filter.query.trim() }
  const list = useQuery({
    queryKey: ['history', query],
    queryFn: () => historyApi.list(query),
    enabled: !badRange && !selectedId,
  })

  const open = (id: number | null) => {
    setDiscarded(null)
    setParams(id ? { id: String(id) } : {}, { replace: false })
  }

  if (selectedId) {
    return (
      <Page
        title="발급이력"
        actions={
          <Button icon={ArrowLeft} onClick={() => open(null)}>
            목록으로
          </Button>
        }
      >
        <HistoryDetail
          key={selectedId}
          id={selectedId}
          onOpen={open}
          onCopy={() => navigate('/issue', { state: { copyFrom: selectedId } })}
          onChanged={() => {
            void qc.invalidateQueries({ queryKey: ['history'] })
            void qc.invalidateQueries({ queryKey: ['certificate-recent'] })
          }}
          onDiscarded={(issueNo) => {
            qc.removeQueries({ queryKey: ['certificate', selectedId] })
            void qc.invalidateQueries({ queryKey: ['history'] })
            void qc.invalidateQueries({ queryKey: ['certificate-recent'] })
            void qc.invalidateQueries({ queryKey: ['recovery'] })
            void qc.invalidateQueries({ queryKey: ['backup-reminder'] })
            setParams({}, { replace: true })
            setDiscarded(issueNo)
          }}
        />
      </Page>
    )
  }

  const filtered = filter.query.trim() || filter.from || filter.to || filter.status !== 'ALL'
  return (
    <Page title="발급이력" description="성명·발급번호로 찾고, 발급 당시 내용 그대로 다시 인쇄하거나 PDF 로 저장합니다.">
      {discarded && (
        <Notice tone="success">
          {discarded} 증명서 기록을 오발급 폐기했습니다. 이 발급번호로 새 증명서를 발급할 수 있습니다.
        </Notice>
      )}
      <div className={s.filters}>
        <div className={s.searchBox}>
          <Search size={16} className={s.searchIcon} />
          <Input
            className={s.searchInput}
            value={filter.query}
            onChange={(e) => set('query', e.target.value)}
            placeholder="성명 또는 발급번호"
            title="성명·발급번호의 일부로 찾습니다"
          />
        </div>
        <label className={s.dateField}>
          <span>발급일</span>
          <Input type="date" value={filter.from ?? ''} max={filter.to ?? undefined} onChange={(e) => set('from', e.target.value || null)} />
          <span>~</span>
          <Input type="date" value={filter.to ?? ''} min={filter.from ?? undefined} onChange={(e) => set('to', e.target.value || null)} />
        </label>
        <div className={s.statuses} role="tablist">
          {STATUSES.map((o) => (
            <button
              key={o.value}
              type="button"
              role="tab"
              aria-selected={filter.status === o.value}
              className={filter.status === o.value ? s.statusOn : s.status}
              onClick={() => set('status', o.value)}
            >
              {o.label}
            </button>
          ))}
        </div>
        {filtered && (
          <Button size="sm" variant="ghost" icon={RotateCcw} onClick={() => setFilter(EMPTY)}>
            조건 지우기
          </Button>
        )}
      </div>

      {badRange && <Notice tone="warn">발급일 기간의 시작일이 끝 날짜보다 늦습니다. 기간을 다시 골라 주세요.</Notice>}
      <ErrorNotice error={list.error} />
      {list.data?.truncated && (
        <Notice tone="info">최근 {list.data.limit}건만 보입니다. 성명·발급번호·기간으로 좁혀 찾아 주세요.</Notice>
      )}

      {list.data && !badRange && (
        <div className={s.tableWrap}>
          <table className={s.table}>
            <thead>
              <tr>
                <th>발급번호</th>
                <th>성명</th>
                <th>발급일</th>
                <th>용도</th>
                <th className={s.num}>경력</th>
                <th>주민번호 표시</th>
                <th>상태</th>
              </tr>
            </thead>
            <tbody>
              {list.data.rows.map((r) => (
                <Row key={r.id} row={r} onClick={() => open(r.id)} />
              ))}
            </tbody>
          </table>
          {list.data.rows.length === 0 && (
            <p className={s.empty}>{filtered ? '조건에 맞는 발급 기록이 없습니다.' : '아직 발급한 증명서가 없습니다.'}</p>
          )}
        </div>
      )}
      {list.data && list.data.rows.length > 0 && <p className={s.foot}>{list.data.rows.length}건 · 최근 발급순</p>}
    </Page>
  )
}

function Row({ row, onClick }: { row: HistoryRow; onClick: () => void }) {
  const voided = row.status === 'VOIDED'
  return (
    <tr className={voided ? s.rowVoided : s.row} onClick={onClick} tabIndex={0} onKeyDown={(e) => e.key === 'Enter' && onClick()}>
      <td className={s.issueNo}>{row.issueNo}</td>
      <td>{row.holderName}</td>
      <td className={s.nowrap}>{row.issuedOnLabel}</td>
      <td>{row.purpose}</td>
      <td className={s.num}>{row.itemCount}건</td>
      <td className={s.nowrap}>{rrnDisplayLabel(row.rrnDisplay)}</td>
      <td>{voided ? <Badge tone="error">취소</Badge> : <Badge tone="success">발급</Badge>}</td>
    </tr>
  )
}

function HistoryDetail({
  id,
  onOpen,
  onCopy,
  onChanged,
  onDiscarded,
}: {
  id: number
  onOpen: (id: number) => void
  onCopy: () => void
  onChanged: () => void
  onDiscarded: (issueNo: string) => void
}) {
  const qc = useQueryClient()
  const detail = useQuery({ queryKey: ['certificate', id], queryFn: () => issuanceApi.get(id), staleTime: 0 })
  const update = (r: Issued) => {
    qc.setQueryData(['certificate', id], r)
    onChanged()
  }
  if (detail.error) return <ErrorNotice error={detail.error} />
  if (!detail.data) return null
  const issueNo = detail.data.issueNo
  return (
    <IssuedPanel
      issued={detail.data}
      variant="history"
      onChange={update}
      onCopy={onCopy}
      onOpen={onOpen}
      onDiscarded={() => onDiscarded(issueNo)}
    />
  )
}
