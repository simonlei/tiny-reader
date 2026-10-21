import assert from 'node:assert/strict'
import fs from 'node:fs'
import { describe, it } from 'node:test'
import {
  catalogCategoryLabel,
  feedDetail,
  recommendFeedPage,
  type CatalogFeed,
} from './recommendFeeds.ts'

const catalog: CatalogFeed[] = [
  {
    id: 'a1',
    title: 'OpenAI',
    url: 'https://openai.com/feed',
    site: 'https://openai.com',
    category: 'Artificial Intelligence',
    language: 'en',
    packs: ['ai', 'top200'],
  },
  {
    id: 'a2',
    title: 'DeepSeek',
    url: 'https://deepseek.com/feed',
    site: 'https://deepseek.com',
    category: 'Artificial Intelligence',
    language: 'zh',
    packs: ['ai', 'top200'],
  },
  {
    id: 'e1',
    title: 'Rust Blog',
    url: 'https://blog.rust-lang.org/feed.xml',
    site: 'https://blog.rust-lang.org',
    category: 'Engineering & Technology',
    language: 'en',
    packs: ['engineering', 'top200'],
  },
  {
    id: 'b1',
    title: '某博客',
    url: 'https://example.com/atom',
    site: 'https://example.com',
    category: 'Personal Blogs',
    language: 'zh',
    packs: ['blogs'],
  },
  {
    id: 'n1',
    title: '每日新闻',
    url: 'https://news.example/rss',
    site: 'https://news.example',
    category: 'News',
    language: 'zh',
    packs: ['news', 'top200'],
  },
  {
    id: 's1',
    title: '安全通告',
    url: 'https://sec.example/rss',
    site: 'https://sec.example',
    category: 'Security',
    language: 'zh',
    packs: ['security'],
  },
]

describe('catalogCategoryLabel', () => {
  it('把目录里的英文分类显示成侧边栏用的中文分组', () => {
    assert.equal(catalogCategoryLabel('Artificial Intelligence'), 'AI')
    assert.equal(catalogCategoryLabel('Engineering & Technology'), '技术')
    assert.equal(catalogCategoryLabel('没见过的分类'), '没见过的分类')
  })
})

describe('recommendFeedPage', () => {
  it('没有订阅时，第一批分散在不同分类，并优先中文', () => {
    const page = recommendFeedPage(catalog, [], 0, 4)
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['a2', 'n1', 'b1', 's1'],
    )
    assert.equal(page.page, 0)
    assert.equal(page.pages, 2)
    assert.ok(page.items.every((f) => f.language === 'zh'))
    assert.equal(new Set(page.items.map((f) => f.category)).size, 4)
  })

  it('换一批给出下一批，翻过最后一批回到第一批', () => {
    const next = recommendFeedPage(catalog, [], 1, 4)
    assert.deepEqual(
      next.items.map((f) => f.id),
      ['e1', 'a1'],
    )
    assert.equal(next.page, 1)
    const wrapped = recommendFeedPage(catalog, [], 2, 4)
    assert.deepEqual(
      wrapped.items.map((f) => f.id),
      ['a2', 'n1', 'b1', 's1'],
    )
    assert.equal(wrapped.page, 0)
    const negative = recommendFeedPage(catalog, [], -1, 4)
    assert.equal(negative.page, 1)
  })

  it('已订阅的地址换协议、www 或末尾斜杠后仍然排除', () => {
    const page = recommendFeedPage(
      catalog,
      [{ url: 'https://www.OpenAI.com/feed/', title: 'OpenAI', category: 'AI' }],
      0,
      6,
    )
    assert.ok(page.items.every((f) => f.id !== 'a1'))
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['a2'],
    )
  })

  it('按已订阅源的标题匹配主题，中文订阅优先推荐中文同类', () => {
    const page = recommendFeedPage(
      catalog,
      [{ url: 'https://mine.example/rss', title: 'LLM 周刊', category: 'AI' }],
      0,
      6,
    )
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['a2', 'a1'],
    )
    const again = recommendFeedPage(
      catalog,
      [{ url: 'https://mine.example/rss', title: 'LLM 周刊', category: 'AI' }],
      1,
      6,
    )
    assert.equal(again.pages, 1)
    assert.deepEqual(
      again.items.map((f) => f.id),
      ['a2', 'a1'],
    )
  })

  it('订阅源标题没有关键词时，用同站点在目录里的主题来推荐', () => {
    const page = recommendFeedPage(
      [
        ...catalog,
        {
          id: 'apple',
          title: 'Apple ML',
          url: 'https://machinelearning.apple.com/rss.xml',
          site: 'https://machinelearning.apple.com',
          category: 'Artificial Intelligence',
          language: 'en',
          packs: ['ai', 'research', 'top200'],
        },
      ],
      [
        {
          url: 'https://machinelearning.apple.com/rss.xml',
          title: 'Apple ML',
          category: '',
        },
      ],
      0,
      6,
    )
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['a1', 'a2'],
    )
  })

  it('RSSHub 中转域名不把不相干的源算成同一主题', () => {
    const page = recommendFeedPage(
      [
        {
          id: 'hub-ai',
          title: 'Anthropic',
          url: 'https://rsshub.bestblogs.dev/anthropic/news',
          site: 'https://www.anthropic.com/news',
          category: 'Artificial Intelligence',
          language: 'en',
          packs: ['ai'],
        },
        {
          id: 'hub-forum',
          title: 'V2EX',
          url: 'https://rsshub.bestblogs.dev/v2ex/topics',
          site: 'https://v2ex.com',
          category: 'Communities',
          language: 'zh',
          packs: ['communities'],
        },
      ],
      [
        {
          url: 'https://rsshub.bestblogs.dev/anthropic/news',
          title: 'Anthropic',
          category: '',
        },
      ],
      0,
      6,
    )
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['hub-forum'],
    )
  })

  it('换一批时跳过这一轮已经展示过的源', () => {
    const page = recommendFeedPage(catalog, [], 0, 4, new Set(['a2', 'n1', 'b1', 's1']))
    assert.deepEqual(
      page.items.map((f) => f.id),
      ['e1', 'a1'],
    )
    assert.equal(page.page, 0)
    assert.equal(page.pages, 1)
    assert.equal(page.total, 2)
  })

  it('目录里的源都订阅过时没有推荐', () => {
    const page = recommendFeedPage(
      catalog,
      catalog.map((f) => ({ url: f.url, title: f.title, category: f.category })),
      0,
      4,
    )
    assert.deepEqual(page.items, [])
    assert.equal(page.pages, 0)
    assert.equal(page.page, 0)
  })
})

