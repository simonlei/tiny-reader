<script setup lang="ts">
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import BaseDialog from './BaseDialog.vue'
import { settings, apply } from '@/stores/settings'
import * as api from '@/api/client'
import { checkForUpdate, installUpdate } from '@/lib/updater'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; saved: [] }>()

const currentVersion = ref('')

// 弹窗打开时拉取一次当前版本（后端已有 app_info 命令）
watch(
  () => props.open,
  async (open) => {
    if (!open || currentVersion.value) return
    try {
      const info = await invoke<{ version: string }>('app_info')
      currentVersion.value = info.version
    } catch {
      // 非 Tauri 环境（浏览器开发）下忽略
    }
  },
  { immediate: true }
)

const testing = ref(false)
const testResult = ref<{ ok: boolean; msg: string } | null>(null)

const checking = ref(false)
const updating = ref(false)
const updateResult = ref<{ ok: boolean; msg: string } | null>(null)
const pendingUpdate = ref<{ version: string; notes: string | null } | null>(null)

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

async function checkUpdate() {
  checking.value = true
  updateResult.value = null
  pendingUpdate.value = null
  try {
    const info = await checkForUpdate()
    if (!info) {
      updateResult.value = { ok: true, msg: `当前已是最新版 ${currentVersion.value || ''}` }
    } else {
      pendingUpdate.value = info
      updateResult.value = { ok: true, msg: `发现新版本 ${info.version}` }
    }
  } catch (e) {
    updateResult.value = { ok: false, msg: (e as Error).message }
  } finally {
    checking.value = false
  }
}

async function doUpdate() {
  if (!pendingUpdate.value) return
  updating.value = true
  updateResult.value = null
  try {
    await installUpdate()
    // 安装成功后应用会重启，这里通常不会走到
  } catch (e) {
    updateResult.value = { ok: false, msg: `更新失败：${(e as Error).message}` }
    updating.value = false
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

    <div class="divider"></div>

    <div class="field">
      <label>版本与更新</label>
      <div class="update-row">
        <span class="ver text-mute">当前版本 {{ currentVersion || '—' }}</span>
        <button :disabled="checking || updating" @click="checkUpdate">
          {{ checking ? '检查中…' : '检查更新' }}
        </button>
      </div>
      <p class="tip text-mute">从发布通道检查是否有新版本，支持 Windows 与 macOS 应用内升级。</p>
    </div>

    <div v-if="updateResult" class="result" :class="updateResult.ok ? 'ok' : 'bad'">
      {{ updateResult.msg }}
    </div>

    <div v-if="pendingUpdate" class="update-notes">
      <div class="notes-title">更新说明</div>
      <div class="notes-body">{{ pendingUpdate.notes || '（本次发布无说明）' }}</div>
      <button class="primary" :disabled="updating" @click="doUpdate">
        {{ updating ? '下载并安装中…' : '下载并安装更新' }}
      </button>
      <p class="tip text-mute">安装完成后应用会自动重启。</p>
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
.divider {
  height: 1px;
  background: var(--border);
  margin: 4px 0 16px;
}
.update-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.ver {
  font-size: 12.5px;
}
.update-notes {
  margin-top: 10px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-alt);
}
.notes-title {
  font-size: 12px;
  margin-bottom: 6px;
}
.notes-body {
  font-size: 12px;
  white-space: pre-wrap;
  max-height: 160px;
  overflow-y: auto;
  margin-bottom: 10px;
  color: var(--text-mute, inherit);
}
</style>
