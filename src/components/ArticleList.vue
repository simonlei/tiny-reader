<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import * as app from '@/stores/app'
import type { Article } from '@/api/types'
import { formatTime, stripHtml, truncate } from '@/lib/format'

const { state } = app
const currentFeedTitle = app.currentFeedTitle
const hasMore = app.hasMore
const listEl = ref<HTMLElement | null>(null)

// 选中项滚动到可视区域
watch(
  () => state.selectedId,
  async () => {
    await nextTick()
    const el = listEl.value?.querySelector<HTMLElement>('.art.active')
    if (!el) return
    const box = listEl.value!.getBoundingClientRect()
    const r = el.getBoundingClientRect()
    if (r.top < box.top + 8) {
      listEl.value!.scrollTop -= box.top + 8 - r.top
    } else if (r.bottom > box.bottom - 8) {
      listEl.value!.scrollTop += r.bottom - (box.bottom - 8)
    }
  },
)

function onScroll() {
  const el = listEl.value
  if (!el) return
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 240) {
    void app.loadArticles(false)
  }
}

function open(a: Article, e: MouseEvent) {
  app.selectArticle(a.id)
  // 中键 / Ctrl+点击 打开原文
  if (e.metaKey || e.ctrlKey) window.open(a.url || '', '_blank')
}

onMounted(() => {
  if (listEl.value) listEl.value.scrollTop = 0
})

watch(() => state.selectedFeedId, onScrollReset)
watch(() => state.filter, onScrollReset)
function onScrollReset() {
  if (listEl.value) listEl.value.scrollTop = 0
}

function snippet(a: Article): string {
  return truncate(stripHtml(a.content || a.summary), 160)
}
</script>

<template>
  <section class="list-pane">
    <header class="list-head">
      <div class="lh-title">
        <span class="truncate">{{ currentFeedTitle }}</span>
        <span class="count text-mute">{{ state.total }}</span>
      </div>
      <div class="lh-actions">
        <button class="ghost" title="全部标为已读 (Shift+A)" @click="app.markAllRead()">
          全部已读
        </button>
      </div>
    </header>

    <div ref="listEl" class="list" @scroll.passive="onScroll">
      <div v-if="state.loading && !state.articles.length" class="hint text-mute">加载中…</div>

      <div v-else-if="!state.articles.length" class="hint text-mute">
        {{ state.filter === 'unread' ? '没有未读文章 🎉' : '这里还没有文章' }}
      </div>

      <article
        v-for="a in state.articles"
        :key="a.id"
        class="art"
        :class="{ active: a.id === state.selectedId, read: a.is_read }"
        @click="open(a, $event)"
      >
        <div class="art-top">
          <span class="dot" :class="{ on: !a.is_read }" />
          <h4 class="art-title truncate">{{ a.title }}</h4>
          <span v-if="a.is_starred" class="star" title="已加星标">★</span>
        </div>
        <div class="art-meta text-mute">
          <span v-if="state.selectedFeedId == null" class="src truncate">
            {{ a.feed_title }}
          </span>
          <span class="time">{{ formatTime(a.published_at || a.fetched_at) }}</span>
        </div>
        <p class="art-snippet">{{ snippet(a) }}</p>
      </article>

      <div v-if="state.loadingMore" class="hint text-mute">加载更多…</div>
      <div v-else-if="!hasMore && state.articles.length" class="hint text-mute">
        — 到底了 —
      </div>
    </div>
  </section>
</template>

<style scoped>
.list-pane {
  width: 350px;
  flex: 0 0 350px;
  border-right: 1px solid var(--border);
  background: var(--panel);
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-soft);
  min-height: 44px;
}
.lh-title {
  display: flex;
  align-items: baseline;
  gap: 7px;
  min-width: 0;
  font-weight: 600;
  font-size: 13.5px;
}
.count {
  font-size: 11.5px;
  font-weight: 400;
}
.lh-actions button {
  font-size: 12px;
  padding: 4px 8px;
}

.list {
  flex: 1;
  overflow-y: auto;
}

.hint {
  padding: 22px 14px;
  text-align: center;
  font-size: 12.5px;
}

.art {
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-soft);
  cursor: pointer;
  border-left: 3px solid transparent;
}
.art:hover {
  background: var(--panel-2);
}
.art.active {
  background: var(--accent-soft);
  border-left-color: var(--accent);
}
.art.read .art-title {
  color: var(--text-mute);
  font-weight: 400;
}

.art-top {
  display: flex;
  align-items: center;
  gap: 7px;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
  flex: 0 0 7px;
}
.dot.on {
  box-shadow: 0 0 0 3px rgba(76, 141, 255, 0.16);
}
.art:not(.read) .dot {
  opacity: 1;
}
.art.read .dot {
  opacity: 0.22;
}
.art-title {
  flex: 1;
  margin: 0;
  font-size: 13.5px;
  font-weight: 600;
  line-height: 1.45;
  min-width: 0;
}
.star {
  color: var(--warn);
  font-size: 12px;
}

.art-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
  margin-top: 3px;
  padding-left: 14px;
}
.src {
  max-width: 62%;
}
.time {
  margin-left: auto;
  font-variant-numeric: tabular-nums;
}

.art-snippet {
  margin: 5px 0 0;
  padding-left: 14px;
  font-size: 12.5px;
  color: var(--text-mute);
  line-height: 1.55;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
