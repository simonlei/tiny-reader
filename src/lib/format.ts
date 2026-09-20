/**
 * 相对时间：今天显示 HH:mm，今年显示 M月D日，更早显示 YYYY-M-D
 * @param iso 时间字符串（RFC3339）
 * @param opts.fallback 为空时的兜底时间（通常是 fetched_at）。
 *   传 null 表示「没有真实发表时间就不显示」，避免把拉取时间伪装成发表时间。
 */
export function formatTime(
  iso?: string | null,
  opts?: { fallback?: string | null },
): string {
  const fallback = opts && 'fallback' in opts ? opts.fallback : undefined
  const raw = iso || fallback || ''
  if (!raw) return ''
  const d = new Date(raw)
  if (Number.isNaN(d.getTime())) return ''
  const now = new Date()

  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()

  const pad = (n: number) => String(n).padStart(2, '0')

  if (sameDay) return `${pad(d.getHours())}:${pad(d.getMinutes())}`
  if (d.getFullYear() === now.getFullYear()) return `${d.getMonth() + 1}月${d.getDate()}日`
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

/** 去掉 HTML 标签，用于列表摘要 */
export function stripHtml(html?: string | null): string {
  if (!html) return ''
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<br\s*\/?>/gi, ' ')
    .replace(/<\/(p|div|li|h[1-6])>/gi, ' ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, ' ')
    .trim()
}

/** 截断到指定长度 */
export function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + '…' : s
}
