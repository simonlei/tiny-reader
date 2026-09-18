<script setup lang="ts">
import { ref } from 'vue'
import BaseDialog from './BaseDialog.vue'
import { settings, apply } from '@/stores/settings'
import * as api from '@/api/client'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; saved: [] }>()

const testing = ref(false)
const testResult = ref<{ ok: boolean; msg: string } | null>(null)

async function test() {
  testing.value = true
  testResult.value = null
  apply()
  try {
    const alive = await api.health()
    if (!alive) {
      testResult.value = { ok: false, msg: '服务端没有响应' }
      return
    }
    const feeds = await api.listFeeds()
    testResult.value = { ok: true, msg: `连接成功，共 ${feeds.length} 个订阅源` }
  } catch (e) {
    testResult.value = { ok: false, msg: (e as Error).message }
  } finally {
    testing.value = false
  }
}

function save() {
  apply()
  emit('saved')
  emit('close')
}
</script>

<template>
  <BaseDialog :open="props.open" title="服务端设置" :width="520" @close="emit('close')">
    <div class="field">
      <label>服务端地址</label>
      <input v-model="settings.serverUrl" placeholder="http://127.0.0.1:8787" spellcheck="false" />
      <p class="tip text-mute">
        例：<code>http://127.0.0.1:8787</code>、<code>http://192.168.1.10:8787</code>。
        多台客户端填同一个地址即可共享阅读状态。
      </p>
    </div>

    <div class="field">
      <label>访问 token</label>
      <input v-model="settings.token" placeholder="填写服务端 config.toml 里的 auth.token" spellcheck="false" />
      <p class="tip text-mute">单用户模式下服务端只认这一个 token，改完记得保存。</p>
    </div>

    <div v-if="testResult" class="result" :class="testResult.ok ? 'ok' : 'bad'">
      {{ testResult.msg }}
    </div>

    <template #footer>
      <button :disabled="testing" @click="test">
        {{ testing ? '测试中…' : '测试连接' }}
      </button>
      <button class="primary" @click="save">保存</button>
    </template>
  </BaseDialog>
</template>

<style scoped>
.field {
  margin-bottom: 16px;
}
.tip {
  margin: 6px 0 0;
  font-size: 11.5px;
}
.tip code {
  font-family: var(--mono);
  background: var(--bg-alt);
  padding: 1px 4px;
  border-radius: 3px;
}
.result {
  font-size: 12.5px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  margin-bottom: 4px;
}
.result.ok {
  color: var(--ok);
  background: rgba(63, 185, 80, 0.1);
}
.result.bad {
  color: var(--danger);
  background: rgba(239, 91, 91, 0.1);
  word-break: break-all;
}
</style>
