<script setup lang="ts">
defineProps<{
  open: boolean
  title: string
  width?: number
}>()

const emit = defineEmits<{ close: [] }>()
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay" @click.self="emit('close')">
      <div class="dialog" :style="{ width: (width ?? 460) + 'px' }">
        <header class="dlg-head">
          <h3>{{ title }}</h3>
          <button class="ghost icon-btn" title="关闭 (Esc)" @click="emit('close')">✕</button>
        </header>
        <div class="dlg-body">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="dlg-foot">
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(6, 8, 11, 0.6);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 9vh;
  z-index: 100;
}
.dialog {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5);
  max-width: calc(100vw - 32px);
  max-height: 82vh;
  display: flex;
  flex-direction: column;
}
.dlg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-soft);
}
.dlg-head h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}
.dlg-body {
  padding: 16px;
  overflow-y: auto;
}
.dlg-foot {
  padding: 12px 16px;
  border-top: 1px solid var(--border-soft);
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.icon-btn {
  padding: 2px 8px;
  font-size: 13px;
  line-height: 1.4;
}
</style>
