<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, Bot } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import type { Agent } from '$lib/types';
    import CreateAgentModal from './CreateAgentModal.svelte';
    
    let agents = $state<Agent[]>([]);
    let loading = $state(true);
    let modalOpen = $state(false);
    
    async function loadAgents() {
        loading = true;
        try {
            agents = await invoke('list_agents');
        } catch (err) {
            console.error('Failed to load agents:', err);
        } finally {
            loading = false;
        }
    }
    
    onMount(() => {
        loadAgents();
    });
</script>

<div class="flex flex-col h-full">
    <header class="flex items-center justify-between p-4 border-b border-border bg-surface">
        <h2 class="text-lg font-semibold">Agent 管理</h2>
        <button onclick={() => modalOpen = true} class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors">
            <Plus size={18} />
            <span>新建 Agent</span>
        </button>
    </header>
    
    <div class="flex-1 overflow-y-auto p-4">
        {#if loading}
            <div class="flex items-center justify-center h-full text-text-secondary">加载中...</div>
        {:else if agents.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                <Bot size={48} class="mb-4 opacity-50" />
                <p>还没有创建任何 Agent</p>
                <p class="text-sm mt-1">点击右上角"新建 Agent"开始创建</p>
            </div>
        {:else}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {#each agents as agent}
                    <div class="bg-surface border border-border rounded-xl p-4 hover:shadow-md transition-shadow cursor-pointer">
                        <div class="flex items-center gap-3 mb-3">
                            <div class="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                                {#if agent.avatar_path}
                                    <img src={agent.avatar_path} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                                {:else}
                                    <Bot size={24} />
                                {/if}
                            </div>
                            <div>
                                <h3 class="font-semibold text-text">{agent.name}</h3>
                                <p class="text-sm text-text-secondary">{agent.model_name || '未配置模型'}</p>
                            </div>
                        </div>
                        <p class="text-sm text-text-secondary line-clamp-2">{agent.simplified_persona}</p>
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>

<CreateAgentModal bind:open={modalOpen} onSuccess={loadAgents} />