describe('feedDetail', () => {
  const feed: CatalogFeed = {
    id: 'x',
    title: 'Last Week in AI',
    url: 'https://lastweekin.ai/feed',
    site: 'https://lastweekin.ai',
    category: 'Artificial Intelligence',
    language: 'zh',
    packs: ['all', 'ai', 'top200', 'weeklies', 'chinese'],
    description: '每周梳理人工智能研究、产品与政策动态。',
    latest_item_at: '2026-08-11T16:30:00Z',
    organization: 'Last Week in AI',
    company_direction: 'AI',
  }

  it('悬停时列出简介、标签、语言、机构和地址', () => {
    const detail = feedDetail(feed)
    assert.equal(detail.description, '每周梳理人工智能研究、产品与政策动态。')
    assert.deepEqual(detail.tags, ['AI', '精选', '周刊', '中文'])
    assert.deepEqual(
      detail.facts.map((row) => row.label),
      ['语言', '机构', '方向', '站点', '订阅地址', '最近更新'],
    )
    assert.equal(detail.facts.find((row) => row.label === '语言')?.value, '中文')
    assert.equal(detail.facts.find((row) => row.label === '机构')?.value, 'Last Week in AI')
    assert.equal(detail.facts.find((row) => row.label === '方向')?.value, 'AI')
    assert.equal(detail.facts.find((row) => row.label === '站点')?.value, 'https://lastweekin.ai')
    assert.equal(detail.facts.find((row) => row.label === '订阅地址')?.value, 'https://lastweekin.ai/feed')
    assert.equal(detail.facts.find((row) => row.label === '最近更新')?.value, '2026-08-12')
  })

  it('分类套话和空字段不占一行', () => {
    const detail = feedDetail({
      ...feed,
      description: 'Artificial Intelligence 订阅源',
      organization: '',
      company_direction: undefined,
      latest_item_at: undefined,
      site: '',
      packs: ['all'],
    })
    assert.equal(detail.description, '')
    assert.deepEqual(detail.tags, [])
    assert.deepEqual(
      detail.facts.map((row) => row.label),
      ['语言', '订阅地址'],
    )
  })
})

describe('内置目录', () => {
  const file = new URL('../data/feedCatalog.json', import.meta.url)
  const data = JSON.parse(fs.readFileSync(file, 'utf8')) as {
    source: string
    feeds: CatalogFeed[]
  }

  it('来自 Tidings 精选，并且能按当前订阅推荐出一批', () => {
    assert.match(data.source, /tidings-rss/)
    assert.ok(data.feeds.length > 100)
    const page = recommendFeedPage(
      data.feeds,
      [{ url: 'https://example.invalid/openai.xml', title: 'OpenAI', category: '' }],
      0,
      6,
    )
    assert.equal(page.items.length, 6)
    assert.ok(page.items.every((f) => f.packs.includes('ai')))
    assert.ok(page.pages > 1)

    const next = recommendFeedPage(
      data.feeds,
      [{ url: 'https://example.invalid/openai.xml', title: 'OpenAI', category: '' }],
      1,
      6,
    )
    const seen = new Set(page.items.map((f) => f.id))
    assert.ok(next.items.every((f) => !seen.has(f.id)))
  })

  it('多数源带有简介，悬停信息能读到简介', () => {
    const described = data.feeds.filter((f) => f.description)
    assert.ok(described.length > 200)
    const detail = feedDetail(described[0])
    assert.equal(detail.description, described[0].description)
    assert.ok(detail.facts.some((row) => row.label === '订阅地址'))
  })

  it('没有任何订阅时，第一批覆盖多个中文分类', () => {
    const page = recommendFeedPage(data.feeds, [], 0, 6)
    assert.equal(page.items.length, 6)
    assert.ok(new Set(page.items.map((f) => f.category)).size >= 4)
    assert.ok(page.items.filter((f) => f.language === 'zh').length >= 4)
  })
})
