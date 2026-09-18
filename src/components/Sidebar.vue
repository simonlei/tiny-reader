<script setup lang="ts">
import { computed, ref } from 'vue'
import * as app from '@/stores/app'
import type { ArticleFilter, Feed } from '@/api/types'

const emit = defineEmits<{
  add: []
  importOpml: []
  settings: []
  editFeed: [Feed]
}>()

const { state } = app

const collapsed = ref<Record<string, boolean>>({})

interface Group {
  name: string
  feeds: Feed[]
}

const groups = computed<Group[]>(() => {
  const map = new Map<string, Feed[]>()
  for (const f of state.feeds) {
    const key = f.category || '未分类'
    const list = map.get(key)
    if (list) list.push(f)
    else map.set(key, [f])
  }
  return [...map.entries()]
    .sort((a, b) => a[0].localeCompare(b[0], 'zh-Hans-CN'))
    .map(([name, feeds]) => ({ name, feeds }))
})

const filters: { key: ArticleFilter; label: string }[] = [
  { key: 'all', label: '全部' },
  { key: 'unread', label: '未读' },
  { key: 'starred', label: '星标' },
]

function toggleGroup(name: string) {
  collapsed.value[name] = !collapsed.value[name]
}

function isCollapsed(name: string) {
  return !!collapsed.value[name]
}
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <span class="logo">TR</span>
      <span class="name">Tiny Reader</span>
      <button class="ghost icon-btn" title="设置" @click="emit('settings')">⚙</button>
    </div>

    <div class="search">
      <input
        :value="state.keyword"
        placeholder="搜索标题 / 摘要…"
        @input="app.setKeyword(($event.target as HTMLInputElement).value)"
        @keyup.enter="app.reload()"
      />
    </div>

    <nav class="quick">
      <button
        class="quick-item"
        :class="{ active: state.selectedFeedId == null }"
        @click="app.selectFeed(null)"
      >
        <span class="qi-label">全部文章</span>
        <span v-if="state.stats.unread" class="badge">{{ state.stats.unread }}</span>
      </button>
      <button
        class="quick-item"
        :class="{ active: state.selectedFeedId == null && state.filter === 'starred' }"
        @click="
          () => {
            app.selectFeed(null)
            app.setFilter('starred')
          }
        "
      >
        <span class="qi-label">★ 星标</span>
        <span v-if="state.stats.starred" class="badge dim">{{ state.stats.starred }}</span>
      </button>
    </nav>

    <div class="filter-row">
      <button
        v-for="f in filters"
        :key="f.key"
        class="seg"
        :class="{ on: state.filter === f.key }"
        @click="app.setFilter(f.key)"
      >
        {{ f.label }}
      </button>
    </div>

    <div class="feeds">
      <div class="feeds-head">
        <span class="text-mute">订阅源 ({{ state.feeds.length }})</span>
        <button class="ghost icon-btn" title="添加订阅源" @click="emit('add')">＋</button>
      </div>

      <div v-if="!state.feeds.length" class="empty text-mute">
        还没有订阅源，点上面的 ＋ 或导入 OPML
      </div>

      <div v-for="g in groups" :key="g.name" class="group">
        <button class="group-head" @click="toggleGroup(g.name)">
          <span class="caret">{{ isCollapsed(g.name) ? '▸' : '▾' }}</span>
          <span class="gname truncate">{{ g.name }}</span>
          <span class="badge dim">{{ g.feeds.length }}</span>
        </button>

        <ul v-show="!isCollapsed(g.name)">
          <li v-for="f in g.feeds" :key="f.id">
            <div
              class="feed-item"
              :class="{ active: state.selectedFeedId === f.id, failed: !!f.last_error }"
              @click="app.selectFeed(f.id)"
            >
              <span class="fname truncate" :title="f.last_error || f.url">
                {{ f.title }}
              </span>
              <span v-if="f.unread_count" class="badge">{{ f.unread_count }}</span>
              <button
                class="ghost icon-btn edit"
                title="编辑 / 删除"
                @click.stop="emit('editFeed', f)"
              >
                ⋯
              </button>
            </div>
          </li>
        </ul>
      </div>
    </div>

    <div class="sidebar-foot">
      <button class="ghost" @click="emit('importOpml')">导入 OPML</button>
      <div class="stat text-mute">
        共 {{ state.stats.total }} 篇 · 未读 {{ state.stats.unread }}
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 244px;
  flex: 0 0 244px;
  background: var(--bg-alt);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 12px 8px;
}
.logo {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: linear-gradient(135deg, var(--accent), #7c5cff);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}
.name {
  font-weight: 600;
  font-size: 14px;
  flex: 1;
}
.icon-btn {
  padding: 2px 7px;
  font-size: 14px;
  line-height: 1.3;
}

.search {
  padding: 0 12px 10px;
}
.search input {
  font-size: 12.5px;
  padding: 6px 9px;
}

.quick {
  padding: 0 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.quick-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: transparent;
  border-color: transparent;
  padding: 6px 8px;
  text-align: left;
  border-radius: var(--radius-sm);
}
.quick-item:hover {
  background: var(--panel-hover);
}
.quick-item.active {
  background: var(--accent-soft);
  color: #cfe0ff;
}

.filter-row {
  display: flex;
  gap: 4px;
  padding: 8px 12px 4px;
}
.seg {
  flex: 1;
  padding: 4px 0;
  font-size: 12px;
  background: transparent;
  border-color: var(--border);
}
.seg.on {
  background: var(--accent-soft);
  border-color: var(--accent-dim);
  color: #cfe0ff;
}

.feeds {
  flex: 1;
  overflow-y: auto;
  padding: 6px 8px 4px;
  margin-top: 4px;
}
.feeds-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 6px 6px;
  font-size: 11.5px;
}
.empty {
  padding: 10px 8px;
  font-size: 12px;
  line-height: 1.5;
}

.group {
  margin-bottom: 2px;
}
.group-head {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 5px;
  background: transparent;
  border-color: transparent;
  padding: 4px 6px;
  font-size: 11.5px;
  color: var(--text-mute);
  text-transform: none;
}
.group-head:hover {
  background: var(--panel-hover);
}
.caret {
  width: 10px;
  font-size: 9px;
}
.gname {
  flex: 1;
  text-align: left;
}

ul {
  list-style: none;
  margin: 0;
  padding: 0;
}
.feed-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 13px;
}
.feed-item:hover {
  background: var(--panel-hover);
}
.feed-item.active {
  background: var(--accent-soft);
  color: #cfe0ff;
}
.feed-item.failed .fname {
  color: var(--warn);
}
.fname {
  flex: 1;
  min-width: 0;
}
.edit {
  padding: 0 5px;
  opacity: 0;
  font-size: 13px;
}
.feed-item:hover .edit {
  opacity: 1;
}

.badge {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: var(--text-dim);
  background: var(--panel-2);
  border-radius: 9px;
  padding: 0 6px;
  min-width: 20px;
  text-align: center;
}
.badge.dim {
  background: transparent;
  padding: 0 2px;
}

.sidebar-foot {
  border-top: 1px solid var(--border-soft);
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.stat {
  font-size: 11px;
}
</style>
