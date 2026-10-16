import assert from 'node:assert/strict'
import { afterEach, beforeEach, describe, it } from 'node:test'
import { configure } from '../api/client.ts'
import * as app from './app.ts'

type Call = { path: string; method: string }

const originalFetch = globalThis.fetch

function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  })
}

function article(id: number) {
  return {
    id,
    feed_id: 1,
    guid: `g-${id}`,
    title: `文章 ${id}`,
    author: null,
    url: null,
    summary: null,
    content: null,
    published_at: '2026-01-01T00:00:00Z',
    fetched_at: '2026-01-01T00:00:00Z',
    is_read: false,
    is_starred: false,
    feed_title: '源',
  }
}

describe('refreshList', () => {
  const calls: Call[] = []
  let release: (() => void) | undefined
  let gate: Promise<void> | undefined

  beforeEach(() => {
    calls.length = 0
    release = undefined
    gate = undefined
    configure({ serverUrl: 'http://reader.test', token: 't' })
    app.state.selectedId = 7
    app.state.articles = []
    app.state.loading = false
    app.state.loadingMore = false
    app.state.reloading = false
    app.state.error = ''

    globalThis.fetch = async (input, init) => {
      const url = new URL(String(input))
      calls.push({ path: url.pathname, method: (init?.method ?? 'GET').toUpperCase() })
      if (gate) await gate
      if (url.pathname === '/api/feeds') return json([])
      if (url.pathname === '/api/stats') return json({ total: 1, unread: 0, starred: 0 })
      if (url.pathname === '/api/articles') {
        return json({ items: [article(7), article(8)], total: 2, offset: 0, limit: 50, next_cursor: null })
      }
      return json({ ok: true, total: 0 })
    }
  })

  afterEach(() => {
    release?.()
    globalThis.fetch = originalFetch
  })

  it('只从服务端重载文章列表，不重新抓取订阅源', async () => {
    await app.refreshList()

    assert.deepEqual(
      calls.map((c) => `${c.method} ${c.path}`).sort(),
      ['GET /api/articles', 'GET /api/feeds', 'GET /api/stats'],
    )
    assert.equal(app.state.selectedId, 7)
    assert.equal(app.state.reloading, false)
  })

  it('刷新进行中时忽略重复点击', async () => {
    gate = new Promise<void>((resolve) => {
      release = resolve
    })

    const first = app.refreshList()
    const second = app.refreshList()
    release?.()
    await Promise.all([first, second])

    assert.equal(calls.filter((c) => c.path === '/api/articles').length, 1)
  })
})
