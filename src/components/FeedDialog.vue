<script setup lang="ts">
import { ref, watch } from 'vue'
import BaseDialog from './BaseDialog.vue'
import * as app from '@/stores/app'
import type { Feed } from '@/api/types'

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

watch(
  () => [props.open, props.feed] as const,
  () => {
    error.value = ''
    confirmDelete.value = false
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
  if (!url.value.trim()) {
    error.value = '请填写订阅源地址'
    return
  }
  busy.value = true
  error.value = ''
  try {
    if (isEdit()) {
      await app.updateFeed(props.feed!.id, {
        url: url.value.trim(),
        title: title.value.trim() || undefined,
        category: category.value.trim(),
      })
    } else {
      await app.addFeed(url.value.trim(), title.value.trim() || undefined, category.value.trim())
    }
    emit('saved')
    emit('close')
  } catch (e) {
    error.value = (e as Error).message
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
      <label>订阅源地址 (RSS / Atom)</label>
      <input
        v-model="url"
        placeholder="https://example.com/feed.xml"
        spellcheck="false"
        :disabled="busy"
        @keyup.enter="submit"
      />
      <p class="tip text-mute">不加 <code>https://</code> 也可以，服务端会自动补上。</p>
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
