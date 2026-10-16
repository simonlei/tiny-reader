import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { listUnreadCount } from './listUnread.ts'

const feeds = [
  { id: 1, unread_count: 3 },
  { id: 2, unread_count: 5 },
]

describe('listUnreadCount', () => {
  it('全部文章时显示全局未读数', () => {
    assert.equal(listUnreadCount(null, 12, feeds), 12)
  })

  it('选中订阅源时显示该源的未读数', () => {
    assert.equal(listUnreadCount(2, 12, feeds), 5)
  })

  it('选中的源不存在时记为 0', () => {
    assert.equal(listUnreadCount(9, 12, feeds), 0)
  })
})
