/** 文章列表头部要展示的未读数：全部文章用全局统计，单个订阅源用该源自己的未读数。 */
export function listUnreadCount(
  selectedFeedId: number | null,
  statsUnread: number,
  feeds: readonly { id: number; unread_count: number }[],
): number {
  if (selectedFeedId == null) return statsUnread
  return feeds.find((f) => f.id === selectedFeedId)?.unread_count ?? 0
}
