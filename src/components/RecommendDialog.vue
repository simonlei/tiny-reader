<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import BaseDialog from './BaseDialog.vue'
import * as app from '@/stores/app'
import { feedCatalog, feedCatalogSource } from '@/lib/feedCatalog'
import { openExternal } from '@/lib/tauri'
import {
  RECOMMEND_BATCH_SIZE,
  catalogCategoryLabel,
  feedDetail,
  recommendFeedPage,
  type CatalogFeed,
} from '@/lib/recommendFeeds'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; added: [title: string] }>()

const { state } = app

const items = ref<CatalogFeed[]>([])
const page = ref(0)
const pages = ref(0)
/** 这次打开对话框后已经看过的目录 id。换一批从剩下的里面继续取。 */
const shownIds = ref(new Set<string>())
const pendingId = ref<string | null>(null)
const added = ref(new Set<string>())
const errors = ref<Record<string, string>>({})
const tip = ref<{ item: CatalogFeed; top: number; left: number } | null>(null)
const tipDetail = computed(() => (tip.value ? feedDetail(tip.value.item) : null))

const hint = computed(() =>
  state.feeds.length
    ? '按你已经订阅的源匹配主题，订过的不会再出现。'
    : '还没有订阅。下面是精选目录里不同分类的一批，可以直接添加。',
)

function hints() {
  return state.feeds.map((f) => ({ url: f.url, title: f.title, category: f.category }))
}

function load() {
  const result = recommendFeedPage(feedCatalog, hints(), 0, RECOMMEND_BATCH_SIZE, shownIds.value)
  items.value = result.items
  const total = shownIds.value.size + result.total
  pages.value = total === 0 ? 0 : Math.ceil(total / RECOMMEND_BATCH_SIZE)
  page.value = total === 0 ? 0 : Math.floor(shownIds.value.size / RECOMMEND_BATCH_SIZE)
  errors.value = {}
  tip.value = null
}

function shuffle() {
  if (pendingId.value || pages.value <= 1 || !items.value.length) return
  const nextShown = new Set(shownIds.value)
  for (const item of items.value) nextShown.add(item.id)
  shownIds.value = nextShown
  load()
  if (!items.value.length) {
    shownIds.value = new Set()
    load()
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open) return
    pendingId.value = null
    added.value = new Set()
    shownIds.value = new Set()
    load()
  },
)

function showTip(event: MouseEvent, item: CatalogFeed) {
  const row = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const width = 300
  const margin = 8
  let left = row.right + margin
  if (left + width > window.innerWidth - margin) left = Math.max(margin, row.left - width - margin)
  let top = row.top
  const height = 280
  if (top + height > window.innerHeight - margin) top = Math.max(margin, window.innerHeight - height - margin)
  tip.value = { item, top, left }
}

function hideTip() {
  tip.value = null
}

function hostLabel(item: CatalogFeed): string {
  const raw = item.site || item.url
  try {
    return new URL(raw).hostname.replace(/^www\./i, '')
  } catch {
    return raw
  }
}

async function add(item: CatalogFeed) {
  if (pendingId.value || added.value.has(item.id)) return
  pendingId.value = item.id
  const nextErrors = { ...errors.value }
  delete nextErrors[item.id]
  errors.value = nextErrors
  try {
    await app.addFeed(item.url, item.title, catalogCategoryLabel(item.category))
    added.value = new Set(added.value).add(item.id)
    emit('added', item.title)
  } catch (e) {
    errors.value = { ...errors.value, [item.id]: (e as Error).message }
  } finally {
    if (pendingId.value === item.id) pendingId.value = null
  }
}
</script>

