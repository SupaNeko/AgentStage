<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Bot, Loader2, Import } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { logger } from '$lib/logger';
    import type { Agent } from '$lib/types';

    let { open = $bindable(false), currentAgentId, onImport }: { open: boolean; currentAgentId: string; onImport: (agent: Agent) => void } = $props();

    let agents = $state<Agent[]>([]);
    let loading = $state(false);
    let error = $state('');

    async function loadAgents() {
        loading = true;
        error = '';
        try {
            const result = await invoke<Agent[]>('list_agents');
            // 排除当前正在编辑的角色和已删除角色，只保留有模型配置的角色
            agents = result.filter(a =>
                a.id !== currentAgentId &&
                !a.is_deleted &&
                a.model_provider
            );
        } catch (err: any) {
            logger.error('Failed to load agents:', err);
            error = '加载角色列表失败';
        } finally {
            loading = false;
        }
    }

    function handleSelect(agent: Agent) {
        onImport(agent);
        open = false;
    }

    $effect(() => {
        if (open) {
            loadAgents();
        }
    });
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 modal-overlay" onclick={() => open = false} role="dialog" aria-modal="true">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md max-h-[70vh] flex flex-col modal-card" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold flex items-center gap-2">
                <Import size={18} />
                从其他角色导入模型配置
            </h3>
            <button onclick={() => open = false} class="p-1 hover:bg-gray-100 rounded" aria-label="关闭">
                <X size={20} />
            </button>
        </div>

        <div class="flex-1 overflow-y-auto p-4">
            {#if loading}
                <div class="flex items-center justify-center py-8 text-text-secondary">
                    <Loader2 size={20} class="animate-spin mr-2" />
                    加载中...
                </div>
            {:else if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {:else if agents.length === 0}
                <div class="text-center py-8 text-text-secondary text-sm">
                    <Bot size={32} class="mx-auto mb-2 opacity-40" />
                    <p>没有其他可导入配置的角色</p>
                </div>
            {:else}
                <div class="space-y-2">
                    {#each agents as agent}
                        <button
                            type="button"
                            onclick={() => handleSelect(agent)}
                            class="w-full flex items-center gap-3 p-3 rounded-lg border border-border hover:bg-bg hover:border-primary/30 transition-colors text-left"
                        >
                            <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                {#if agent.avatar_path}
                                    <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full object-cover" />
                                {:else}
                                    <Bot size={18} />
                                {/if}
                            </div>
                            <div class="flex-1 min-w-0">
                                <p class="font-medium text-sm truncate">{agent.name}</p>
                                <p class="text-xs text-text-secondary truncate">
                                    {agent.model_provider} / {agent.model_name || '未配置模型'}
                                </p>
                            </div>
                            <span class="text-xs text-text-secondary shrink-0">
                                {agent.temperature} / {agent.max_tokens}
                            </span>
                        </button>
                    {/each}
                </div>
            {/if}
        </div>

        <div class="p-4 border-t border-border">
            <button onclick={() => open = false} class="w-full px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors text-sm">
                取消
            </button>
        </div>
    </div>
</div>
{/if}
