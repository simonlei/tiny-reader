<script setup lang="ts">
import { ref, watch } from 'vue'
import BaseDialog from './BaseDialog.vue'
import * as app from '@/stores/app'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; done: [string] }>()

const xml = ref('')
const busy = ref(false)
const error = ref('')
const result = ref<{ added: number; skipped: number } | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

watch(
  () => props.open,
  () => {
    if (props.open) {
      error.value = ''
      result.value = null
    }
  },
)

function pickFile() {
  fileInput.value?.click()
}

async function onFile(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  try {
    xml.value = await file.text()
    error.value = ''
  } catch (err) {
    error.value = `读取文件失败：${(err as Error).message}`
  } finally {
    input.value = ''
  }
}

async function submit() {
  if (!xml.value.trim()) {
    error.value = '请先选择 .opml 文件或粘贴 OPML 内容'
    return
  }
  busy.value = true
  error.value = ''
  result.value = null
  try {
    const res = await app.importOpml(xml.value)
    result.value = { added: res.added, skipped: res.skipped }
    emit('done', `导入完成：新增 ${res.added} 个，跳过 ${res.skipped} 个`)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <BaseDialog :open="props.open" title="导入 OPML" :width="560" @close="emit('close')">
    <div class="row">
      <button :disabled="busy" @click="pickFile">选择 .opml 文件…</button>
      <span class="text-mute">或直接把 OPML 内容粘到下面</span>
      <input ref="fileInput" type="file" accept=".opml,.xml,text/xml" hidden @change="onFile" />
    </div>

    <textarea
      v-model="xml"
      rows="10"
      spellcheck="false"
      placeholder='<opml version="2.0"><body><outline type="rss" text="示例" xmlUrl="https://example.com/feed.xml"/></body></opml>'
      class="mono"
    />

    <p v-if="error" class="err">{{ error }}</p>
    <p v-if="result" class="ok">
      新增 {{ result.added }} 个订阅源，跳过 {{ result.skipped }} 个（已存在或地址不合法）。
      新源的标题和文章正在后台拉取，稍等几秒即可看到。
    </p>

    <template #footer>
      <button @click="emit('close')">{{ result ? '完成' : '取消' }}</button>
      <button v-if="!result" class="primary" :disabled="busy" @click="submit">
        {{ busy ? '导入中…' : '开始导入' }}
      </button>
    </template>
  </BaseDialog>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
  font-size: 12.5px;
}
textarea {
  font-size: 12px;
  resize: vertical;
  line-height: 1.5;
}
.err {
  color: var(--danger);
  font-size: 12.5px;
  margin: 10px 0 0;
}
.ok {
  color: var(--ok);
  font-size: 12.5px;
  margin: 10px 0 0;
}
</style>
