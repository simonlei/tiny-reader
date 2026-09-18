<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ArticleList from './components/ArticleList.vue'
import ArticleView from './components/ArticleView.vue'
import SettingsDialog from './components/SettingsDialog.vue'
import FeedDialog from './components/FeedDialog.vue'
import ImportDialog from './components/ImportDialog.vue'
import * as app from '@/stores/app'
import { needsSetup } from '@/stores/settings'
import { openExternal } from '@/lib/tauri'
import type { Feed } from '@/api/types'

const { state } = app

const showSettings = ref(false)
const showFeed = ref(false)
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
    else if (showImport.value) showImport.value = false
    else if (showSettings.value) showSettings.value = false
    return
  }

  if (e.ctrlKey || e.metaKey || e.altKey) return
  if (isTyping(e)) return
  if (showSettings.value || showFeed.value || showImport.value) return

  const key = e.key.toLowerCase()

  switch (key) {
    case 'j':
    case 'arrowdown':
      e.preventDefault()
      await app.move(1)
      break
    case 'k':
    case 'arrowup':
      e.preventDefault()
      await app.move(-1)
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
</script>

<template>
  <div class="app">
    <header class="topbar">
      <button
        class="refresh"
        :disabled="state.refreshing"
        title="重新拉取所有订阅源 (R)"
        @click="app.refreshAll()"
      >
        <span :class="{ spin: state.refreshing }">⟳</span>
        {{ state.refreshing ? '刷新中' : '刷新' }}
      </button>

      <div v-if="state.refreshing" class="progress">
        <span class="text-dim">
          {{ state.refreshInfo.done }} / {{ state.refreshInfo.total }}
          <template v-if="state.refreshInfo.new_articles">
            · 新增 {{ state.refreshInfo.new_articles }} 篇
          </template>
        </span>
        <span class="bar">
          <i
            :style="{
              width:
                (state.refreshInfo.total
                  ? (state.refreshInfo.done / state.refreshInfo.total) * 100
                  : 0) + '%',
            }"
          />
        </span>
      </div>

      <div v-else-if="state.error" class="err-box">
        <span>⚠ {{ state.error }}</span>
        <button class="ghost small" @click="showSettings = true">去设置</button>
      </div>

      <div v-else class="status text-mute">
        {{ state.feeds.length }} 个订阅源 · 未读 {{ state.stats.unread }}
        <template v-if="state.refreshInfo.errors.length">
          · <span class="warn">{{ state.refreshInfo.errors.length }} 个源失败</span>
        </template>
      </div>

      <span class="spacer" />

      <button class="ghost" title="添加订阅源" @click="openAdd">＋ 订阅源</button>
      <button class="ghost" title="设置" @click="showSettings = true">⚙</button>
    </header>

    <main class="body">
      <Sidebar
        @add="openAdd"
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

.topbar {
  height: 42px;
  flex: 0 0 42px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  background: var(--bg-alt);
  border-bottom: 1px solid var(--border);
  -webkit-user-select: none;
  user-select: none;
}
.refresh {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 12.5px;
}
.refresh span {
  display: inline-block;
  font-size: 14px;
}

.progress {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.bar {
  display: block;
  width: 130px;
  height: 4px;
  background: var(--panel-2);
  border-radius: 2px;
  overflow: hidden;
}
.bar i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width 0.25s;
}

.err-box {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--warn);
  min-width: 0;
}
.small {
  font-size: 11.5px;
  padding: 2px 7px;
}

.status {
  font-size: 12px;
}
.warn {
  color: var(--warn);
}

.spacer {
  flex: 1;
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
