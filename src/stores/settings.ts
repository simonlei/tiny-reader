import { computed, reactive, watch } from 'vue'
import { configure } from '@/api/client'

const KEY = 'tiny-reader.settings.v1'
const DEFAULT_URL = 'http://127.0.0.1:8787'

export interface SettingsState {
  serverUrl: string
  token: string
}

function load(): SettingsState {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) {
      const p = JSON.parse(raw) as Partial<SettingsState>
      return {
        serverUrl: p.serverUrl || DEFAULT_URL,
        token: p.token || '',
      }
    }
  } catch {
    /* localStorage 不可用时用默认值 */
  }
  return { serverUrl: DEFAULT_URL, token: '' }
}

export const settings = reactive<SettingsState>(load())

/** 是否是首次运行（还没有填过 token） */
export const needsSetup = computed(() => !settings.serverUrl || !settings.token)

/** 把设置同步给 API 客户端 */
export function apply() {
  configure({ serverUrl: settings.serverUrl, token: settings.token })
}

watch(
  settings,
  () => {
    try {
      localStorage.setItem(KEY, JSON.stringify(settings))
    } catch {
      /* 忽略写入失败 */
    }
    apply()
  },
  { deep: true },
)

apply()
