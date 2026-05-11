<script lang="ts">
    import { onMount } from 'svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { formatTime } from '$lib/utils';
    import { Search, MessageSquare, Plus } from 'lucide-svelte';
    import CreateGroupModal from './CreateGroupModal.svelte';

    let showCreateGroup = $state(false);

    onMount(() => {
        sessionStore.loadSessions();
    });

    function handleSessionClick(sessionId: string) {
        sessionStore.selectSession(sessionId);
        appState.switchView('chat');
    }
</script>

<div class="flex flex-col h-full w-full bg-surface border-r border-border">
    <!-- Header -->
    <header class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-base font-semibold">会话列表</h2>
        <button onclick={() => showCreateGroup = true}
            class="p-1.5 hover:bg-bg rounded-lg text-text-secondary hover:text-text transition-colors" title="新建群聊">
            <Plus size={18} />
        </button>
    </header>

    <!-- Search -->
    <div class="px-4 py-3 border-b border-border">
        <div class="relative">
            <Search size={16} class="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary" />
            <input
                type="text"
                placeholder="搜索会话..."
                class="w-full pl-9 pr-3 py-2 text-sm bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
        </div>
    </div>

    <!-- Session List -->
    <div class="flex-1 overflow-y-auto">
        {#if sessionStore.sessions.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary p-4">
                <MessageSquare size={40} class="mb-3 opacity-50" />
                <p class="text-sm">还没有会话，去角色列表创建一个吧</p>
            </div>
        {:else}
            <div class="divide-y divide-border">
                {#each sessionStore.sessions as session}
                    <button
                        class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {sessionStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                        onclick={() => handleSessionClick(session.id)}
                    >
                        <!-- Avatar -->
                        <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                            {#if session.agent_avatar || session.group_avatar}
                                <img
                                    src={session.agent_avatar || session.group_avatar}
                                    alt={session.agent_name || session.group_name || '会话'}
                                    class="w-full h-full object-cover"
                                />
                            {:else}
                                <MessageSquare size={20} />
                            {/if}
                        </div>

                        <!-- Content -->
                        <div class="min-w-0 flex-1">
                            <div class="flex items-center justify-between">
                                <h3 class="font-medium text-sm text-text truncate">
                                    {session.agent_name || session.group_name || '未命名会话'}
                                </h3>
                                {#if session.last_message_at}
                                    <span class="text-xs text-text-secondary shrink-0 ml-2">
                                        {formatTime(session.last_message_at)}
                                    </span>
                                {/if}
                            </div>
                            <div class="flex items-center justify-between mt-0.5">
                                <p class="text-xs text-text-secondary truncate flex-1">
                                    {session.last_message_preview || '暂无消息'}
                                </p>
                                {#if session.unread_count > 0}
                                    <span class="ml-2 min-w-[1.25rem] h-5 px-1.5 flex items-center justify-center bg-primary text-white text-xs font-medium rounded-full shrink-0">
                                        {session.unread_count > 99 ? '99+' : session.unread_count}
                                    </span>
                                {/if}
                            </div>
                        </div>
                    </button>
                {/each}
            </div>
        {/if}
    </div>
</div>

{#if showCreateGroup}
    <CreateGroupModal onclose={() => showCreateGroup = false} />
{/if}
