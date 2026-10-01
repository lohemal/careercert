/** `김가람 (1985년생)` — 동명이인을 가리는 구분 메모를 이름 뒤에 붙인다. 확인 창·목록에서 사람을 가리킬 때. */
export function whoLabel(i: { name: string; distinguisher: string }) {
  return i.distinguisher ? `${i.name} (${i.distinguisher})` : i.name
}
