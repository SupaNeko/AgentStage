<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount, untrack } from 'svelte';
    import { messageStore } from '$lib/stores/messageStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import MessageBubble from './MessageBubble.svelte';
    import { Send, MessageSquare, User, Settings, Bot, Clock } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import type { GroupMember, SessionConfig } from '$lib/types';
    import SessionSettingsPanel from './SessionSettingsPanel.svelte';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { formatTime } from '$lib/utils';

    interface Props {
        mode?: 'chat' | 'history';
    }
    let { mode = 'chat' }: Props = $props();

    let inputText = $state('');
    let inputBySession = $state<Map<string, string>>(new Map());
    let sending = $state(false);
    let typingAgents = $state<Set<string>>(new Set());
    let typingTimeouts = $state<Map<string, number>>(new Map());
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
        mode === 'chat'
            ? sessionStore.sessions.find((s) => s.id === sessionStore.selectedSessionId)
            : historyStore.sessions.find((s) => s.id === historyStore.selectedSessionId)
    );
    let currentAgentId = $state<string | undefined>(undefined);
    let isAgentTyping = $derived(
        currentAgentId != null && typingAgents.has(currentAgentId)
    );
    let isMessageLimitReached = $derived(
        sessionConfig != null &&
        sessionConfig.message_limit_enabled &&
        sessionConfig.agent_message_count >= sessionConfig.message_limit
    );

    async function loadSessionConfig(sessionId: string, sessionType: string) {
        try {
            const config = await invoke<SessionConfig>('get_session_config', {
                req: {
                    session_id: sessionId,
                    session_type: sessionType,
                },
            });
            sessionConfig = config;
        } catch (err) {
            logger.error('Failed to load session config:', err);
        }
    }

    $effect(() => {
        const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;
        prevMsgCount = 0;
        // 恢复该会话的输入内容
        if (id) {
            inputText = inputBySession.get(id) || '';
        } else {
            inputText = '';
        }
    });

    $effect(() => {
        const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;
        if (id) {
            if (pageIdx != null) {
                messageStore.loadMessages(id, pageIdx);
            } else {
                messageStore.loadMessages(id);
            }
            const session = untrack(() =>
                mode === 'chat'
                    ? sessionStore.sessions.find(s => s.id === id)
                    : historyStore.sessions.find(s => s.id === id)
            );
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
        const selectedId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
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
        const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        if (!sessionId || !sessionConfig) return;
        try {
            await invoke('reset_message_count', {
                req: {
                    session_id: sessionId,
                },
            });
            const session = mode === 'chat'
                ? sessionStore.sessions.find(s => s.id === sessionId)
                : historyStore.sessions.find(s => s.id === sessionId);
            if (session) {
                await loadSessionConfig(sessionId, session.session_type);
            }
        } catch (err) {
            logger.error('Failed to reset message count:', err);
        }
    }

    async function handleSend() {
        const content = inputText.trim();
        const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : undefined;
        logger.debug('[DEBUG ChatView.handleSend]', { sessionId, content, mode });
        if (!content || !sessionId) return;

        // History 模式：使用独立的 send_history_message 命令，不走 Scheduler + new_message 事件
        if (mode === 'history') {
            if (pageIdx == null) return;
            sending = true;
            inputText = '';
            const nextInput = new Map(inputBySession);
            nextInput.set(sessionId, '');
            inputBySession = nextInput;

            // 乐观更新：立即在 UI 中显示用户消息
            const optimisticMsg: import('$lib/types').Message = {
                id: 'optimistic-' + Date.now(),
                session_id: sessionId,
                sender_type: 'user',
                sender_id: 'user',
                sender_name: '用户',
                content,
                created_at: Date.now(),
                message_type: 'text',
            };
            messageStore.addMessage(optimisticMsg);

            try {
                await invoke('send_history_message', {
                    req: { session_id: sessionId, content, page_index: pageIdx },
                });
                logger.debug('[DEBUG ChatView.handleSend] history mode success');
                await messageStore.loadMessages(sessionId, pageIdx);
            } catch (err) {
                logger.error('[DEBUG ChatView.handleSend] history mode failed', { error: err });
                // 发送失败时移除乐观消息
                messageStore.removeMessage(sessionId, optimisticMsg.id);
            } finally {
                sending = false;
            }
            return;
        }

        // Chat 模式：保持原有逻辑（Scheduler + new_message 事件）
        sending = true;
        inputText = '';
        const nextInput = new Map(inputBySession);
        nextInput.set(sessionId, '');
        inputBySession = nextInput;

        // 乐观更新：立即在 UI 中显示用户消息
        const optimisticMsg: import('$lib/types').Message = {
            id: 'optimistic-' + Date.now(),
            session_id: sessionId,
            sender_type: 'user',
            sender_id: 'user',
            sender_name: '用户',
            content,
            created_at: Date.now(),
            message_type: 'text',
        };
        messageStore.addMessage(optimisticMsg);

        try {
            const req: Record<string, unknown> = {
                session_id: sessionId,
                content,
            };
            if (pageIdx != null) {
                req.page_index = pageIdx;
            }
            await invoke('send_user_message', { req });
            logger.debug('[DEBUG ChatView.handleSend] chat mode success');
            if (pageIdx != null) {
                await messageStore.loadMessages(sessionId, pageIdx);
            } else {
                await messageStore.loadMessages(sessionId);
            }
        } catch (err) {
            logger.debug('[DEBUG ChatView.handleSend] chat mode failed', { error: err });
            // 发送失败时移除乐观消息
            messageStore.removeMessage(sessionId, optimisticMsg.id);
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

        // Chat 模式才需要监听 new_message / agent_typing / agent_completed / agent_error
        // History 模式：消息更新通过 send_history_message 的返回值同步获取
        if (mode === 'chat') {
            listen('new_message', (event) => {
                const msg = event.payload as { session_id: string; sender_type?: string; content?: string; id?: string; page_index?: number } & Record<string, unknown>;
                logger.debug('[DEBUG ChatView.listen new_message]', { sessionId: msg.session_id, contentPreview: msg.content?.slice(0, 50) });

                // 将消息添加到对应会话的存储中（后台更新，绑定对应会话）
                const exists = messageStore.messagesBySession.get(msg.session_id)?.some((m) => m.id === msg.id);
                if (!exists) {
                    messageStore.addMessage(msg as unknown as import('$lib/types').Message);
                }

                // 当 agent 消息到达且用户正在查看该会话时，刷新 sessionConfig
                if (msg.session_id === messageStore.currentSessionId && msg.sender_type === 'agent') {
                    const session = untrack(() => sessionStore.sessions.find(s => s.id === msg.session_id));
                    if (session) {
                        loadSessionConfig(msg.session_id, session.session_type);
                    }
                }
            }).then((fn) => unlistenFns.push(fn));

            listen('agent_typing', (event) => {
                const payload = event.payload as { agent_id?: string };
                logger.debug('[DEBUG ChatView.listen agent_typing]', { agentId: payload.agent_id });
                if (payload.agent_id) {
                    const agentId = payload.agent_id;
                    const next = new Set(typingAgents);
                    next.add(agentId);
                    typingAgents = next;
                    // Defense: 5-minute timeout in case agent_completed is lost
                    const existing = typingTimeouts.get(agentId);
                    if (existing) clearTimeout(existing);
                    const t = setTimeout(() => {
                        const n = new Set(typingAgents);
                        n.delete(agentId);
                        typingAgents = n;
                        const nextTimeouts = new Map(typingTimeouts);
                        nextTimeouts.delete(agentId);
                        typingTimeouts = nextTimeouts;
                    }, 5 * 60 * 1000);
                    const nextTimeouts = new Map(typingTimeouts);
                    nextTimeouts.set(agentId, t);
                    typingTimeouts = nextTimeouts;
                }
            }).then((fn) => unlistenFns.push(fn));

            listen('agent_completed', (event) => {
                const payload = event.payload as { agent_id?: string };
                logger.debug('[DEBUG ChatView.listen agent_completed]', { agentId: payload.agent_id });
                if (payload.agent_id) {
                    const next = new Set(typingAgents);
                    next.delete(payload.agent_id);
                    typingAgents = next;
                    const existing = typingTimeouts.get(payload.agent_id);
                    if (existing) {
                        clearTimeout(existing);
                        const nextTimeouts = new Map(typingTimeouts);
                        nextTimeouts.delete(payload.agent_id);
                        typingTimeouts = nextTimeouts;
                    }
                }
            }).then((fn) => unlistenFns.push(fn));

            listen('agent_error', (event) => {
                const payload = event.payload as { agent_id?: string; error?: string };
                logger.debug('[DEBUG ChatView.listen agent_error]', { agentId: payload.agent_id });
                if (payload.agent_id) {
                    const next = new Set(typingAgents);
                    next.delete(payload.agent_id);
                    typingAgents = next;
                    const existing = typingTimeouts.get(payload.agent_id);
                    if (existing) {
                        clearTimeout(existing);
                        const nextTimeouts = new Map(typingTimeouts);
                        nextTimeouts.delete(payload.agent_id);
                        typingTimeouts = nextTimeouts;
                    }
                }
            }).then((fn) => unlistenFns.push(fn));
        }

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
            typingTimeouts.forEach((t) => clearTimeout(t));
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
        <header class="flex items-center justify-between px-6 py-4 border-b border-border bg-surface shrink-0 relative">
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
                {#if mode === 'history' && historyStore.chatPages.length > 0}
                    <div class="absolute left-1/2 -translate-x-1/2">
                        <select
                            value={historyStore.selectedPageIndex ?? 0}
                            onchange={(e) => {
                                const idx = Number((e.target as HTMLSelectElement).value);
                                historyStore.selectPage(idx);
                                if (historyStore.selectedSessionId) {
                                    messageStore.loadMessages(historyStore.selectedSessionId, idx);
                                }
                            }}
                            class="px-3 py-1.5 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 min-w-[180px]"
                        >
                            {#each historyStore.chatPages as page (page.page_index)}
                                <option value={page.page_index}>
                                    {page.name} #{page.page_index + 1} — {formatTime(page.updated_at)}
                                </option>
                            {/each}
                        </select>
                    </div>
                {/if}
                <button
                    onclick={() => settingsOpen = !settingsOpen}
                    class="p-2 hover:bg-bg rounded-lg text-text-secondary transition-colors"
                    title="会话配置"
                >
                    <Settings size={20} />
                </button>
            {:else}
                <h2 class="text-lg font-semibold text-text-secondary">
                    {mode === 'history' ? '选择一个会话查看历史' : '选择一个会话开始聊天'}
                </h2>
            {/if}
        </header>

        <!-- History mode banner -->
        {#if mode === 'history'}
            <div class="px-4 py-2 bg-amber-50 border-b border-amber-200 text-amber-800 text-sm flex items-center gap-2">
                <Clock size={16} />
                <span>当前处于<strong>历史会话</strong>模式。此处的对话仅基于当前会话的历史记录，不会影响其他会话，也不会触发跨会话的 Agent 互动。</span>
            </div>
        {/if}

        <!-- Message list -->
        {#if !selectedSession}
            <div class="flex-1 flex items-center justify-center text-text-secondary">
                <p>{mode === 'history' ? '选择一个会话查看历史' : '选择一个会话开始聊天'}</p>
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
                            <div class="flex flex-col max-w-[80%] items-start">
                                <div class="flex items-center gap-2 mb-1">
                                    <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                        {#if selectedSession.agent_avatar}
                                            <img src={selectedSession.agent_avatar} alt={selectedSession.agent_name || 'Agent'} class="w-full h-full object-cover" />
                                        {:else}
                                            <Bot size={16} />
                                        {/if}
                                    </div>
                                    <div class="flex flex-col justify-center h-8">
                                        <span class="text-xs text-text-secondary leading-none">{selectedSession.agent_name || 'Agent'}</span>
                                        <span class="text-[10px] text-text-secondary opacity-70 leading-none mt-0.5">正在输入中...</span>
                                    </div>
                                </div>
                                <div class="bg-surface border border-border rounded-2xl rounded-tl-sm px-4 py-2 text-text-secondary text-sm">
                                    <span class="inline-block animate-bounce">.</span>
                                    <span class="inline-block animate-bounce" style="animation-delay: 0.2s">.</span>
                                    <span class="inline-block animate-bounce" style="animation-delay: 0.4s">.</span>
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
                        value={inputText}
                        oninput={(e) => {
                            inputText = e.currentTarget.value;
                            const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
                            if (id) {
                                const next = new Map(inputBySession);
                                next.set(id, e.currentTarget.value);
                                inputBySession = next;
                            }
                        }}
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
