import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 在移动端 / 部分环境下会注入 TAURI_DEV_HOST，用于局域网调试
const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [vue()],

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  // 让 Tauri 的启动日志不被 vite 清屏冲掉
  clearScreen: false,

  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 5174,
        }
      : undefined,
    watch: {
      // 避免监听 Rust 目录导致频繁重编译
      ignored: ['**/src-tauri/**', '**/server/**'],
    },
  },

  envPrefix: ['VITE_', 'TAURI_ENV_'],

  build: {
    // Tauri 使用系统 WebView：Windows 用 Chromium(WebView2)，macOS 用 WebKit
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    // 调试构建不压缩，方便排查
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
