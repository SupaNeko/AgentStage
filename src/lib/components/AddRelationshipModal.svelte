<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { logger } from '$lib/logger';
    import { resolveAvatarUrl } from '$lib/utils';
    import { User, X } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    interface Props {
        open: boolean;
        observerId: string;
        existingFriendIds: string[];
        onClose: () => void;
        onAdded: () => void;
    }

    let { open, observerId, existingFriendIds, onClose, onAdded }: Props = $props();
    let selectedIds = $state<string[]>([]);
    let loading = $state(false);
    let mouseDownOnOverlay = $state(false);

    $effect(() => {
        if (open) {
            selectedIds = [];
            agentStore.loadAgents();
        }
    });

    const availableAgents = $derived(
        agentStore.agents.filter(a => a.id !== observerId && !existingFriendIds.includes(a.id) && !a.is_deleted)
    );

    function toggleAgent(id: string) {
        if (selectedIds.includes(id)) {
            selectedIds = selectedIds.filter(x => x !== id);
        } else {
            selectedIds = [...selectedIds, id];
        }
    }

    async function handleAdd() {
        if (selectedIds.length === 0) return;
        loading = true;
        try {
            await invoke('add_friendships', { observerId, targetIds: selectedIds });
            selectedIds = [];
            onAdded();
            onClose();
        } catch (err) {
            logger.error('Failed to add friendships:', err);
            toastStore.error('添加关系失败: ' + String(err));
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 modal-overlay"
        onmousedown={(e) => { mouseDownOnOverlay = e.target === e.currentTarget; }}
        onclick={(e) => { if (mouseDownOnOverlay && e.target === e.currentTarget) onClose(); mouseDownOnOverlay = false; }}
        role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-[28rem] max-w-full shadow-lg border border-border modal-card" onmousedown={() => mouseDownOnOverlay = false} onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-2">
                <h3 class="text-lg font-semibold">添加关系</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
            </div>
            <p class="text-xs text-text-secondary mb-4">添加关系是双向的，被添加的角色关系列表中也会增加该角色。</p>
            <div class="max-h-64 overflow-y-auto grid grid-cols-2 gap-2 mb-4">
                {#each availableAgents as agent}
                    <button
                        onclick={() => toggleAgent(agent.id)}
                        class="flex items-center gap-2 p-2 rounded-lg border border-border text-left transition-colors {selectedIds.includes(agent.id) ? 'bg-primary/10 border-primary' : 'hover:bg-bg'}"
                    >
                        <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                            {#if agent.avatar_path}
                                <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full object-cover" />
                            {:else}
                                <User size={16} />
                            {/if}
                        </div>
                        <span class="text-sm truncate">{agent.name}</span>
                    </button>
                {:else}
                    <p class="text-sm text-text-secondary col-span-2 py-4 text-center">没有可添加的角色</p>
                {/each}
            </div>
            <div class="flex gap-2">
                <button onclick={onClose} class="flex-1 py-2 bg-bg text-text-primary rounded-lg hover:bg-surface border border-border">
                    取消
                </button>
                <button
                    onclick={handleAdd}
                    disabled={selectedIds.length === 0 || loading}
                    class="flex-1 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark disabled:opacity-50 btn-primary"
                >
                    {loading ? '添加中...' : `添加 (${selectedIds.length})`}
                </button>
            </div>
        </div>
    </div>
{/if}
