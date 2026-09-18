import type {
  Article,
  ArticleFilter,
  ArticlePage,
  Feed,
  ImportResult,
  NewFeed,
  RefreshStatus,
  Stats,
  UpdateFeed,
} from './types'

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
  get isUnauthorized(): boolean {
    return this.status === 401 || this.status === 403
  }
}

interface Settings {
  serverUrl: string
  token: string
}

let settings: Settings = { serverUrl: '', token: '' }

export function configure(next: Settings) {
  settings = {
    serverUrl: (next.serverUrl || '').trim().replace(/\/+$/, ''),
    token: (next.token || '').trim(),
  }
}

export function currentSettings(): Settings {
  return { ...settings }
}

function ensureConfigured() {
  if (!settings.serverUrl) throw new ApiError(0, '尚未配置服务端地址')
  if (!settings.token) throw new ApiError(0, '尚未配置访问 token')
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  ensureConfigured()
  const url = `${settings.serverUrl}${path}`
  let resp: Response
  try {
    resp = await fetch(url, {
      ...init,
      headers: {
        Authorization: `Bearer ${settings.token}`,
        'Content-Type': 'application/json',
        ...(init.headers || {}),
      },
    })
  } catch (e) {
    throw new ApiError(0, `无法连接服务端（${settings.serverUrl}）：${(e as Error).message}`)
  }

  if (!resp.ok) {
    let msg = `HTTP ${resp.status}`
    try {
      const body = await resp.json()
      if (body && typeof body.error === 'string') msg = body.error
    } catch {
      /* 忽略解析失败 */
    }
    throw new ApiError(resp.status, msg)
  }

  if (resp.status === 204) return undefined as T
  const text = await resp.text()
  if (!text) return undefined as T
  return JSON.parse(text) as T
}

function query(params: Record<string, string | number | boolean | undefined | null>) {
  const usp = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === '') continue
    usp.set(k, String(v))
  }
  const s = usp.toString()
  return s ? `?${s}` : ''
}

// ------------------------------------------------------------------ 连通性

export async function health(): Promise<boolean> {
  ensureConfigured()
  try {
    const resp = await fetch(`${settings.serverUrl}/health`)
    return resp.ok
  } catch {
    return false
  }
}

// ------------------------------------------------------------------- 订阅源

export function listFeeds(): Promise<Feed[]> {
  return request<Feed[]>('/api/feeds')
}

export function getFeed(id: number): Promise<Feed> {
  return request<Feed>(`/api/feeds/${id}`)
}

export function addFeed(payload: NewFeed): Promise<Feed> {
  return request<Feed>('/api/feeds', { method: 'POST', body: JSON.stringify(payload) })
}

export function updateFeed(id: number, payload: UpdateFeed): Promise<Feed> {
  return request<Feed>(`/api/feeds/${id}`, {
    method: 'PUT',
    body: JSON.stringify(payload),
  })
}

export function deleteFeed(id: number): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/api/feeds/${id}`, { method: 'DELETE' })
}

export function refreshFeed(id: number): Promise<{ ok: boolean; new_articles: number }> {
  return request<{ ok: boolean; new_articles: number }>(`/api/feeds/${id}/refresh`, {
    method: 'POST',
  })
}

/** 导出 OPML 的下载地址（带鉴权，仅用于浏览器下载时手动拼接） */
export function opmlExportUrl(): string {
  return `${settings.serverUrl}/api/feeds/export`
}

export function importOpml(xml: string, fetchNow = true): Promise<ImportResult> {
  return request<ImportResult>('/api/feeds/import', {
    method: 'POST',
    body: JSON.stringify({ opml: xml, fetch_now: fetchNow }),
  })
}

// --------------------------------------------------------------------- 文章

export function listArticles(opts: {
  feedId?: number | null
  filter?: ArticleFilter
  limit?: number
  offset?: number
  keyword?: string
}): Promise<ArticlePage> {
  return request<ArticlePage>(
    `/api/articles${query({
      feed_id: opts.feedId ?? undefined,
      unread_only: opts.filter === 'unread',
      starred_only: opts.filter === 'starred',
      limit: opts.limit,
      offset: opts.offset,
      q: opts.keyword,
    })}`,
  )
}

export function getArticle(id: number): Promise<Article> {
  return request<Article>(`/api/articles/${id}`)
}

export function setRead(id: number, value: boolean): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/api/articles/${id}/read`, {
    method: 'POST',
    body: JSON.stringify({ value }),
  })
}

export function setStarred(id: number, value: boolean): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/api/articles/${id}/star`, {
    method: 'POST',
    body: JSON.stringify({ value }),
  })
}

export function markRead(opts: { feedId?: number | null; ids?: number[] }): Promise<{
  ok: boolean
  updated: number
}> {
  return request<{ ok: boolean; updated: number }>('/api/articles/mark-read', {
    method: 'POST',
    body: JSON.stringify({ feed_id: opts.feedId ?? null, ids: opts.ids ?? null }),
  })
}

export function stats(): Promise<Stats> {
  return request<Stats>('/api/stats')
}

// --------------------------------------------------------------------- 刷新

export function refreshAll(): Promise<{ ok: boolean; total: number }> {
  return request<{ ok: boolean; total: number }>('/api/refresh', { method: 'POST' })
}

export function refreshStatus(): Promise<RefreshStatus> {
  return request<RefreshStatus>('/api/refresh/status')
}
