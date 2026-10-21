/**
 * 从内置精选目录里按「当前已订阅的源」挑下一批可添加的订阅。
 * 目录来自 Tidings RSS 的 data/feeds.json（见 src/data/feedCatalog.json）。
 */

export interface CatalogFeed {
  id: string
  title: string
  url: string
  site: string
  category: string
  language: string
  packs: string[]
  /** 有内容的简介。分类套话（例如「News 订阅源」）不放进来。 */
  description?: string
  latest_item_at?: string
  organization?: string
  company_direction?: string
}

export interface FeedDetailRow {
  label: string
  value: string
}

export interface FeedDetail {
  description: string
  tags: string[]
  facts: FeedDetailRow[]
}

export interface SubscribedFeedHint {
  url: string
  title: string
  category: string
}

export interface RecommendPage {
  items: CatalogFeed[]
  /** 归一化后的批次，从 0 开始 */
  page: number
  pages: number
  /** 去掉已订阅、已展示之后还剩多少条 */
  total: number
}

export const RECOMMEND_BATCH_SIZE = 6

/** 目录英文分类 → 侧边栏分组。未知分类原样返回。 */
const CATEGORY_ZH: Record<string, string> = {
  'Artificial Intelligence': 'AI',
  'Engineering & Technology': '技术',
  'Personal Blogs': '博客',
  'News': '新闻',
  'Research & Science': '科研',
  Communities: '社区',
  'Culture & Ideas': '文化',
  'Technology Media': '科技媒体',
  'Tech Newsletters & Weeklies': '周刊',
  Security: '安全',
  'Business & Startups': '商业',
  'Product & Design': '产品设计',
  Videos: '视频',
  Podcasts: '播客',
}

/** 分类对应的主题。有的分类没有单独的 pack，靠这个对上关键词。 */
const CATEGORY_INTEREST: Record<string, string> = {
  'Artificial Intelligence': 'ai',
  'Engineering & Technology': 'engineering',
  'Personal Blogs': 'blogs',
  News: 'news',
  'Research & Science': 'research',
  Communities: 'communities',
  'Culture & Ideas': 'culture',
  'Technology Media': 'tech-media',
  'Tech Newsletters & Weeklies': 'weeklies',
  Security: 'security',
  'Business & Startups': 'business',
  'Product & Design': 'product',
  Videos: 'videos',
  Podcasts: 'podcasts',
}

const PACK_HINTS: { pack: string; words: string[] }[] = [
  { pack: 'ai', words: ['ai', 'llm', 'gpt', 'openai', 'claude', 'deepseek', 'gemini', '人工智能', '机器学习', '大模型'] },
  { pack: 'security', words: ['安全', '漏洞', 'security', 'cve'] },
  { pack: 'news', words: ['新闻', '资讯'] },
  { pack: 'research', words: ['论文', '科研', '科学', 'arxiv', 'research'] },
  { pack: 'communities', words: ['社区', '论坛', 'v2ex', 'reddit', 'hacker news', 'hackernews'] },
  { pack: 'weeklies', words: ['周刊', '周报', '月刊', 'newsletter'] },
  { pack: 'tech-media', words: ['少数派', '极客公园', '科技媒体'] },
  { pack: 'blogs', words: ['博客', '随笔'] },
  {
    pack: 'engineering',
    words: ['技术', '编程', '开发', '程序员', '前端', '后端', '架构', 'rust', 'python', 'javascript', 'typescript', 'golang'],
  },
  { pack: 'company-tech', words: ['字节', '阿里', '美团', '腾讯'] },
  { pack: 'product', words: ['产品设计', '用户体验'] },
  { pack: 'culture', words: ['文化', '思想'] },
  { pack: 'business', words: ['创业', '商业'] },
]

const IGNORED_PACKS = new Set(['all', 'chinese', 'top200'])

const PACK_ZH: Record<string, string> = {
  ai: 'AI',
  engineering: '技术',
  blogs: '博客',
  top200: '精选',
  news: '新闻',
  research: '科研',
  communities: '社区',
  'tech-media': '科技媒体',
  weeklies: '周刊',
  security: '安全',
  'company-tech': '机构技术',
  wechat: '公众号',
  chinese: '中文',
  videos: '视频',
  podcasts: '播客',
  product: '产品设计',
  culture: '文化',
  business: '商业',
}

