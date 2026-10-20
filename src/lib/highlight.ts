/** 列表摘要只显示约两行。窗口按这个长度截，命中才不会被行数裁掉。 */
export const LIST_EXCERPT_LIMIT = 56

export interface HighlightPart {
  text: string
  hit: boolean
}

/** 不区分大小写的子串位置。按原文字长度切片，避免把 `%`、`.` 当成正则。 */
function indexOfFold(text: string, keyword: string, from = 0): number {
  const needle = keyword.toLowerCase()
  const n = keyword.length
  if (!needle || n === 0) return -1
  for (let i = from; i + n <= text.length; i++) {
    if (text.slice(i, i + n).toLowerCase() === needle) return i
  }
  return -1
}

/** 把纯文本拆成命中 / 未命中片段，供列表用文本节点渲染，不走 HTML。 */
export function highlightParts(text: string, keyword: string): HighlightPart[] {
  if (!text) return []
  const kw = keyword.trim()
  if (!kw) return [{ text, hit: false }]

  const parts: HighlightPart[] = []
  let i = 0
  while (i < text.length) {
    const at = indexOfFold(text, kw, i)
    if (at < 0) {
      parts.push({ text: text.slice(i), hit: false })
      break
    }
    if (at > i) parts.push({ text: text.slice(i, at), hit: false })
    parts.push({ text: text.slice(at, at + kw.length), hit: true })
    i = at + kw.length
  }
  return parts
}

/**
 * 列表摘要窗口。命中在后文时留少量前文，让两行预览能看见关键词。
 * 没有命中时保留开头。
 */
export function excerptAround(text: string, keyword: string, limit: number): string {
  const src = text.replace(/\s+/g, ' ').trim()
  if (!src || limit <= 0) return ''
  const kw = keyword.trim()
  const at = kw ? indexOfFold(src, kw) : -1
  if (at < 0) return src.length > limit ? src.slice(0, limit) + '…' : src
  if (src.length <= limit) return src

  const lead = Math.min(12, Math.max(0, limit - kw.length))
  let start = Math.max(0, at - lead)
  let end = Math.min(src.length, start + limit)
  const matchEnd = at + kw.length
  if (kw.length <= limit && matchEnd > end) {
    end = Math.min(src.length, matchEnd)
    start = Math.max(0, end - limit)
  }
  const prefix = start > 0 ? '…' : ''
  const suffix = end < src.length ? '…' : ''
  return prefix + src.slice(start, end) + suffix
}

/**
 * 挑选列表摘要的来源。关键词只出现在摘要里时用摘要，
 * 否则用正文（正文为空再退回摘要），并把窗口对准命中。
 */
export function searchExcerpt(
  content: string,
  summary: string,
  keyword: string,
  limit: number,
): string {
  const kw = keyword.trim()
  const bodyHas = !!kw && indexOfFold(content, kw) >= 0
  const summaryHas = !!kw && indexOfFold(summary, kw) >= 0
  const src = bodyHas || !summaryHas ? content || summary : summary
  return excerptAround(src, kw, limit)
}
