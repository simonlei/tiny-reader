export interface Feed {
  id: number
  url: string
  title: string
  site_url: string | null
  description: string | null
  category: string
  last_fetched_at: string | null
  last_error: string | null
  created_at: string
  /** 未读数（列表接口返回） */
  unread_count: number
  total_count: number
}

export interface Article {
  id: number
  feed_id: number
  guid: string
  title: string
  author: string | null
  url: string | null
  summary: string | null
  content: string | null
  published_at: string | null
  fetched_at: string
  is_read: boolean
  is_starred: boolean
  feed_title?: string | null
  feed_site_url?: string | null
}

export interface ArticlePage {
  items: Article[]
  total: number
  offset: number
  limit: number
}

export interface Stats {
  total: number
  unread: number
  starred: number
}

export interface FeedError {
  feed_id: number
  title: string
  message: string
}

export interface RefreshStatus {
  running: boolean
  total: number
  done: number
  new_articles: number
  errors: FeedError[]
  started_at: string | null
  finished_at: string | null
}

export interface NewFeed {
  url: string
  title?: string
  category?: string
}

export interface UpdateFeed {
  url?: string
  title?: string
  category?: string
}

export interface ImportResult {
  ok: boolean
  added: number
  skipped: number
}

export type ArticleFilter = 'all' | 'unread' | 'starred'
