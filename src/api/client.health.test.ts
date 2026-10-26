import assert from 'node:assert/strict'
import { afterEach, describe, it } from 'node:test'
import {
  ApiError,
  configure,
  createNativeHttpTransport,
  health,
  installPlatformHttp,
  listFeeds,
  setHttpTransport,
} from './client.ts'

afterEach(() => {
  setHttpTransport(null)
})

describe('health', () => {
  it('传输失败时带上地址和原因，而不是只返回 false', async () => {
    configure({ serverUrl: 'http://192.168.1.8:8787', token: 'tok' })
    setHttpTransport(async () => {
      throw new Error('Load failed')
    })
    await assert.rejects(health(), (err: unknown) => {
      assert.ok(err instanceof ApiError)
      assert.equal(err.status, 0)
      assert.match(err.message, /服务端没有响应/)
      assert.match(err.message, /192\.168\.1\.8:8787/)
      assert.match(err.message, /Load failed/)
      return true
    })
  })

  it('HTTP 非成功状态带上状态码', async () => {
    configure({ serverUrl: 'http://10.0.0.2:8787', token: 'tok' })
    setHttpTransport(async () => new Response('nope', { status: 502 }))
    await assert.rejects(health(), /HTTP 502/)
  })

  it('去掉地址末尾斜杠后再请求 /health', async () => {
    configure({ serverUrl: 'http://10.0.0.2:8787/', token: 'tok' })
    let seen = ''
    setHttpTransport(async (url) => {
      seen = url
      return new Response('{"ok":true}', { status: 200 })
    })
    assert.equal(await health(), true)
    assert.equal(seen, 'http://10.0.0.2:8787/health')
  })
})

describe('桌面端传输', () => {
  it('listFeeds 和 health 走同一个传输层', async () => {
    configure({ serverUrl: 'http://10.0.0.2:8787', token: 'secret' })
    setHttpTransport(async (url, init) => {
      assert.equal(url, 'http://10.0.0.2:8787/api/feeds')
      assert.equal(new Headers(init.headers).get('authorization'), 'Bearer secret')
      return new Response('[]', { status: 200, headers: { 'content-type': 'application/json' } })
    })
    assert.deepEqual(await listFeeds(), [])
  })

  it('原生传输把方法、正文和状态码交回去', async () => {
    const transport = createNativeHttpTransport(async (args) => {
      assert.equal(args.url, 'http://10.0.0.2:8787/api/feeds')
      assert.equal(args.method, 'POST')
      assert.equal(args.body, '{"a":1}')
      assert.equal(args.headers.authorization, 'Bearer secret')
      return { status: 201, body: '{"id":1}' }
    })
    const resp = await transport('http://10.0.0.2:8787/api/feeds', {
      method: 'post',
      headers: { Authorization: 'Bearer secret', 'Content-Type': 'application/json' },
      body: '{"a":1}',
    })
    assert.equal(resp.status, 201)
    assert.equal(await resp.text(), '{"id":1}')
  })

  it('只有 macOS 改走原生请求', async () => {
    configure({ serverUrl: 'http://10.0.0.2:8787', token: 'tok' })
    let calls = 0
    const invoke = async () => {
      calls += 1
      return { status: 200, body: '{"ok":true}' }
    }
    assert.equal(installPlatformHttp('windows', invoke), false)
    assert.equal(calls, 0)
    assert.equal(installPlatformHttp('macos', invoke), true)
    assert.equal(await health(), true)
    assert.equal(calls, 1)
  })
})
