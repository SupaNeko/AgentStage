<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';
    import { messageStore } from '$lib/stores/messageStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import MessageBubble from './MessageBubble.svelte';
    import { Send, MessageSquare, User, Settings, Bot } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import type { GroupMember, SessionConfig } from '$lib/types';
    import SessionSettingsPanel from './SessionSettingsPanel.svelte';

    let inputText = $state('');
    let sending = $state(false);
    let isAgentTyping = $state(false);
    let members = $state<GroupMember[]>([]);
    let loadingMembers = $state(false);
    let settingsOpen = $state(false);
    let sessionConfig = $state<SessionConfig | null>(null);
    let messageListEl: HTMLDivElement | null = $state(null);
    let prevMsgCount = $state(0);

    function scrollToBottom() {
        if (messageListEl) {
            messageListEl.scrollTop = messageListEl.scrollHeight;
        }
    }

    let selectedSession = $derived(
        sessionStore.sessions.find((s) => s.id === sessionStore.selectedSessionId)
    );
    let currentAgentId = $state<string | undefined>(undefined);
    let isMessageLimitReached = $derived(
        sessionConfig != null &&
        sessionConfig.message_limit_enabled &&
        sessionConfig.agent_message_count >= sessionConfig.message_limit
    );

    async function loadSessionConfig(sessionId: string, sessionType: string) {
        try {
            const config = await invoke<SessionConfig>('get_session_config', {
                sessionId,
                sessionType,
            });
            sessionConfig = config;
        } catch (err) {
            logger.error('Failed to load session config:', err);
        }
    }

    $effect(() => {
        const id = sessionStore.selectedSessionId;
        prevMsgCount = 0;
    });

    $effect(() => {
        const id = sessionStore.selectedSessionId;
        if (id) {
            messageStore.loadMessages(id);
            const session = sessionStore.sessions.find(s => s.id === id);
            currentAgentId = session?.agent_id;
            if (session) {
                loadSessionConfig(id, session.session_type);
            }
            if (session?.session_type === 'group') {
                loadingMembers = true;
                invoke<GroupMember[]>('get_group_members', { sessionId: id })
                    .then((data) => { members = data; })
                    .catch((err) => logger.error('Failed to load group members:', err))
                    .finally(() => { loadingMembers = false; });
            } else {
                members = [];
            }
        } else {
            messageStore.setSessionId(null);
            currentAgentId = undefined;
            members = [];
            sessionConfig = null;
        }
    });

    $effect(() => {
        const msgs = messageStore.messages;
        const selectedId = sessionStore.selectedSessionId;
        const currentId = messageStore.currentSessionId;

        if (!selectedId || currentId !== selectedId) return;

        const diff = msgs.length - prevMsgCount;
        prevMsgCount = msgs.length;

        if (diff > 1) {
            scrollToBottom();
        } else if (diff === 1) {
            const lastMsg = msgs[msgs.length - 1];
            if (lastMsg.sender_type === 'user') {
                scrollToBottom();
            }
        }
    });

    async function handleResetMessageCount() {
        if (!sessionStore.selectedSessionId || !sessionConfig) return;
        try {
            await invoke('reset_message_count', {
                sessionId: sessionStore.selectedSessionId,
            });
            const session = sessionStore.sessions.find(s => s.id === sessionStore.selectedSessionId);
            if (session) {
                await loadSessionConfig(sessionStore.selectedSessionId, session.session_type);
            }
        } catch (err) {
            logger.error('Failed to reset message count:', err);
        }
    }

    async function handleSend() {
        const content = inputText.trim();
        logger.debug('[DEBUG ChatView.handleSend]', { sessionId: sessionStore.selectedSessionId, content });
        if (!content || !sessionStore.selectedSessionId) return;

        sending = true;
        inputText = '';

        // 乐观更新：立即在 UI 中显示用户消息
        const optimisticMsg: import('$lib/types').Message = {
            id: 'optimistic-' + Date.now(),
            session_id: sessionStore.selectedSessionId,
            sender_type: 'user',
            sender_id: 'user',
            sender_name: '用户',
            content,
            created_at: Date.now(),
            message_type: 'text',
        };
        messageStore.addMessage(optimisticMsg);

        try {
            await invoke('send_user_message', {
                req: {
                    session_id: sessionStore.selectedSessionId,
                    content,
                },
            });
            logger.debug('[DEBUG ChatView.handleSend] success');
            await messageStore.loadMessages(sessionStore.selectedSessionId);
        } catch (err) {
            logger.debug('[DEBUG ChatView.handleSend] failed', { error: err });
            // 发送失败时移除乐观消息
            messageStore.messages = messageStore.messages.filter((m) => m.id !== optimisticMsg.id);
        } finally {
            sending = false;
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        logger.debug('[DEBUG ChatView.handleKeydown]', { key: e.key, shiftKey: e.shiftKey });
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    }

    onMount(() => {
        const unlistenFns: (() => void)[] = [];

        listen('new_message', (event) => {
            const msg = event.payload as { session_id: string; content?: string; id?: string } & Record<string, unknown>;
            logger.debug('[DEBUG ChatView.listen new_message]', { sessionId: msg.session_id, contentPreview: msg.content?.slice(0, 50) });
            if (msg.session_id === sessionStore.selectedSessionId) {
                // 去重：如果消息已存在，不重复添加
                const exists = messageStore.messages.some((m) => m.id === msg.id);
                if (!exists) {
                    messageStore.addMessage(msg as unknown as import('$lib/types').Message);
                }
            }
        }).then((fn) => unlistenFns.push(fn));

        listen('agent_typing', (event) => {
            const payload = event.payload as { agent_id?: string };
            logger.debug('[DEBUG ChatView.listen agent_typing]', { agentId: payload.agent_id });
            if (currentAgentId === payload.agent_id) {
                isAgentTyping = true;
            }
        }).then((fn) => unlistenFns.push(fn));

        listen('agent_completed', (event) => {
            const payload = event.payload as { agent_id?: string };
            logger.debug('[DEBUG ChatView.listen agent_completed]', { agentId: payload.agent_id });
            if (currentAgentId === payload.agent_id) {
                isAgentTyping = false;
            }
        }).then((fn) => unlistenFns.push(fn));

        function handleDocumentClick(e: MouseEvent) {
            if (settingsOpen) {
                const target = e.target as HTMLElement;
                if (!target.closest('.session-settings-panel') && !target.closest('[title="会话配置"]')) {
                    settingsOpen = false;
                }
            }
        }

        document.addEventListener('click', handleDocumentClick);
        unlistenFns.push(() => document.removeEventListener('click', handleDocumentClick));

        return () => {
            unlistenFns.forEach((fn) => fn());
        };
    });
</script>

<div class="flex h-full bg-bg relative">
    <SessionSettingsPanel
        open={settingsOpen}
        sessionId={selectedSession?.id ?? ''}
        sessionType={selectedSession?.session_type ?? ''}
        {members}
        onClose={() => settingsOpen = false}
        onMembersChange={() => {
            if (selectedSession?.session_type === 'group') {
                loadingMembers = true;
                invoke<GroupMember[]>('get_group_members', { sessionId: selectedSession.id })
                    .then((data) => { members = data; })
                    .catch((err) => logger.error('Failed to reload members:', err))
                    .finally(() => { loadingMembers = false; });
            }
        }}
    />
    <div class="flex flex-col flex-1 min-w-0">
        <!-- Header -->
        <header class="flex items-center justify-between px-6 py-4 border-b border-border bg-surface shrink-0">
            {#if selectedSession}
                <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                        {#if selectedSession.agent_avatar || selectedSession.group_avatar}
                            <img
                                src={selectedSession.agent_avatar || selectedSession.group_avatar}
                                alt={selectedSession.agent_name || selectedSession.group_name || '会话'}
                                class="w-full h-full object-cover"
                            />
                        {:else}
                            <MessageSquare size={20} />
                        {/if}
                    </div>
                    <div>
                        <h2 class="text-lg font-semibold">
                            {selectedSession.agent_name || selectedSession.group_name || '未命名会话'}
                        </h2>
                    </div>
                </div>
                <button
                    onclick={() => settingsOpen = !settingsOpen}
                    class="p-2 hover:bg-bg rounded-lg text-text-secondary transition-colors"
                    title="会话配置"
                >
                    <Settings size={20} />
                </button>
            {:else}
                <h2 class="text-lg font-semibold text-text-secondary">选择一个会话开始聊天</h2>
            {/if}
        </header>

        <!-- Message list -->
        {#if !selectedSession}
            <div class="flex-1 flex items-center justify-center text-text-secondary">
                <p>选择一个会话开始聊天</p>
            </div>
        {:else}
            <div class="flex-1 overflow-y-auto" bind:this={messageListEl} data-testid="message-list">
                {#if messageStore.messages.length === 0 && !isAgentTyping}
                    <div class="flex items-center justify-center h-full text-text-secondary p-4">
                        <p>还没有消息，发送第一条消息吧</p>
                    </div>
                {:else}
                <div class="py-4 space-y-2">
                    {#each messageStore.messages as message (message.id)}
                        <div
                            class="flex px-4 {message.sender_type === 'user' ? 'justify-end' : 'justify-start'}"
                        >
                            <MessageBubble
                                {message}
                                isMe={message.sender_type === 'user'}
                                senderName={message.sender_name || '未知'}
                            />
                        </div>
                    {/each}
                    {#if isAgentTyping}
                        <div class="flex px-4 justify-start" data-testid="typing-indicator">
                            <div class="flex gap-2 max-w-[80%]">
                                <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                    {#if selectedSession.agent_avatar}
                                        <img src={selectedSession.agent_avatar} alt={selectedSession.agent_name || 'Agent'} class="w-full h-full object-cover" />
                                    {:else}
                                        <Bot size={16} />
                                    {/if}
                                </div>
                                <div class="flex flex-col">
                                    <div class="text-xs text-text-secondary mb-1">{selectedSession.agent_name || 'Agent'}</div>
                                    <div class="bg-surface border border-border rounded-2xl rounded-tl-sm px-4 py-2 text-text-secondary text-sm">
                                        正在输入中...
                                    </div>
                                </div>
                            </div>
                        </div>
                    {/if}
                </div>
                {/if}
            </div>

            <!-- Message limit warning -->
            {#if isMessageLimitReached}
                <div class="shrink-0 px-4 py-2 bg-yellow-50 border-b border-yellow-200 text-yellow-800 text-sm flex items-center justify-between">
                    <span>已达到消息上限，角色不再主动回复</span>
                    <button
                        onclick={handleResetMessageCount}
                        class="px-3 py-1 bg-yellow-100 hover:bg-yellow-200 rounded-md text-yellow-900 text-sm font-medium transition-colors"
                    >
                        重置限制
                    </button>
                </div>
            {/if}

            <!-- Input area -->
            <div class="shrink-0 border-t border-border p-4 bg-surface">
                <div class="flex items-end gap-2">
                    <textarea
                        bind:value={inputText}
                        onkeydown={handleKeydown}
                        placeholder="输入消息..."
                        rows={3}
                        class="flex-1 resize-none px-4 py-2.5 bg-bg border border-border rounded-xl focus:outline-none focus:ring-2 focus:ring-primary/20 max-h-32"
                    ></textarea>
                    <button
                        onclick={handleSend}
                        disabled={sending || !inputText.trim()}
                        class="p-2.5 bg-primary text-white rounded-xl hover:bg-primary-dark transition-colors disabled:opacity-50 shrink-0"
                    >
                        <Send size={18} />
                    </button>
                </div>
            </div>
        {/if}
    </div>
    {#if selectedSession?.session_type === 'group'}
        <aside class="w-56 border-l border-border bg-surface flex flex-col shrink-0">
            <div class="p-3 border-b border-border">
                <h3 class="text-sm font-medium">成员 ({members.length})</h3>
            </div>
            <div class="flex-1 overflow-y-auto p-2 space-y-1">
                {#if loadingMembers}
                    <p class="text-xs text-text-secondary p-2">加载中...</p>
                {:else}
                    {#each members as member}
                        <div class="flex items-center gap-2 p-2 rounded-lg hover:bg-bg">
                            <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                {#if member.avatar_path}
                                    <img src={member.avatar_path} alt={member.name} class="w-full h-full object-cover" />
                                {:else}
                                    <User size={16} />
                                {/if}
                            </div>
                            <span class="text-sm truncate">{member.name}</span>
                        </div>
                    {/each}
                {/if}
            </div>
        </aside>
    {/if}
</div>
