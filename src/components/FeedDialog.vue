<script setup lang="ts">
import { ref, watch } from 'vue'
import BaseDialog from './BaseDialog.vue'
import * as app from '@/stores/app'
import type { Feed, RssCandidate } from '@/api/types'

const props = defineProps<{
  open: boolean
  /** 传 null 表示新增，传 Feed 表示编辑 */
  feed: Feed | null
}>()
const emit = defineEmits<{ close: []; saved: [] }>()

const url = ref('')
const title = ref('')
const category = ref('')
const busy = ref(false)
const error = ref('')
const confirmDelete = ref(false)
/** RSSHub Radar 探测出来的候选路由 */
const candidates = ref<RssCandidate[]>([])

const isRsshubUrl = (s: string) => s.toLowerCase().startsWith('rsshub:')

watch(
  () => [props.open, props.feed] as const,
  () => {
    error.value = ''
    confirmDelete.value = false
    candidates.value = []
    if (props.feed) {
      url.value = props.feed.url
      title.value = props.feed.title
      category.value = props.feed.category
    } else {
      url.value = ''
      title.value = ''
      category.value = ''
    }
  },
  { immediate: true },
)

const isEdit = () => props.feed != null

async function submit() {
  const raw = url.value.trim()
  if (!raw) {
    error.value = '请填写订阅源地址'
    return
  }
  // 候选已经列出来了，再回车表示「不用 RSSHub，按原地址添加」
  await runSubmit(raw, candidates.value.length === 0)
}

/** 选中一条 RSSHub 候选路由 */
async function pick(c: RssCandidate) {
  url.value = c.rsshub_url
  if (!title.value.trim()) title.value = c.title
  candidates.value = []
  await runSubmit(c.rsshub_url, false)
}

/** 忽略候选，直接按原地址添加 */
async function addDirectly() {
  candidates.value = []
  const raw = url.value.trim()
  if (raw) await runSubmit(raw, false)
}

/**
 * @param allowDiscover 新增时是否先用 RSSHub Radar 探测候选路由
 */
async function runSubmit(raw: string, allowDiscover: boolean) {
  busy.value = true
  error.value = ''
  try {
    if (isEdit()) {
      await app.updateFeed(props.feed!.id, {
        url: raw,
        title: title.value.trim() || undefined,
        category: category.value.trim(),
      })
    } else {
      if (allowDiscover && !isRsshubUrl(raw)) {
        // 探测失败（比如服务端没配 RSSHub）不影响原来的直接添加流程
        try {
          const res = await app.discoverFeeds(raw)
          if (res.candidates.length > 0) {
            candidates.value = res.candidates
            return
          }
        } catch {
          /* 忽略 */
        }
      }
      await app.addFeed(raw, title.value.trim() || undefined, category.value.trim())
    }
    emit('saved')
    emit('close')
  } catch (e) {
    let msg = (e as Error).message
    if (msg.includes('无法读取该订阅源')) {
      // 已经是 rsshub:// 地址时，问题出在路由本身，再让人去配 base_url 只会误导
      msg += isRsshubUrl(raw)
        ? '。该 RSSHub 路由可能不存在或参数不合法，可换一条候选路由试试。'
        : '。若该站点本身不提供 RSS，可在服务端 config.toml 配置 [rsshub].base_url 后用 RSSHub 订阅。'
    }
    error.value = msg
  } finally {
    busy.value = false
  }
}

async function doDelete() {
  if (!props.feed) return
  busy.value = true
  try {
    await app.removeFeed(props.feed.id)
    emit('saved')
    emit('close')
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <BaseDialog
    :open="props.open"
    :title="isEdit() ? '编辑订阅源' : '添加订阅源'"
    :width="520"
    @close="emit('close')"
  >
    <div class="field">
      <label>订阅源地址 (RSS / Atom / 任意网页)</label>
      <input
        v-model="url"
        placeholder="https://example.com/feed.xml"
        spellcheck="false"
        :disabled="busy"
        @keyup.enter="submit"
      />
      <p class="tip text-mute">
        不加 <code>https://</code> 也可以，服务端会自动补上。粘贴普通网页地址时，会用
        RSSHub 自动找出可用路由；也可以直接填 <code>rsshub://zhihu/daily</code>。
      </p>
    </div>

    <div v-if="candidates.length > 0" class="field">
      <label>这个网站可以用 RSSHub 订阅，选一个：</label>
      <ul class="candidates">
        <li v-for="c in candidates" :key="c.rsshub_url">
          <button class="candidate" :disabled="busy" @click="pick(c)">
            <span class="cand-title">{{ c.title }}</span>
            <code>{{ c.rsshub_url }}</code>
          </button>
        </li>
      </ul>
      <p class="tip text-mute">
        都不是想要的？可以
        <a href="#" @click.prevent="addDirectly">直接按原地址添加</a>
        <template v-if="candidates[0].docs">
          ，或先看
          <a :href="candidates[0].docs" target="_blank" rel="noopener">路由文档</a>
          确认合法参数
        </template>
      </p>
    </div>

    <div class="field">
      <label>显示名称（留空则自动取源里的标题）</label>
      <input v-model="title" placeholder="可留空" :disabled="busy" @keyup.enter="submit" />
    </div>

    <div class="field">
      <label>分组（留空为「未分类」，支持 a/b 多级）</label>
      <input v-model="category" placeholder="例如：技术 / 新闻" :disabled="busy" @keyup.enter="submit" />
    </div>

    <p v-if="error" class="err">{{ error }}</p>

    <div v-if="isEdit()" class="danger-zone">
      <template v-if="!confirmDelete">
        <button class="danger" :disabled="busy" @click="confirmDelete = true">删除这个订阅源</button>
      </template>
      <template v-else>
        <span class="text-dim">连同该源下的所有文章一起删除，确定？</span>
        <button class="danger" :disabled="busy" @click="doDelete">确定删除</button>
        <button :disabled="busy" @click="confirmDelete = false">取消</button>
      </template>
    </div>

    <template #footer>
      <button @click="emit('close')">取消</button>
      <button class="primary" :disabled="busy" @click="submit">
        {{ busy ? '处理中…' : isEdit() ? '保存' : '添加并拉取' }}
      </button>
    </template>
  </BaseDialog>
</template>

<style scoped>
.field {
  margin-bottom: 14px;
}
.tip {
  margin: 6px 0 0;
  font-size: 11.5px;
}
.tip code {
  font-family: var(--mono);
}
.candidates {
  list-style: none;
  margin: 6px 0 0;
  padding: 0;
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--border-soft);
  border-radius: 6px;
}
.candidate {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  width: 100%;
  padding: 7px 10px;
  border: 0;
  border-radius: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.candidate:hover {
  background: var(--bg-hover, rgba(127, 127, 127, 0.12));
}
.cand-title {
  font-size: 12.5px;
}
.candidate code {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--text-dim, #888);
}
.err {
  color: var(--danger);
  font-size: 12.5px;
  margin: 0 0 10px;
}
.danger-zone {
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--border-soft);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 12.5px;
}
</style>
