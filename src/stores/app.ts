import { computed, reactive } from 'vue'
import * as api from '@/api/client'
import { ApiError } from '@/api/client'
import type { Article, ArticleFilter, Feed, RefreshStatus, Stats } from '@/api/types'

const PAGE_SIZE = 50
/** 选中后多久自动标记已读（毫秒） */
const AUTO_READ_DELAY = 600

export const state = reactive({
  connected: false,
  loading: false,
  loadingMore: false,
  error: '',

  feeds: [] as Feed[],
  articles: [] as Article[],
  total: 0,

  selectedFeedId: null as number | null,
  filter: 'unread' as ArticleFilter,
  selectedId: null as number | null,
  keyword: '',

  stats: { total: 0, unread: 0, starred: 0 } as Stats,

  refreshing: false,
  refreshInfo: {
    running: false,
    total: 0,
    done: 0,
    new_articles: 0,
    errors: [],
    started_at: null,
    finished_at: null,
  } as RefreshStatus,
})

let autoReadTimer: ReturnType<typeof setTimeout> | undefined

export const selectedIndex = computed(() =>
  state.articles.findIndex((a) => a.id === state.selectedId),
)

export const selectedArticle = computed<Article | null>(
  () => state.articles.find((a) => a.id === state.selectedId) ?? null,
)

export const hasMore = computed(() => state.articles.length < state.total)

export const currentFeedTitle = computed(() => {
  if (state.selectedFeedId == null) return '全部文章'
  return state.feeds.find((f) => f.id === state.selectedFeedId)?.title ?? '全部文章'
})

export const totalUnread = computed(() =>
  state.feeds.reduce((sum, f) => sum + (f.unread_count || 0), 0),
)

export function handleError(e: unknown) {
  if (e instanceof ApiError) {
    state.connected = e.status !== 0
    state.error = e.isUnauthorized ? 'token 无效，请在设置里重新填写' : e.message
  } else {
    state.connected = false
    state.error = (e as Error)?.message ?? String(e)
  }
  console.error('[tiny-reader]', e)
}

// ------------------------------------------------------------------ 加载数据

export async function loadFeeds() {
  try {
    state.feeds = await api.listFeeds()
    state.error = ''
    state.connected = true
  } catch (e) {
    handleError(e)
    throw e
  }
}

export async function loadStats() {
  try {
    state.stats = await api.stats()
  } catch (e) {
    handleError(e)
  }
}

export async function loadArticles(reset = true) {
  if (reset) {
    if (state.loading) return
    state.loading = true
  } else {
    if (state.loadingMore || !hasMore.value) return
    state.loadingMore = true
  }

  try {
    const offset = reset ? 0 : state.articles.length
    const page = await api.listArticles({
      feedId: state.selectedFeedId,
      filter: state.filter,
      limit: PAGE_SIZE,
      offset,
      keyword: state.keyword.trim() || undefined,
    })
    if (reset) {
      state.articles = page.items
      // 刷新后保留当前选中的文章；若已不在列表里则选第一篇
      if (!page.items.some((a) => a.id === state.selectedId)) {
        state.selectedId = page.items[0]?.id ?? null
      }
    } else {
      const known = new Set(state.articles.map((a) => a.id))
      state.articles.push(...page.items.filter((a) => !known.has(a.id)))
      if (state.selectedId == null) state.selectedId = state.articles[0]?.id ?? null
    }
    state.total = page.total
    state.error = ''
    state.connected = true
  } catch (e) {
    handleError(e)
  } finally {
    state.loading = false
    state.loadingMore = false
  }
}

/** 切换订阅源 / 筛选条件 / 关键词时调用 */
export async function reload() {
  state.selectedId = null
  await loadArticles(true)
}

export function selectFeed(id: number | null) {
  state.selectedFeedId = id
  void reload()
}

export function setFilter(f: ArticleFilter) {
  if (state.filter === f) return
  state.filter = f
  void reload()
}

export function setKeyword(kw: string) {
  state.keyword = kw
}

// ------------------------------------------------------------------ 选择文章

