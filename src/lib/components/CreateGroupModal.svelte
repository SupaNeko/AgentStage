<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import type { Agent, Session } from '$lib/types';
    import { X, Users, Bot } from 'lucide-svelte';

    let groupName = $state('');
    let selectedAgentIds = $state<Set<string>>(new Set());
    let agents = $state<Agent[]>([]);
    let loadingAgents = $state(true);
    let creating = $state(false);

    async function loadAgents() {
        loadingAgents = true;
        try {
            agents = await invoke<Agent[]>('list_agents');
        } catch (err) {
            toastStore.show('加载角色列表失败', 'error');
        } finally {
            loadingAgents = false;
        }
    }

    onMount(() => { loadAgents(); });

    function toggleAgent(agentId: string) {
        const next = new Set(selectedAgentIds);
        if (next.has(agentId)) next.delete(agentId);
        else next.add(agentId);
        selectedAgentIds = next;
    }

    async function handleCreate() {
        const name = groupName.trim();
        if (!name) { toastStore.show('请输入群聊名称', 'error'); return; }
        if (selectedAgentIds.size < 2) { toastStore.show('请选择至少 2 个角色', 'error'); return; }
        creating = true;
        try {
            const session = await invoke<Session>('create_group_session', {
                req: { name, agent_ids: Array.from(selectedAgentIds) },
            });
            sessionStore.addSession(session);
            sessionStore.selectSession(session.id);
            appState.switchView('chat');
            toastStore.show('群聊创建成功', 'success', 2000);
            onclose?.();
        } catch (err) {
            toastStore.show(`创建失败：${err}`, 'error');
        } finally {
            creating = false;
        }
    }

    let { onclose }: { onclose?: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md max-h-[80vh] flex flex-col">
        <div class="flex items-center justify-between p-4 border-b border-border shrink-0">
            <h3 class="text-lg font-semibold flex items-center gap-2">
                <Users size={20} /> 新建群聊
            </h3>
            <button onclick={onclose} class="p-1 hover:bg-gray-100 rounded"><X size={20} /></button>
        </div>
        <div class="p-4 space-y-4 overflow-y-auto flex-1">
            <div>
                <label class="block text-sm font-medium mb-1">群聊名称</label>
                <input bind:value={groupName} placeholder="输入群聊名称..."
                    class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            <div>
                <label class="block text-sm font-medium mb-2">
                    选择角色 <span class="text-text-secondary font-normal">(至少 2 个)</span>
                </label>
                {#if loadingAgents}
                    <p class="text-sm text-text-secondary">加载中...</p>
                {:else}
                    <div class="space-y-1">
                        {#each agents as agent}
                            <label class="flex items-center gap-3 p-2 rounded-lg hover:bg-bg cursor-pointer">
                                <input type="checkbox" checked={selectedAgentIds.has(agent.id)}
                                    onchange={() => toggleAgent(agent.id)} />
                                <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                    {#if agent.avatar_path}
                                        <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full object-cover" />
                                    {:else}
                                        <Bot size={16} />
                                    {/if}
                                </div>
                                <span class="text-sm">{agent.name}</span>
                            </label>
                        {/each}
                    </div>
                {/if}
            </div>
        </div>
        <div class="p-4 border-t border-border flex justify-end gap-2 shrink-0">
            <button onclick={onclose} class="px-4 py-2 text-sm rounded-lg hover:bg-bg border border-border">取消</button>
            <button onclick={handleCreate}
                disabled={creating || selectedAgentIds.size < 2 || !groupName.trim()}
                class="px-4 py-2 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                {creating ? '创建中...' : '创建'}
            </button>
        </div>
    </div>
</div>
