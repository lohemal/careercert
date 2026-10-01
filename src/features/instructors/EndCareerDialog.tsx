import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Button, ErrorNotice, Field, Input, Modal } from '@/components/ui'
import { useConfirm, withFutureConfirm } from '@/components/useConfirm'
import { careerApi, END_REASONS, type Career, type EndReason } from '@/ipc/career'
import { invalidateCareerData } from './invalidate'
import s from './dialogs.module.css'

interface Props {
  career: Career
  instructorLabel: string
  onClose: () => void
}

/**
 * 재직중 경력 종료 처리. 종료일 < 시작일 이면 Rust 가 거절하고, 종료일이 오늘 이후면 확인 창을 거친다.
 */
export function EndCareerDialog({ career, instructorLabel, onClose }: Props) {
  const qc = useQueryClient()
  const [confirm, confirmDialog] = useConfirm()
  const [endDate, setEndDate] = useState('')
  const [reason, setReason] = useState<EndReason>('CONTRACT_END')

  const save = useMutation({
    mutationFn: () => withFutureConfirm((ok) => careerApi.end(career.id, endDate, reason, ok), confirm),
    onSuccess: (saved) => {
      if (!saved) return
      invalidateCareerData(qc)
      onClose()
    },
  })

  return (
    <Modal
      title="종료 처리"
      onClose={onClose}
      busy={save.isPending}
      width={480}
      footer={
        <>
          <Button onClick={onClose} disabled={save.isPending}>
            취소
          </Button>
          <Button variant="primary" onClick={() => save.mutate()} disabled={save.isPending || !endDate}>
            {save.isPending ? '처리 중…' : '종료 처리'}
          </Button>
        </>
      }
    >
      <dl className={s.facts}>
        <dt>강사</dt>
        <dd>{instructorLabel}</dd>
        <dt>경력</dt>
        <dd>
          {career.programName} · {career.period}
        </dd>
      </dl>
      <ErrorNotice error={save.error} />
      <Field label="종료일">
        <Input
          type="date"
          value={endDate}
          onChange={(e) => {
            setEndDate(e.target.value)
            save.reset()
          }}
          autoFocus
        />
      </Field>
      <Field label="종료 사유">
        <div className={s.segment} role="radiogroup">
          {END_REASONS.map((r) => (
            <label key={r.value} className={reason === r.value ? s.segOn : s.seg}>
              <input
                type="radio"
                name="reason"
                checked={reason === r.value}
                onChange={() => {
                  setReason(r.value)
                  save.reset()
                }}
              />
              {r.label}
            </label>
          ))}
        </div>
      </Field>
      {confirmDialog}
    </Modal>
  )
}