function scheduleAutoRead(id: number) {
  if (autoReadTimer) clearTimeout(autoReadTimer)
  autoReadTimer = setTimeout(() => {
    const a = state.articles.find((x) => x.id === id)
    if (!a || a.is_read) return
    a.is_read = true
    const feed = state.feeds.find((f) => f.id === a.feed_id)
    if (feed && feed.unread_count > 0) feed.unread_count -= 1
    if (state.stats.unread > 0) state.stats.unread -= 1
    void api
      .setRead(id, true)
      .then(() => loadStats())
      .catch(handleError)
  }, AUTO_READ_DELAY)
}

export function selectArticle(id: number | null, opts: { autoRead?: boolean } = {}) {
  state.selectedId = id
  if (id != null && opts.autoRead !== false) scheduleAutoRead(id)
}

/**
 * j / k 的核心：在当前列表里上下移动。
 * 到末尾时自动加载下一页。返回是否真的移动了。
 */
export async function move(delta: number) {
  const list = state.articles
  if (list.length === 0) return false

  let idx = selectedIndex.value
  if (idx < 0) {
    idx = 0
  } else {
    idx += delta
  }

  // 越界处理
  if (idx < 0) idx = 0
  if (idx >= list.length) {
    if (hasMore.value) {
      await loadArticles(false)
      // 加载后重新计算
      const newIdx = selectedIndex.value + delta
      if (newIdx >= state.articles.length) return false
      selectArticle(state.articles[newIdx].id)
      return true
    }
    idx = list.length - 1
  }

  selectArticle(list[idx].id)
  return true
}

// ------------------------------------------------------------- 文章状态操作

export async function toggleRead(article?: Article | null) {
  const a = article ?? selectedArticle.value
  if (!a) return
  const next = !a.is_read
  a.is_read = next
  const feed = state.feeds.find((f) => f.id === a.feed_id)
  if (feed) feed.unread_count = Math.max(0, feed.unread_count + (next ? -1 : 1))
  state.stats.unread = Math.max(0, state.stats.unread + (next ? -1 : 1))
  try {
    await api.setRead(a.id, next)
    await loadStats()
  } catch (e) {
    handleError(e)
  }
}

export async function toggleStar(article?: Article | null) {
  const a = article ?? selectedArticle.value
  if (!a) return
  const next = !a.is_starred
  a.is_starred = next
  state.stats.starred = Math.max(0, state.stats.starred + (next ? 1 : -1))
  try {
    await api.setStarred(a.id, next)
  } catch (e) {
    handleError(e)
  }
}

export async function markAllRead() {
  try {
    await api.markRead({ feedId: state.selectedFeedId })
    await Promise.all([loadFeeds(), loadStats(), loadArticles(true)])
  } catch (e) {
    handleError(e)
  }
}

// ----------------------------------------------------------------- 刷新

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

export async function refreshAll() {
  if (state.refreshing) return
  state.refreshing = true
  state.error = ''
  try {
    await api.refreshAll()
    // 轮询刷新进度
    for (let i = 0; i < 300; i++) {
      const s = await api.refreshStatus()
      state.refreshInfo = s
      if (!s.running) break
      await sleep(500)
    }
    await Promise.all([loadFeeds(), loadStats(), loadArticles(true)])
  } catch (e) {
    handleError(e)
  } finally {
    state.refreshing = false
  }
}

export async function refreshOne(feedId: number) {
  try {
    await api.refreshFeed(feedId)
    await Promise.all([loadFeeds(), loadStats(), loadArticles(true)])
  } catch (e) {
    handleError(e)
  }
}

// ------------------------------------------------------------ 订阅源管理

export async function addFeed(url: string, title?: string, category?: string) {
  const feed = await api.addFeed({ url, title, category })
  await Promise.all([loadFeeds(), loadStats()])
  return feed
}

export async function updateFeed(id: number, patch: { url?: string; title?: string; category?: string }) {
  const feed = await api.updateFeed(id, patch)
  await loadFeeds()
  return feed
}

export async function removeFeed(id: number) {
  await api.deleteFeed(id)
  if (state.selectedFeedId === id) state.selectedFeedId = null
  await Promise.all([loadFeeds(), loadStats(), loadArticles(true)])
}

export async function importOpml(xml: string) {
  const res = await api.importOpml(xml)
  await Promise.all([loadFeeds(), loadStats()])
  return res
}

/** 首次进入 / 设置变更后的统一入口 */
export async function bootstrap() {
  try {
    await Promise.all([loadFeeds(), loadStats()])
    await loadArticles(true)
  } catch {
    /* 错误已写入 state.error */
  }
}
