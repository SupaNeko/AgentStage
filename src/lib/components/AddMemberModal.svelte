<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { logger } from '$lib/logger';
    import { resolveAvatarUrl } from '$lib/utils';
    import { User, X } from 'lucide-svelte';

    interface Props {
        open: boolean;
        sessionId: string;
        existingMemberIds: string[];
        onClose: () => void;
        onAdded: () => void;
    }

    let { open, sessionId, existingMemberIds, onClose, onAdded }: Props = $props();
    let selectedIds = $state<string[]>([]);
    let loading = $state(false);

    $effect(() => {
        if (open) {
            agentStore.loadAgents();
        }
    });

    const availableAgents = $derived(
        agentStore.agents.filter(a => !existingMemberIds.includes(a.id) && !a.is_deleted)
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
            for (const agentId of selectedIds) {
                await invoke('add_group_member', { req: { session_id: sessionId, agent_id: agentId } });
            }
            selectedIds = [];
            onAdded();
            onClose();
        } catch (err) {
            logger.error('Failed to add members:', err);
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 modal-overlay" onclick={onClose}>
        <div class="bg-surface rounded-xl p-6 w-96 max-w-full shadow-lg border border-border modal-card" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold">添加成员</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
            </div>
            <div class="max-h-64 overflow-y-auto space-y-1 mb-4">
                {#each availableAgents as agent}
                    <button
                        onclick={() => toggleAgent(agent.id)}
                        class="w-full flex items-center gap-3 p-2 rounded-lg hover:bg-bg text-left {selectedIds.includes(agent.id) ? 'bg-primary/10 ring-1 ring-primary' : ''}"
                    >
                        <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                            {#if agent.avatar_path}
                                <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full object-cover" />
                            {:else}
                                <User size={16} />
                            {/if}
                        </div>
                        <span class="text-sm">{agent.name}</span>
                    </button>
                {:else}
                    <p class="text-sm text-text-secondary p-2">没有可添加的角色</p>
                {/each}
            </div>
            <button
                onclick={handleAdd}
                disabled={selectedIds.length === 0 || loading}
                class="w-full py-2 bg-primary text-white rounded-lg hover:bg-primary-dark disabled:opacity-50 btn-primary"
            >
                {loading ? '添加中...' : `添加 (${selectedIds.length})`}
            </button>
        </div>
    </div>
{/if}
