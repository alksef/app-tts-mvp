<template>
  <Transition name="toast">
    <div v-if="visible" class="toast" :class="`toast-${type}`">
      <div class="toast-icon">{{ icon }}</div>
      <div class="toast-content">
        <div class="toast-message">{{ message }}</div>
      </div>
      <button class="toast-close" @click="hide">×</button>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';

const props = defineProps<{
  show: boolean;
  message: string;
  type?: 'error' | 'warning' | 'info';
  duration?: number;
}>();

const emit = defineEmits<{
  hide: [];
}>();

const visible = ref(false);
let hideTimer: number | null = null;

const icon = computed(() => {
  switch (props.type) {
    case 'error': return '⚠️';
    case 'warning': return '⚡';
    case 'info': return 'ℹ️';
    default: return '⚠️';
  }
});

const hide = () => {
  visible.value = false;
  emit('hide');
};

watch(() => props.show, (show) => {
  if (show) {
    visible.value = true;
    if (props.duration && props.duration > 0) {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = window.setTimeout(() => {
        hide();
      }, props.duration);
    }
  } else {
    visible.value = false;
  }
});
</script>

<style scoped>
.toast {
  position: fixed;
  top: 2rem;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 1rem 1.5rem;
  border-radius: 12px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
  z-index: 1000;
  min-width: 320px;
  max-width: 500px;
}

.toast-error {
  background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
  color: white;
}

.toast-warning {
  background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
  color: white;
}

.toast-info {
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
  color: white;
}

.toast-icon {
  font-size: 1.5rem;
  flex-shrink: 0;
}

.toast-content {
  flex: 1;
}

.toast-message {
  font-size: 0.9rem;
  font-weight: 500;
  line-height: 1.4;
}

.toast-close {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  color: white;
  font-size: 1.5rem;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s ease;
  flex-shrink: 0;
}

.toast-close:hover {
  background: rgba(255, 255, 255, 0.3);
}

.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(20px);
}
</style>
