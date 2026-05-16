<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { formatTime, resolveAvatarUrl } from '$lib/utils';
    import { MessageSquare, ChevronDown, ChevronRight, Trash2 } from 'lucide-svelte';
    import type { Session } from '$lib/types';

    let expandedPrivate = $state(true);
    let expandedGroup = $state(true);
    let contextMenuOpen = $state(false);
    let contextMenuX = $state(0);
    let contextMenuY = $state(0);
    let contextSessionId = $state<string | null>(null);

    onMount(() => {
        historyStore.loadSessions();
    });

    function handleContextMenu(e: MouseEvent, sessionId: string) {
        e.preventDefault();
        contextSessionId = sessionId;
        contextMenuX = e.clientX;
        contextMenuY = e.clientY;
        contextMenuOpen = true;
    }

    function closeContextMenu() {
        contextMenuOpen = false;
        contextSessionId = null;
    }

    async function handlePermanentDelete() {
        if (!contextSessionId) return;
        if (!confirm('确定要彻底删除此群聊吗？所有历史记录将被移除，此操作不可恢复。')) {
            closeContextMenu();
            return;
        }
        try {
            await invoke('delete_session', { id: contextSessionId });
            await historyStore.loadSessions();
            if (historyStore.selectedSessionId === contextSessionId) {
                historyStore.selectedSessionId = null;
            }
            toastStore.show('群聊已彻底删除', 'success', 2000);
        } catch (err) {
            toastStore.show('删除失败: ' + String(err), 'error', 5000);
        } finally {
            closeContextMenu();
        }
    }

    function handleSessionClick(sessionId: string) {
        historyStore.selectSession(sessionId);
    }

    function getSessionDisplay(session: Session) {
        const userParticipant = session.participants.find(p => p.participant_type === 'user');
        const agentParticipants = session.participants.filter(p => p.participant_type === 'agent');

        if (session.session_type === 'group') {
            return {
                avatar: session.group_avatar || null,
                agents: [] as typeof agentParticipants,
                name: session.group_name || '群聊',
                isAgentAgent: false,
            };
        }

        if (userParticipant) {
            const agent = agentParticipants[0];
            return {
                avatar: agent?.avatar_path || null,
                agents: [] as typeof agentParticipants,
                name: agent?.name || '未命名',
                isAgentAgent: false,
            };
        }

        return {
            avatar: null as string | null,
            agents: agentParticipants,
            name: `${agentParticipants[0]?.name || 'Agent1'}-${agentParticipants[1]?.name || 'Agent2'}`,
            isAgentAgent: true,
        };
    }
</script>

<div class="flex flex-col h-full w-full bg-surface border-r border-border">
    <header class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-base font-semibold">历史会话</h2>
    </header>

    <div class="flex-1 overflow-y-auto">
        {#if historyStore.sessions.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary p-4">
                <MessageSquare size={40} class="mb-3 opacity-50" />
                <p class="text-sm">还没有会话</p>
            </div>
        {:else}
            <!-- 私聊分组 -->
            <div class="border-b border-border">
                <button
                    onclick={() => expandedPrivate = !expandedPrivate}
                    class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium transition-colors"
                >
                    <span>私聊</span>
                    {#if expandedPrivate}
                        <ChevronDown size={16} />
                    {:else}
                        <ChevronRight size={16} />
                    {/if}
                </button>
                {#if expandedPrivate}
                    {#each historyStore.groupedSessions.private as session (session.id)}
                        {@const display = getSessionDisplay(session)}
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                        >
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if display.isAgentAgent && display.agents.length >= 2}
                                    <div class="relative w-full h-full">
                                        <div class="absolute left-0 top-0 w-1/2 h-full overflow-hidden">
                                            {#if display.agents[0]?.avatar_path}
                                                <img src={resolveAvatarUrl(display.agents[0].avatar_path)} alt="" class="w-10 h-10 object-cover" style="object-position: left center;" />
                                            {:else}
                                                <div class="w-10 h-10 bg-primary/20 flex items-center justify-center text-primary text-xs font-bold" style="padding-right: 0.5rem;">
                                                    {display.agents[0]?.name?.charAt(0) || 'A'}
                                                </div>
                                            {/if}
                                        </div>
                                        <div class="absolute right-0 top-0 w-1/2 h-full overflow-hidden border-l-2 border-white">
                                            {#if display.agents[1]?.avatar_path}
                                                <img src={resolveAvatarUrl(display.agents[1].avatar_path)} alt="" class="w-10 h-10 object-cover" style="object-position: right center;" />
                                            {:else}
                                                <div class="w-10 h-10 bg-secondary/20 flex items-center justify-center text-secondary text-xs font-bold" style="padding-left: 0.5rem;">
                                                    {display.agents[1]?.name?.charAt(0) || 'B'}
                                                </div>
                                            {/if}
                                        </div>
                                    </div>
                                {:else if display.avatar}
                                    <img src={resolveAvatarUrl(display.avatar)} alt={display.name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{display.name}</h3>
                                    {#if session.last_message_at}
                                        <span class="text-xs text-text-secondary shrink-0 ml-2">{formatTime(session.last_message_at)}</span>
                                    {/if}
                                </div>
                                <!-- 历史会话标签不显示最后一条消息预览 -->
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>

            <!-- 群聊分组 -->
            <div>
                <button
                    onclick={() => expandedGroup = !expandedGroup}
                    class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium transition-colors"
                >
                    <span>群聊</span>
                    {#if expandedGroup}
                        <ChevronDown size={16} />
                    {:else}
                        <ChevronRight size={16} />
                    {/if}
                </button>
                {#if expandedGroup}
                    {#each historyStore.groupedSessions.group as session (session.id)}
                        {@const display = getSessionDisplay(session)}
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                            oncontextmenu={(e) => handleContextMenu(e, session.id)}
                        >
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if display.avatar}
                                    <img src={resolveAvatarUrl(display.avatar)} alt={display.name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{display.name}</h3>
                                    {#if session.last_message_at}
                                        <span class="text-xs text-text-secondary shrink-0 ml-2">{formatTime(session.last_message_at)}</span>
                                    {/if}
                                </div>
                                <p class="text-xs text-text-secondary truncate">&nbsp;</p>
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>
        {/if}
    </div>
</div>

<!-- Context Menu -->
{#if contextMenuOpen}
    <div class="fixed inset-0 z-40" onclick={closeContextMenu} oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}></div>
    <div class="fixed z-50 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[140px]"
         style="left: {contextMenuX}px; top: {contextMenuY}px;">
        <button
            onclick={handlePermanentDelete}
            class="w-full px-4 py-2 text-left text-sm text-red-500 hover:bg-bg flex items-center gap-2"
        >
            <Trash2 size={14} />
            彻底删除
        </button>
    </div>
{/if}
