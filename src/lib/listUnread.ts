/** 文章列表头部的数量跟着当前 tab：全部 / 未读 / 星标。未选源用全局统计，选中源用该源自己的数。 */

export type ListCountFilter = 'all' | 'unread' | 'starred'

export interface ListCountStats {
  total: number
  unread: number
  starred: number
}

export interface ListCountFeed {
  id: number
  total_count: number
  unread_count: number
  starred_count: number
}

export function listFilterLabel(filter: ListCountFilter): string {
  if (filter === 'all') return '全部'
  if (filter === 'starred') return '星标'
  return '未读'
}

export function listFilterCount(
  filter: ListCountFilter,
  selectedFeedId: number | null,
  stats: ListCountStats,
  feeds: readonly ListCountFeed[],
): number {
  if (selectedFeedId == null) {
    if (filter === 'all') return stats.total
    if (filter === 'starred') return stats.starred
    return stats.unread
  }
  const feed = feeds.find((f) => f.id === selectedFeedId)
  if (!feed) return 0
  if (filter === 'all') return feed.total_count || 0
  if (filter === 'starred') return feed.starred_count || 0
  return feed.unread_count || 0
}
