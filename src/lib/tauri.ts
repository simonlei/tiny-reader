import { invoke } from '@tauri-apps/api/core'

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