const LANGUAGE_ZH: Record<string, string> = {
  zh: '中文',
  en: 'English',
}

export function catalogCategoryLabel(category: string): string {
  return CATEGORY_ZH[category] ?? category
}

/** 悬停时能展示的简介、标签和其余字段。空的和分类套话都不占一行。 */
export function feedDetail(feed: CatalogFeed): FeedDetail {
  const description = usefulDescription(feed)
  const tags = feed.packs.filter((pack) => pack !== 'all').map((pack) => PACK_ZH[pack] ?? pack)
  const facts: FeedDetailRow[] = []
  const language = (feed.language || '').trim()
  if (language) facts.push({ label: '语言', value: LANGUAGE_ZH[language] ?? language })
  pushFact(facts, '机构', feed.organization)
  pushFact(facts, '方向', feed.company_direction)
  pushFact(facts, '站点', feed.site)
  pushFact(facts, '订阅地址', feed.url)
  const updated = formatCatalogDate(feed.latest_item_at)
  if (updated) facts.push({ label: '最近更新', value: updated })
  return { description, tags, facts }
}

function usefulDescription(feed: CatalogFeed): string {
  const text = (feed.description || '').trim()
  if (!text || text === `${feed.category} 订阅源`) return ''
  return text
}

function pushFact(facts: FeedDetailRow[], label: string, value: string | undefined) {
  const text = (value || '').trim()
  if (text) facts.push({ label, value: text })
}

function formatCatalogDate(iso: string | undefined): string {
  const text = (iso || '').trim()
  if (!text) return ''
  const date = new Date(text)
  if (Number.isNaN(date.getTime())) return text
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Shanghai',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(date)
}

export function recommendFeedPage(
  catalog: readonly CatalogFeed[],
  subscribed: readonly SubscribedFeedHint[],
  batch = 0,
  batchSize = RECOMMEND_BATCH_SIZE,
  /** 这一轮对话框里已经展示过的目录 id，换一批时用来接着往下取 */
  excludeIds?: ReadonlySet<string>,
): RecommendPage {
  if (batchSize < 1) return { items: [], page: 0, pages: 0, total: 0 }

  const taken = new Set(subscribed.map((s) => feedUrlKey(s.url)))
  const available = catalog.filter((f) => f.url && !taken.has(feedUrlKey(f.url)))
  const language = preferredLanguage(subscribed)
  const interests = inferInterests(catalog, subscribed)

  const ranked =
    interests.size === 0
      ? browseOrder(available, language)
      : available
          .filter((f) => overlap(f, interests) > 0)
          .sort(byScore(language, interests))
  const unseen = excludeIds ? ranked.filter((f) => !excludeIds.has(f.id)) : ranked

  return slicePage(unseen, batch, batchSize)
}

function preferredLanguage(subscribed: readonly SubscribedFeedHint[]): string {
  if (subscribed.length === 0) return 'zh'
  const blob = subscribed.map((s) => `${s.title}\n${s.category}`).join('\n')
  return /[\u4e00-\u9fff]/.test(blob) ? 'zh' : 'en'
}

function inferInterests(
  catalog: readonly CatalogFeed[],
  subscribed: readonly SubscribedFeedHint[],
): Set<string> {
  const interests = new Set<string>()
  for (const sub of subscribed) {
    const blob = `${sub.title}\n${sub.category}\n${hostOf(sub.url)}`
    for (const hint of PACK_HINTS) {
      if (hint.words.some((word) => mentions(blob, word))) interests.add(hint.pack)
    }
  }

  const byHost = new Map<string, CatalogFeed[]>()
  for (const feed of catalog) {
    for (const host of [hostOf(feed.url), hostOf(feed.site)]) {
      if (!host || isGenericHost(host)) continue
      const list = byHost.get(host) ?? []
      list.push(feed)
      byHost.set(host, list)
    }
  }
  for (const sub of subscribed) {
    const host = hostOf(sub.url)
    if (!host || isGenericHost(host)) continue
    for (const feed of byHost.get(host) ?? []) {
      for (const key of feedInterests(feed)) interests.add(key)
    }
  }
  return interests
}

