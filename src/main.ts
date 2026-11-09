import { invoke } from '@tauri-apps/api/core'
import { createApp } from 'vue'
import App from './App.vue'
import { installPlatformHttp, type NativeHttpArgs, type NativeHttpResult } from '@/api/client'
import './styles/main.css'

async function boot() {
  try {
    const info = await invoke<{ platform: string }>('app_info')
    // macOS WebView 会丢掉发往 http:// 的 fetch。启动前换成进程内的 HTTP 客户端。
    // Rust 端命令的入参是名为 args 的结构体，Tauri 要求把整个结构体放在 args 键下；
    // 展平成 url/method/... 会报 "missing required key args"。
    installPlatformHttp(info.platform, (args: NativeHttpArgs) =>
      invoke<NativeHttpResult>('http_request', { args }),
    )
  } catch {
    // 浏览器开发模式没有 app_info，继续用 fetch
  }
  createApp(App).mount('#app')
}

void boot()
