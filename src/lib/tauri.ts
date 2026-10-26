import { invoke } from '@tauri-apps/api/core'

/** 启动本机图片代理。不在 Tauri 里时会抛错，调用方退回服务端代拉。 */
export async function setLocalMediaProxy(hosts: string[], referer: string): Promise<string> {
  return invoke<string>('set_media_proxy', { hosts, referer })
}

/** 用系统默认浏览器打开链接；不在 Tauri 里时退回 window.open */
export async function openExternal(url: string | null | undefined) {
  const u = (url || '').trim()
  if (!u) return
  try {
    await invoke('open_url', { url: u })
  } catch {
    window.open(u, '_blank', 'noopener,noreferrer')
  }
}
