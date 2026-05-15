<script lang="ts">
    import { onMount } from 'svelte';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { formatTime } from '$lib/utils';
    import { MessageSquare, ChevronDown, ChevronRight } from 'lucide-svelte';

    let expandedPrivate = $state(true);
    let expandedGroup = $state(true);

    onMount(() => {
        historyStore.loadSessions();
    });

    function handleSessionClick(sessionId: string) {
        historyStore.selectSession(sessionId);
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
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                        >
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if session.agent_avatar}
                                    <img src={session.agent_avatar} alt={session.agent_name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{session.agent_name || '未命名'}</h3>
                                    {#if session.last_message_at}
                                        <span class="text-xs text-text-secondary shrink-0 ml-2">{formatTime(session.last_message_at)}</span>
                                    {/if}
                                </div>
                                <p class="text-xs text-text-secondary truncate">{session.last_message_preview || '暂无消息'}</p>
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
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                        >
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if session.group_avatar}
                                    <img src={session.group_avatar} alt={session.group_name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{session.group_name || '未命名群聊'}</h3>
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
