import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { hostMatches, isHotlinkHost, rewriteHotlinkImages } from './html.ts'

const JANDAN = ['img.wangmoyu.com', '*.cdn.dfyun.com.cn']

describe('hostMatches', () => {
  it('精确域名和子域名后缀分开认', () => {
    assert.equal(hostMatches('img.wangmoyu.com', 'img.wangmoyu.com'), true)
    assert.equal(hostMatches('*.cdn.dfyun.com.cn', 'node7-27.cdn.dfyun.com.cn'), true)
    assert.equal(hostMatches('.cdn.dfyun.com.cn', 'wangmoyuimg.cdn.dfyun.com.cn'), true)
    assert.equal(hostMatches('.cdn.dfyun.com.cn', 'cdn.dfyun.com.cn'), false)
    assert.equal(hostMatches('photos.example.org', 'photos.example.org'), true)
    assert.equal(isHotlinkHost('example.com', JANDAN), false)
    assert.equal(isHotlinkHost('img.wangmoyu.com', ['photos.example.org']), false)
  })
})

describe('rewriteHotlinkImages', () => {
  it('把配置里的防盗链图片改到带 token 的代理地址', () => {
    const html =
      '<p>hi</p><img src="https://wangmoyuimg.cdn.dfyun.com.cn/mw600/abc.png" alt="a">'
    const out = rewriteHotlinkImages(html, 'http://127.0.0.1:8787/', 'secret token', JANDAN)
    assert.match(out, /src="http:\/\/127\.0\.0\.1:8787\/api\/media\?url=/)
    assert.match(out, /token=secret%20token/)
    assert.match(out, /wangmoyuimg\.cdn\.dfyun\.com\.cn%2Fmw600%2Fabc\.png/)
    assert.doesNotMatch(out, /src="https:\/\/wangmoyuimg/)
  })

  it('名单里没有的域名、或没配服务端时保持原样', () => {
    const html = '<img src="https://example.com/a.png">'
    assert.equal(
      rewriteHotlinkImages(html, 'http://127.0.0.1:8787', 't', JANDAN),
      html,
    )
    const hot = '<img src="https://img.wangmoyu.com/a.jpg">'
    assert.equal(rewriteHotlinkImages(hot, '', 't', JANDAN), hot)
    assert.equal(rewriteHotlinkImages(hot, 'http://127.0.0.1:8787', '', JANDAN), hot)
    assert.equal(rewriteHotlinkImages(hot, 'http://127.0.0.1:8787', 't', []), hot)
  })
})
