import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { LIST_EXCERPT_LIMIT, excerptAround, highlightParts, searchExcerpt } from './highlight.ts'

describe('highlightParts', () => {
  it('没有关键词时整段都不算命中', () => {
    assert.deepEqual(highlightParts('油价上涨', ''), [{ text: '油价上涨', hit: false }])
    assert.deepEqual(highlightParts('油价上涨', '   '), [{ text: '油价上涨', hit: false }])
  })

  it('按原样标出每一处命中，英文不区分大小写', () => {
    assert.deepEqual(highlightParts('Aa bb AA', 'aa'), [
      { text: 'Aa', hit: true },
      { text: ' bb ', hit: false },
      { text: 'AA', hit: true },
    ])
  })

  it('关键词里的符号按字面匹配', () => {
    assert.deepEqual(highlightParts('save 100% now', '100%'), [
      { text: 'save ', hit: false },
      { text: '100%', hit: true },
      { text: ' now', hit: false },
    ])
  })

  it('空文本没有片段', () => {
    assert.deepEqual(highlightParts('', '油价'), [])
  })
})

describe('excerptAround', () => {
  it('没命中时保留开头', () => {
    assert.equal(excerptAround('油价上涨', '天气', 160), '油价上涨')
    assert.equal(excerptAround('abcdefghij', '', 4), 'abcd…')
  })

  it('命中在很后面时，把窗口挪到命中附近', () => {
    const text = `${'甲'.repeat(80)}油价${'乙'.repeat(40)}`
    const out = excerptAround(text, '油价', 40)
    assert.equal(out.startsWith('…'), true)
    assert.equal(out.includes('油价'), true)
    assert.ok(out.indexOf('油价') <= 13)
  })
})

describe('searchExcerpt', () => {
  it('正文没有关键词、摘要有时，用摘要而不是正文开头', () => {
    const content = '甲'.repeat(80)
    const summary = '今天油价波动'
    const out = searchExcerpt(content, summary, '油价', 40)
    assert.equal(out.includes('油价'), true)
    assert.equal(out.includes('甲'), false)
  })

  it('按列表两行的长度取窗口时，后文里的命中仍在开头附近', () => {
    const content = `${'甲'.repeat(80)}油价到了${'乙'.repeat(20)}`
    const out = searchExcerpt(content, '', '油价', LIST_EXCERPT_LIMIT)
    assert.equal(out.startsWith('…'), true)
    assert.ok(out.indexOf('油价') <= 13)
  })

  it('正文里有关键词时，窗口落在正文的命中处', () => {
    const content = `${'甲'.repeat(80)}油价${'乙'.repeat(20)}`
    const summary = '摘要里的油价'
    const out = searchExcerpt(content, summary, '油价', 40)
    assert.equal(out.includes('油价'), true)
    assert.equal(out.includes('乙'), true)
  })
})