function feedInterests(feed: CatalogFeed): string[] {
  const keys = new Set<string>()
  for (const pack of feed.packs) {
    if (!IGNORED_PACKS.has(pack)) keys.add(pack)
  }
  const mapped = CATEGORY_INTEREST[feed.category]
  if (mapped) keys.add(mapped)
  return [...keys]
}

function overlap(feed: CatalogFeed, interests: Set<string>): number {
  let n = 0
  for (const key of feedInterests(feed)) {
    if (interests.has(key)) n += 1
  }
  return n
}

function byScore(language: string, interests: Set<string>) {
  return (a: CatalogFeed, b: CatalogFeed) => {
    const diff = score(b, language, interests) - score(a, language, interests)
    if (diff) return diff
    const title = a.title.localeCompare(b.title, 'zh-Hans-CN')
    if (title) return title
    return a.id.localeCompare(b.id)
  }
}

function score(feed: CatalogFeed, language: string, interests: Set<string>): number {
  let value = overlap(feed, interests) * 5
  if (feed.language === language) value += 2
  if (feed.packs.includes('top200')) value += 1
  return value
}

/** 没有可匹配的主题时，按分类轮转，避免第一批全是同一个分类。 */
function browseOrder(feeds: readonly CatalogFeed[], language: string): CatalogFeed[] {
  const groups = new Map<string, CatalogFeed[]>()
  for (const feed of feeds) {
    const list = groups.get(feed.category) ?? []
    list.push(feed)
    groups.set(feed.category, list)
  }
  const quality = (feed: CatalogFeed) =>
    (feed.language === language ? 2 : 0) + (feed.packs.includes('top200') ? 1 : 0)
  for (const list of groups.values()) {
    list.sort((a, b) => {
      const diff = quality(b) - quality(a)
      if (diff) return diff
      const title = a.title.localeCompare(b.title, 'zh-Hans-CN')
      if (title) return title
      return a.id.localeCompare(b.id)
    })
  }
  const categories = [...groups.entries()].sort((a, b) => {
    const diff = quality(b[1][0]) - quality(a[1][0])
    if (diff) return diff
    return a[0].localeCompare(b[0], 'zh-Hans-CN')
  })

  const ordered: CatalogFeed[] = []
  const max = Math.max(0, ...categories.map(([, list]) => list.length))
  for (let index = 0; index < max; index += 1) {
    for (const [, list] of categories) {
      if (list[index]) ordered.push(list[index])
    }
  }
  return ordered
}

function slicePage(items: CatalogFeed[], batch: number, batchSize: number): RecommendPage {
  if (items.length === 0) return { items: [], page: 0, pages: 0, total: 0 }
  const pages = Math.ceil(items.length / batchSize)
  const page = ((batch % pages) + pages) % pages
  return {
    items: items.slice(page * batchSize, page * batchSize + batchSize),
    page,
    pages,
    total: items.length,
  }
}

function feedUrlKey(raw: string): string {
  const trimmed = raw.trim()
  try {
    const url = new URL(trimmed)
    const host = url.hostname.replace(/^www\./i, '').toLowerCase()
    const path = url.pathname.replace(/\/+$/, '').toLowerCase()
    return `${host}${path}${url.search.toLowerCase()}`
  } catch {
    return trimmed.toLowerCase().replace(/\/+$/, '')
  }
}

function hostOf(raw: string): string {
  try {
    return new URL(raw.trim()).hostname.replace(/^www\./i, '').toLowerCase()
  } catch {
    return ''
  }
}

function isGenericHost(host: string): boolean {
  return host.startsWith('rsshub.') || host.includes('rsshub')
}

function mentions(text: string, word: string): boolean {
  if (/[\u4e00-\u9fff]/.test(word)) return text.toLowerCase().includes(word.toLowerCase())
  const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return new RegExp(`(?:^|[^a-z0-9])${escaped}(?:[^a-z0-9]|$)`, 'i').test(text)
}
