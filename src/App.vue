<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ArticleList from './components/ArticleList.vue'
import ArticleView from './components/ArticleView.vue'
import SettingsDialog from './components/SettingsDialog.vue'
import FeedDialog from './components/FeedDialog.vue'
import RecommendDialog from './components/RecommendDialog.vue'
import ImportDialog from './components/ImportDialog.vue'
import * as app from '@/stores/app'
import { needsSetup } from '@/stores/settings'
import { openExternal } from '@/lib/tauri'
import { scrollArticle } from '@/lib/readerScroll'
import type { Feed } from '@/api/types'

const { state } = app

const showSettings = ref(false)
const showFeed = ref(false)
const showRecommend = ref(false)
const showImport = ref(false)
const editingFeed = ref<Feed | null>(null)
const toast = ref('')
let toastTimer: ReturnType<typeof setTimeout> | undefined

function notify(msg: string) {
  toast.value = msg
  if (toastTimer) clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = ''), 3500)
}

function openAdd() {
  editingFeed.value = null
  showFeed.value = true
}

function openEdit(f: Feed) {
  editingFeed.value = f
  showFeed.value = true
}

// ------------------------------------------------------------------ 快捷键

function isTyping(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null
  if (!t) return false
  const tag = t.tagName.toLowerCase()
  return tag === 'input' || tag === 'textarea' || tag === 'select' || t.isContentEditable
}

async function onKeydown(e: KeyboardEvent) {
  // Esc 关闭任意弹窗
  if (e.key === 'Escape') {
    if (showFeed.value) showFeed.value = false
    else if (showRecommend.value) showRecommend.value = false
    else if (showImport.value) showImport.value = false
    else if (showSettings.value) showSettings.value = false
    return
  }

  if (e.ctrlKey || e.metaKey || e.altKey) return
  if (isTyping(e)) return
  if (showSettings.value || showFeed.value || showRecommend.value || showImport.value) return

  const key = e.key.toLowerCase()

  switch (key) {
    case 'j':
    case 'arrowdown':
      e.preventDefault()
      // 先在文章内向下滚，滚到底了才切下一篇
      if (!scrollArticle(1)) await app.move(1)
      break
    case 'k':
    case 'arrowup':
      e.preventDefault()
      // 先在文章内向上滚，滚到顶了才切上一篇
      if (!scrollArticle(-1)) await app.move(-1)
      break
    // 左右键：无视当前滚动位置，直接切换上/下一篇文章
    case 'arrowleft':
      e.preventDefault()
      await app.move(-1)
      break
    case 'arrowright':
      e.preventDefault()
      await app.move(1)
      break
    case 's':
      e.preventDefault()
      await app.toggleStar()
      break
    case 'u':
      e.preventDefault()
      await app.toggleRead()
      break
    case 'o':
    case 'enter':
      e.preventDefault()
      await openExternal(app.selectedArticle.value?.url)
      break
    case 'r':
      e.preventDefault()
      await app.refreshAll()
      break
    case 'a':
      if (e.shiftKey) {
        e.preventDefault()
        await app.markAllRead()
      }
      break
    case 'home':
      e.preventDefault()
      if (state.articles.length) app.selectArticle(state.articles[0].id)
      break
    case 'end':
      e.preventDefault()
      if (state.articles.length) app.selectArticle(state.articles[state.articles.length - 1].id)
      break
    case '/':
      e.preventDefault()
      document.querySelector<HTMLInputElement>('.search input')?.focus()
      break
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  if (needsSetup.value) {
    showSettings.value = true
  } else {
    void app.bootstrap()
  }
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  if (toastTimer) clearTimeout(toastTimer)
})

async function onSettingsSaved() {
  void app.bootstrap()
}

async function onFeedSaved() {
  await app.loadFeeds()
  await app.loadArticles(true)
}

async function onRecommended(title: string) {
  notify(`已添加「${title}」`)
  await app.loadArticles(true)
}
</script>

<template>
  <div class="app">
    <main class="body">
      <Sidebar
        @add="openAdd"
        @recommend="showRecommend = true"
        @edit-feed="openEdit"
        @import-opml="showImport = true"
        @settings="showSettings = true"
      />
      <ArticleList />
      <ArticleView />
    </main>

    <SettingsDialog
      :open="showSettings"
      @close="showSettings = false"
      @saved="onSettingsSaved"
    />
    <FeedDialog
      :open="showFeed"
      :feed="editingFeed"
      @close="showFeed = false"
      @saved="onFeedSaved"
    />
    <RecommendDialog
      :open="showRecommend"
      @close="showRecommend = false"
      @added="onRecommended"
    />
    <ImportDialog
      :open="showImport"
      @close="showImport = false"
      @done="notify"
    />

    <Transition name="fade">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </Transition>
  </div>
</template>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.body {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 26px;
  transform: translateX(-50%);
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 9px 16px;
  font-size: 12.5px;
  box-shadow: 0 8px 26px rgba(0, 0, 0, 0.45);
  z-index: 200;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
