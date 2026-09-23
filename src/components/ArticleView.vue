<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, nextTick } from 'vue'
import * as app from '@/stores/app'
import { openExternal } from '@/lib/tauri'
import { formatTime } from '@/lib/format'
import { absolutizeHtml } from '@/lib/html'
import { registerReaderBody } from '@/lib/readerScroll'

const { state } = app
const selected = app.selectedArticle

// 正文滚动容器，切换文章时要把滚动条拉回顶部
const bodyEl = ref<HTMLElement | null>(null)

// 注册给快捷键：j / ↓ 与 k / ↑ 先在正文里滚动，到边界才切换文章
watch(bodyEl, (el) => registerReaderBody(el ?? null), { immediate: true })
onBeforeUnmount(() => registerReaderBody(null))

watch(
  () => state.selectedId,
  () => {
    // 等新内容渲染完成后再归零，避免作用在旧内容上
    nextTick(() => {
      if (bodyEl.value) bodyEl.value.scrollTop = 0
    })
  },
)

// 正文：源内容是按原样保存的，这里只在渲染前把相对图片/链接地址补成绝对地址，
// 否则会被解析到 tauri.localhost 上导致图片 404
const body = computed(() => {
  const raw = selected.value?.content || selected.value?.summary || ''
  return absolutizeHtml(raw, selected.value?.feed_site_url)
})
const showFallbackHint = computed(
  () => !selected.value?.content && !!selected.value?.summary,
)

/** 发表时间；源未提供时为 ''，此时整段（含分隔符）都不渲染 */
const fmtPublished = computed(() =>
  formatTime(selected.value?.published_at, { fallback: null }),
)

function onContentClick(e: MouseEvent) {
  const el = (e.target as HTMLElement)?.closest?.('a')
  if (!el) return
  const href = (el as HTMLAnchorElement).getAttribute('href')
  if (!href) return
  // 站点内锚点交给浏览器自己跳
  if (href.startsWith('#')) return
  e.preventDefault()
  void openExternal(href)
}
</script>

<template>
  <section class="reader">
    <template v-if="selected">
      <header class="r-head">
        <div class="r-meta">
          <span class="r-src">{{ selected.feed_title || '未知来源' }}</span>
          <template v-if="fmtPublished">
            <span class="sep">·</span>
            <span>{{ fmtPublished }}</span>
          </template>
          <span v-if="selected.author" class="sep">·</span>
          <span v-if="selected.author">{{ selected.author }}</span>
        </div>
        <h1 class="r-title">
          <a
            v-if="selected.url"
            :href="selected.url"
            @click.prevent="openExternal(selected.url)"
            >{{ selected.title }}</a
          >
          <template v-else>{{ selected.title }}</template>
        </h1>
        <div class="r-actions">
          <button :class="{ on: selected.is_starred }" @click="app.toggleStar()">
            {{ selected.is_starred ? '★ 已收藏' : '☆ 收藏' }}
          </button>
          <button @click="app.toggleRead()">
            {{ selected.is_read ? '标为未读' : '标为已读' }}
          </button>
          <button v-if="selected.url" @click="openExternal(selected.url)">在浏览器中打开</button>
          <span class="spacer" />
          <span class="kbd-hint text-mute">
            <kbd>j</kbd> / <kbd>↓</kbd> 滚动 · <kbd>←</kbd> <kbd>→</kbd> 切换文章
          </span>
        </div>
      </header>

      <div class="r-body" ref="bodyEl">
        <p v-if="showFallbackHint" class="fallback text-mute">
          该源只提供了摘要，点标题可在浏览器里查看全文。
        </p>
        <!-- 内容来自 RSS 源本身，已在服务端按原样保存；这里只做渲染 -->
        <div class="rich-text" @click="onContentClick" v-html="body" />
      </div>
    </template>

    <div v-else class="placeholder text-mute">
      <p v-if="state.error">⚠ {{ state.error }}</p>
      <p v-else-if="state.loading">加载中…</p>
      <p v-else>选择一篇文章开始阅读</p>
      <div class="keys">
        <div><kbd>j</kbd> / <kbd>↓</kbd> 向下滚动，到底换下一篇</div>
        <div><kbd>k</kbd> / <kbd>↑</kbd> 向上滚动，到顶换上一篇</div>
        <div><kbd>←</kbd> / <kbd>→</kbd> 直接切换上/下一篇文章</div>
        <div><kbd>s</kbd> 收藏 / 取消收藏</div>
        <div><kbd>u</kbd> 已读 / 未读</div>
        <div><kbd>o</kbd> 在浏览器打开</div>
        <div><kbd>r</kbd> 刷新所有订阅源</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.reader {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg);
  overflow: hidden;
}

.r-head {
  padding: 18px 28px 12px;
  border-bottom: 1px solid var(--border-soft);
}
.r-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-mute);
}
.r-src {
  color: var(--accent);
}
.sep {
  opacity: 0.5;
}
.r-title {
  margin: 6px 0 12px;
  font-size: 21px;
  line-height: 1.35;
  font-weight: 650;
}
.r-title a {
  color: inherit;
  text-decoration: none;
}
.r-title a:hover {
  color: var(--accent);
}

.r-actions {
  display: flex;
  align-items: center;
  gap: 7px;
}
.r-actions button {
  font-size: 12px;
  padding: 4px 10px;
}
.r-actions button.on {
  color: var(--warn);
  border-color: rgba(224, 163, 58, 0.4);
}
.spacer {
  flex: 1;
}
.kbd-hint {
  font-size: 11.5px;
}

.r-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px 28px 60px;
}
.fallback {
  font-size: 12px;
  margin: 0 0 14px;
  padding: 7px 10px;
  background: var(--panel);
  border-left: 2px solid var(--border);
  border-radius: 3px;
}

.placeholder {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 13px;
}
.keys {
  margin-top: 22px;
  display: grid;
  grid-template-columns: auto auto;
  gap: 8px 26px;
  font-size: 12px;
  color: var(--text-mute);
}
</style>
