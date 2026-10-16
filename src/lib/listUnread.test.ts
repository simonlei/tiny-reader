import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { listFilterCount, listFilterLabel } from './listUnread.ts'

const stats = { total: 20, unread: 12, starred: 6 }
const feeds = [
  { id: 1, total_count: 10, unread_count: 3, starred_count: 1 },
  { id: 2, total_count: 8, unread_count: 5, starred_count: 4 },
]

describe('listFilterCount', () => {
  it('全部文章时，三个 tab 分别用全局的总数、未读数、星标数', () => {
    assert.equal(listFilterCount('all', null, stats, feeds), 20)
    assert.equal(listFilterCount('unread', null, stats, feeds), 12)
    assert.equal(listFilterCount('starred', null, stats, feeds), 6)
  })

  it('选中订阅源时，三个 tab 分别用该源的总数、未读数、星标数', () => {
    assert.equal(listFilterCount('all', 2, stats, feeds), 8)
    assert.equal(listFilterCount('unread', 2, stats, feeds), 5)
    assert.equal(listFilterCount('starred', 2, stats, feeds), 4)
  })

  it('选中的源不存在时记为 0', () => {
    assert.equal(listFilterCount('all', 9, stats, feeds), 0)
    assert.equal(listFilterCount('unread', 9, stats, feeds), 0)
    assert.equal(listFilterCount('starred', 9, stats, feeds), 0)
  })
})

describe('listFilterLabel', () => {
  it('文案跟着当前 tab', () => {
    assert.equal(listFilterLabel('all'), '全部')
    assert.equal(listFilterLabel('unread'), '未读')
    assert.equal(listFilterLabel('starred'), '星标')
  })
})
