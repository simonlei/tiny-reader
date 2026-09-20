import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

/** 更新检查的结果状态 */
export type UpdateState =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'latest'; version: string }
  | { kind: 'available'; version: string; notes: string | null }
  | { kind: 'downloading' }
  | { kind: 'installing' }
  | { kind: 'error'; message: string }

/**
 * 检查是否有新版本。
 * 返回 null 表示当前已是最新版；否则返回新版本信息。
 */
export async function checkForUpdate(): Promise<{ version: string; notes: string | null } | null> {
  const update = await check()
  if (!update) return null
  return {
    version: update.version,
    notes: update.rawJson?.notes ?? null,
  }
}

/**
 * 下载并安装更新，安装完成后重启应用。
 * onProgress 可选，用于展示下载进度。
 */
export async function installUpdate(): Promise<void> {
  const update = await check()
  if (!update) return

  let downloaded = 0
  let contentLength = 0
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case 'Started':
        contentLength = event.data.contentLength ?? 0
        break
      case 'Progress':
        downloaded += event.data.chunkLength
        break
      case 'Finished':
        break
    }
  })

  if (contentLength > 0 && downloaded < contentLength) {
    throw new Error(`下载不完整：${downloaded}/${contentLength}`)
  }

  await relaunch()
}
