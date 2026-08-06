import { toastStore } from '$lib/stores/toastStore.svelte';

const DEDUPE_MS = 2000;
let lastToastAt = 0;

/** 自动保存成功提示：全局 2 秒去重，避免连续输入时 toast 刷屏 */
export function toastAutoSaved() {
    const now = Date.now();
    if (now - lastToastAt < DEDUPE_MS) return;
    lastToastAt = now;
    toastStore.success('已自动保存', 1500);
}
