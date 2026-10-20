import assert from 'node:assert/strict'
import { afterEach, beforeEach, describe, it } from 'node:test'
import { configure } from '../api/client.ts'
import * as app from './app.ts'

const originalFetch = globalThis.fetch

function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('loadArticles keyword', () => {
  const queries: string[] = []

  beforeEach(() => {
    queries.length = 0
    configure({ serverUrl: 'http://reader.test', token: 't' })
    app.state.loading = false
    app.state.loadingMore = false
    app.state.articles = []
    app.state.keyword = ''
    app.state.appliedKeyword = ''
    app.state.selectedId = null

    globalThis.fetch = async (input) => {
      const url = new URL(String(input))
      if (url.pathname === '/api/articles') queries.push(url.search)
      return json({ items: [], total: 0, offset: 0, limit: 50, next_cursor: null })
    }
  })

  afterEach(() => {
    globalThis.fetch = originalFetch
  })

  it('列表高亮用的是这次请求实际带上的关键词', async () => {
    app.state.keyword = ' 油价 '
    await app.loadArticles(true)
    assert.equal(app.state.appliedKeyword, '油价')
    assert.match(queries[0] ?? '', /(?:^|[?&])q=%E6%B2%B9%E4%BB%B7(?:&|$)/)
  })
})
