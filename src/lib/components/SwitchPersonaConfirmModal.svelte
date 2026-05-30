<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { X } from 'lucide-svelte';

    let {
        personaName,
        onConfirm,
        onClose,
    }: {
        personaName: string;
        onConfirm: (reset: boolean) => void;
        onClose: () => void;
    } = $props();

    let resetting = $state(false);

    async function handleSwitchOnly() {
        onConfirm(false);
    }

    async function handleSwitchAndReset() {
        resetting = true;
        try {
            await invoke('reset_all_sessions');
            await sessionStore.loadSessions();
            toastStore.show('所有会话已重置', 'success');
        } catch (e) {
            toastStore.show('重置会话失败: ' + String(e), 'error');
            resetting = false;
            return;
        }
        onConfirm(true);
    }
</script>

<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md p-6">
        <div class="flex items-center justify-between mb-4">
            <h3 class="text-lg font-semibold text-text">切换人设</h3>
            <button onclick={onClose} class="text-text-secondary hover:text-text">
                <X size={20} />
            </button>
        </div>

        <div class="mb-6 space-y-3">
            <p class="text-sm text-text">
                你正在切换到人设 <span class="font-medium text-primary">"{personaName}"</span>。
            </p>
            <div class="bg-yellow-50 border border-yellow-200 rounded-lg p-3 text-sm text-yellow-800">
                <p class="font-medium mb-1">⚠️ 注意</p>
                <p>切换人设后，角色可能会基于之前的对话历史产生认知混乱（例如继续用旧人设理解你）。</p>
            </div>
            <p class="text-sm text-text-secondary">
                建议一键重置所有会话，让角色在新人设下重新开始认识您。
            </p>
        </div>

        <div class="flex justify-end gap-3">
            <button
                onclick={onClose}
                class="px-4 py-2 rounded-lg text-text-secondary hover:bg-gray-100 text-sm"
            >
                取消
            </button>
            <button
                onclick={handleSwitchOnly}
                class="px-4 py-2 rounded-lg border border-border text-text hover:bg-gray-50 text-sm"
            >
                仅切换
            </button>
            <button
                onclick={handleSwitchAndReset}
                disabled={resetting}
                class="px-4 py-2 rounded-lg bg-primary text-white hover:bg-primary-dark disabled:opacity-50 text-sm btn-primary"
            >
                {resetting ? '重置中...' : '切换并重置所有会话'}
            </button>
        </div>
    </div>
</div>