<template>
  <BaseDialog :open="props.open" title="推荐订阅源" :width="520" @close="emit('close')">
    <p class="hint text-mute">{{ hint }}</p>

    <p v-if="!items.length" class="empty text-mute">能推荐的源都已经在订阅列表里了。</p>

    <ul v-else class="list" @scroll="hideTip">
      <li
        v-for="item in items"
        :key="item.id"
        @mouseenter="showTip($event, item)"
        @mouseleave="hideTip"
      >
        <div class="meta">
          <div class="title-row">
            <span class="title truncate">{{ item.title }}</span>
            <span class="cat">{{ catalogCategoryLabel(item.category) }}</span>
          </div>
          <div class="host text-mute truncate">{{ hostLabel(item) }}</div>
          <p v-if="errors[item.id]" class="err">{{ errors[item.id] }}</p>
        </div>
        <button
          :class="{ primary: !added.has(item.id) }"
          :disabled="pendingId !== null || added.has(item.id)"
          @click="add(item)"
        >
          {{ added.has(item.id) ? '已添加' : pendingId === item.id ? '添加中…' : '添加' }}
        </button>
      </li>
    </ul>

    <p class="credit text-mute">
      目录来自
      <a :href="feedCatalogSource" @click.prevent="openExternal(feedCatalogSource)">Tidings RSS 精选</a>
      ，共 {{ feedCatalog.length }} 个源。
    </p>

    <Teleport to="body">
      <div
        v-if="tip && tipDetail"
        class="feed-tip"
        :style="{ top: tip.top + 'px', left: tip.left + 'px' }"
      >
        <div class="tip-title">{{ tip.item.title }}</div>
        <p v-if="tipDetail.description" class="tip-desc">{{ tipDetail.description }}</p>
        <div v-if="tipDetail.tags.length" class="tip-tags">
          <span v-for="tag in tipDetail.tags" :key="tag">{{ tag }}</span>
        </div>
        <dl>
          <div v-for="row in tipDetail.facts" :key="row.label">
            <dt>{{ row.label }}</dt>
            <dd>{{ row.value }}</dd>
          </div>
        </dl>
      </div>
    </Teleport>

    <template #footer>
      <div class="foot">
        <span class="text-mute">{{ pages > 1 ? `第 ${page + 1} / ${pages} 批` : '' }}</span>
        <span class="actions">
          <button :disabled="pendingId !== null || pages <= 1" @click="shuffle">换一批</button>
          <button @click="emit('close')">关闭</button>
        </span>
      </div>
    </template>
  </BaseDialog>
</template>

<style scoped>
.hint {
  margin: 0 0 12px;
  font-size: 12.5px;
}
.list {
  list-style: none;
  margin: 0;
  padding: 0;
  border: 1px solid var(--border-soft);
  border-radius: 6px;
  max-height: 360px;
  overflow-y: auto;
}
.list li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
}
.list li + li {
  border-top: 1px solid var(--border-soft);
}
.meta {
  flex: 1;
  min-width: 0;
}
.title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.title {
  font-size: 13px;
}
.cat {
  flex: none;
  font-size: 11px;
  color: var(--text-dim);
  background: var(--panel-2);
  border-radius: 9px;
  padding: 0 6px;
}
.host {
  font-size: 11.5px;
}
.err {
  margin: 4px 0 0;
  color: var(--danger);
  font-size: 12px;
  white-space: normal;
}
.empty {
  margin: 0 0 12px;
  font-size: 12.5px;
}
.credit {
  margin: 10px 0 0;
  font-size: 11.5px;
}
.credit a {
  color: var(--accent);
}
.foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: 8px;
  font-size: 12px;
}
.actions {
  display: flex;
  gap: 8px;
}
.feed-tip {
  position: fixed;
  z-index: 300;
  width: 300px;
  max-height: 280px;
  overflow: auto;
  padding: 10px 12px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  pointer-events: none;
}
.tip-title {
  font-size: 13px;
  font-weight: 600;
}
.tip-desc {
  margin: 6px 0 0;
  font-size: 12.5px;
  line-height: 1.5;
}
.tip-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 8px;
}
.tip-tags span {
  font-size: 11px;
  color: var(--text-dim);
  background: var(--panel-2);
  border-radius: 9px;
  padding: 0 6px;
}
.feed-tip dl {
  margin: 8px 0 0;
}
.feed-tip dl div {
  display: grid;
  grid-template-columns: 58px 1fr;
  gap: 6px;
  margin-top: 4px;
  font-size: 12px;
}
.feed-tip dt {
  color: var(--text-mute);
}
.feed-tip dd {
  margin: 0;
  word-break: break-all;
}
</style>
