import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Wand2 } from 'lucide-react'

import { Button, ErrorNotice, Field, Input, Modal, Select, Textarea } from '@/components/ui'
import { useConfirm, withFutureConfirm } from '@/components/useConfirm'
import {
  careerApi,
  END_REASONS,
  type Career,
  type CareerInput,
  type CareerStatus,
  type EndReason,
} from '@/ipc/career'
import { invalidateCareerData } from './invalidate'
import s from './dialogs.module.css'

interface Props {
  instructorId: number
  /** 없으면 새 경력 */
  career?: Career
  /** 새 경력의 직위 기본값 (Rust `DEFAULT_POSITION`) */
  defaultPosition: string
  onClose: () => void
}

/**
 * 경력 등록·수정. 새 경력은 '재직중' 으로 시작한다.
 * 상태·날짜 검사는 Rust `domain::career::prepare` 가 한다. 미래 종료일은 확인 창을 거친다.
 * 이미 종료한 경력의 종료일·사유를 고치거나 재직중으로 되돌리는 것도 여기서 한다(별도 '종료 취소' 없음).
 */
export function CareerDialog({ instructorId, career, defaultPosition, onClose }: Props) {
  const qc = useQueryClient()
  const [confirm, confirmDialog] = useConfirm()
  const [form, setForm] = useState<CareerInput>({
    programName: career?.programName ?? '',
    position: career?.position ?? defaultPosition,
    duty: career?.duty ?? '',
    startDate: career?.startDate ?? '',
    status: career?.status ?? 'ACTIVE',
    endDate: career?.endDate ?? null,
    endReason: career?.endReason ?? null,
    plannedEndDate: career?.plannedEndDate ?? null,
    memo: career?.memo ?? '',
  })

  const save = useMutation({
    mutationFn: () => {
      // 재직중이면 종료 칸은 보내지 않는다 (화면에서 상태만 바꾼 경우 남은 값을 지운다)
      const input: CareerInput =
        form.status === 'ACTIVE' ? { ...form, endDate: null, endReason: null } : form
      return withFutureConfirm(
        (ok) => (career ? careerApi.update(career.id, input, ok) : careerApi.create(instructorId, input, ok)),
        confirm,
      )
    },
    onSuccess: (saved) => {
      if (!saved) return // 미래 종료일 확인에서 취소
      invalidateCareerData(qc)
      onClose()
    },
  })

  const set = <K extends keyof CareerInput>(k: K, v: CareerInput[K]) => {
    setForm({ ...form, [k]: v })
    save.reset()
  }

  /** [제안 적용] — 제안은 Rust 가 만든다. 이미 적힌 지도사항은 확인 없이 덮지 않는다. */
  const applySuggestion = async () => {
    const { dutySuggestion } = await careerApi.hints(form.programName)
    if (!dutySuggestion || dutySuggestion === form.duty) return
    if (form.duty.trim()) {
      const ok = await confirm({
        title: '지도사항 제안 적용',
        message: (
          <>
            지금 적힌 지도사항 ‘{form.duty}’ 을(를)
            <br />‘{dutySuggestion}’ (으)로 바꿀까요?
          </>
        ),
        confirmLabel: '바꾸기',
      })
      if (!ok) return
    }
    set('duty', dutySuggestion)
  }

  const ended = form.status === 'ENDED'
  const ready = form.programName.trim() && form.position.trim() && form.duty.trim() && form.startDate
  const editingArchived = career?.archived

  return (
    <Modal
      title={career ? '경력 수정' : '경력 추가'}
      onClose={onClose}
      busy={save.isPending}
      width={600}
      footer={
        <>
          <Button onClick={onClose} disabled={save.isPending}>
            취소
          </Button>
          <Button variant="primary" onClick={() => save.mutate()} disabled={save.isPending || !ready || editingArchived}>
            {save.isPending ? '저장 중…' : '저장'}
          </Button>
        </>
      }
    >
      <ErrorNotice error={save.error} />
      <div className={s.grid}>
        <Field label="프로그램명">
          <Input value={form.programName} onChange={(e) => set('programName', e.target.value)} autoFocus placeholder="예: 마술" />
        </Field>
        <Field label="직위">
          <Input value={form.position} onChange={(e) => set('position', e.target.value)} />
        </Field>
        <Field
          label="지도사항"
          className={s.wide}
          hint={
            <span className={s.hintRow}>
              증명서의 ‘지도사항’ 칸에 그대로 찍힙니다.
              <Button size="sm" variant="ghost" icon={Wand2} onClick={applySuggestion} disabled={!form.programName.trim()}>
                제안 적용
              </Button>
            </span>
          }
        >
          <Input value={form.duty} onChange={(e) => set('duty', e.target.value)} placeholder="예: 방과후학교 마술" />
        </Field>
        <Field label="시작일">
          <Input type="date" value={form.startDate} onChange={(e) => set('startDate', e.target.value)} />
        </Field>
        <Field label="예정 종료일" hint="계약서의 예정 종료일 (선택). 관리용이며 증명서 기간에는 쓰지 않습니다. 이 날이 지나도 저절로 종료되지 않습니다.">
          <Input
            type="date"
            value={form.plannedEndDate ?? ''}
            onChange={(e) => set('plannedEndDate', e.target.value || null)}
          />
        </Field>
        <Field label="상태">
          <div className={s.segment} role="radiogroup">
            {(['ACTIVE', 'ENDED'] as CareerStatus[]).map((v) => (
              <label key={v} className={form.status === v ? s.segOn : s.seg}>
                <input
                  type="radio"
                  name="status"
                  checked={form.status === v}
                  onChange={() =>
                    setForm({
                      ...form,
                      status: v,
                      endReason: v === 'ENDED' ? (form.endReason ?? 'CONTRACT_END') : null,
                    })
                  }
                />
                {v === 'ACTIVE' ? '재직중' : '종료'}
              </label>
            ))}
          </div>
        </Field>
        {ended && (
          <>
            <Field label="종료일">
              <Input type="date" value={form.endDate ?? ''} onChange={(e) => set('endDate', e.target.value || null)} />
            </Field>
            <Field label="종료 사유">
              <Select value={form.endReason ?? ''} onChange={(e) => set('endReason', (e.target.value || null) as EndReason | null)}>
                <option value="">골라 주세요</option>
                {END_REASONS.map((r) => (
                  <option key={r.value} value={r.value}>
                    {r.label}
                  </option>
                ))}
              </Select>
            </Field>
          </>
        )}
        <Field label="메모" className={s.wide}>
          <Textarea value={form.memo} rows={2} onChange={(e) => set('memo', e.target.value)} placeholder="선택" />
        </Field>
      </div>
      {!career && !ended && <p className={s.note}>새 경력은 ‘재직중’ 으로 등록됩니다. 계약이 끝나면 [종료 처리] 를 누르세요.</p>}
      {confirmDialog}
    </Modal>
  )
}
