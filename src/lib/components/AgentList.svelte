<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, Bot, Search } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import type { Agent } from '$lib/types';
    import { appState } from '$lib/stores/appState.svelte';
    import CreateAgentModal from './CreateAgentModal.svelte';

    let agents = $state<Agent[]>([]);
    let loading = $state(true);
    let modalOpen = $state(false);
    let searchQuery = $state('');

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

    const filteredAgents = $derived(
        searchQuery.trim()
            ? agents.filter(a => a.name.toLowerCase().includes(searchQuery.trim().toLowerCase()))
            : agents
    );
</script>

<div class="flex flex-col h-full w-full bg-surface border-r border-border">
    <!-- Header -->
    <header class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-base font-semibold">角色列表</h2>
        <button onclick={() => modalOpen = true} class="flex items-center gap-1.5 px-3 py-1.5 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors">
            <Plus size={16} />
            <span>新建</span>
        </button>
    </header>

    <!-- Search -->
    <div class="px-4 py-3 border-b border-border">
        <div class="relative">
            <Search size={16} class="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary" />
            <input
                type="text"
                placeholder="搜索角色..."
                bind:value={searchQuery}
                class="w-full pl-9 pr-3 py-2 text-sm bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
        </div>
    </div>

    <!-- Agent List -->
    <div class="flex-1 overflow-y-auto">
        {#if loading}
            <div class="flex items-center justify-center h-full text-text-secondary text-sm">加载中...</div>
        {:else if filteredAgents.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary p-4">
                <Bot size={40} class="mb-3 opacity-50" />
                <p class="text-sm">{searchQuery ? '未找到匹配的角色' : '还没有创建任何角色'}</p>
                {#if !searchQuery}
                    <p class="text-xs mt-1">点击"新建"开始创建</p>
                {/if}
            </div>
        {:else}
            <div class="divide-y divide-border">
                {#each filteredAgents as agent}
                    <button
                        class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {appState.selectedAgentId === agent.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                        onclick={() => appState.selectAgent(agent.id)}
                    >
                        <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0">
                            {#if agent.avatar_path}
                                <img src={agent.avatar_path} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                            {:else}
                                <Bot size={20} />
                            {/if}
                        </div>
                        <div class="min-w-0 flex-1">
                            <h3 class="font-medium text-sm text-text truncate">{agent.name}</h3>
                            <p class="text-xs text-text-secondary truncate">{agent.model_name || '未配置模型'}</p>
                        </div>
                    </button>
                {/each}
            </div>
        {/if}
    </div>
</div>

<CreateAgentModal bind:open={modalOpen} onSuccess={loadAgents} />
