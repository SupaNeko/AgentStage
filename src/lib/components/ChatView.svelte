<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount, untrack } from 'svelte';
    import { messageStore } from '$lib/stores/messageStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import MessageBubble from './MessageBubble.svelte';
    import { Send, MessageSquare, User, Settings, Bot, Clock } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import type { GroupMember, SessionConfig, Session, Message } from '$lib/types';
    import SessionSettingsPanel from './SessionSettingsPanel.svelte';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { formatTime, resolveAvatarUrl } from '$lib/utils';

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
    let scrollPositions = $state<Map<string, number>>(new Map());
    let isFirstLoad = $state(false);

    function scrollToBottom() {
        if (messageListEl) {
            messageListEl.scrollTop = messageListEl.scrollHeight;
        }
    }

    function isNearBottom(): boolean {
        if (!messageListEl) return true;
        const threshold = 80;
        return messageListEl.scrollHeight - messageListEl.scrollTop - messageListEl.clientHeight <= threshold;
    }

    let selectedSession = $derived(
        mode === 'chat'
            ? sessionStore.sessions.find((s) => s.id === sessionStore.selectedSessionId)
            : historyStore.sessions.find((s) => s.id === historyStore.selectedSessionId)
    );
    let currentAgentId = $state<string | undefined>(undefined);
    let isAgentAgentPrivate = $derived(
        selectedSession != null &&
        selectedSession.session_type === 'private' &&
        !selectedSession.participants.some(p => p.participant_type === 'user')
    );
    let isDeletedAgentPrivate = $derived(
        selectedSession != null &&
        selectedSession.session_type === 'private' &&
        selectedSession.participants.some(p => p.participant_type === 'agent' && p.is_deleted)
    );
    let displayedTypingAgents = $derived(
        (() => {
            if (!selectedSession) return [] as string[];
            if (selectedSession.session_type === 'group') return [] as string[];
            const hasUser = selectedSession.participants.some(p => p.participant_type === 'user');
            if (hasUser) {
                if (currentAgentId && typingAgents.has(currentAgentId)) {
                    return [currentAgentId];
                }
                return [] as string[];
            }
            return Array.from(typingAgents).filter(id =>
                selectedSession.participants.some(p => p.participant_id === id)
            );
        })()
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

    function isOnRightSide(message: Message, session: Session | undefined): boolean {
        if (message.sender_type === 'user') return true;
        if (!session) return false;
        const isAgentAgent = session.session_type === 'private' && !session.participants.some(p => p.participant_type === 'user');
        if (isAgentAgent && message.sender_type === 'agent') {
            const agentIds = session.participants
                .filter(p => p.participant_type === 'agent')
                .map(p => p.participant_id)
                .sort();
            return message.sender_id === agentIds[1];
        }
        return false;
    }

    function getHeaderDisplay(session: Session) {
        const userParticipant = session.participants.find(p => p.participant_type === 'user');
        const agentParticipants = session.participants.filter(p => p.participant_type === 'agent');

        if (session.session_type === 'group') {
            return { type: 'group' as const, name: session.group_name || '群聊', avatar: session.group_avatar || null, agents: [] as typeof agentParticipants };
        }

        if (userParticipant) {
            const agent = agentParticipants[0];
            return { type: 'single' as const, name: agent?.name || '未命名', avatar: agent?.avatar_path || null, agents: [] as typeof agentParticipants };
        }

        return {
            type: 'agent-agent' as const,
            name: `${agentParticipants[0]?.name || 'Agent1'}-${agentParticipants[1]?.name || 'Agent2'}`,
            avatar: null as string | null,
            agents: agentParticipants,
        };
    }

    $effect(() => {
        const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;
        prevMsgCount = 0;
        isFirstLoad = true;
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
            // History 模式：等待 pageIndex 准备好再加载消息，避免用旧 pageIndex 导致闪烁/空消息
            if (mode === 'history' && pageIdx == null) {
                return;
            }
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
            const agentParticipant = session?.participants.find(p => p.participant_type === 'agent');
            currentAgentId = agentParticipant?.participant_id;
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

        if (isFirstLoad) {
            isFirstLoad = false;
            if (diff > 0 && messageListEl) {
                const saved = scrollPositions.get(selectedId);
                if (saved != null) {
                    messageListEl.scrollTop = saved;
                } else {
                    messageListEl.scrollTop = messageListEl.scrollHeight;
                }
            }
            return;
        }

        if (diff > 1) {
            scrollToBottom();
        } else if (diff === 1 && isNearBottom()) {
            scrollToBottom();
        }
    });

    // Typing indicator 出现时，如果用户之前在底部（scrollPositions 中没有记录），自动滚动到底部
    // 双重 requestAnimationFrame：第一层等 DOM 更新，第二层等浏览器 layout/paint 完成后滚动
    $effect(() => {
        const count = displayedTypingAgents.length;
        const currentId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        if (count > 0 && currentId && !scrollPositions.has(currentId)) {
            requestAnimationFrame(() => {
                requestAnimationFrame(() => {
                    scrollToBottom();
                });
            });
        }
    });

    async function handleResetMessageCount() {
        const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        if (!sessionId || !sessionConfig) return;
        try {
            // 乐观更新：立即清空计数，让提示条立刻消失
            sessionConfig = { ...sessionConfig, agent_message_count: 0 };
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

        // History 模式不更新会话列表预览，避免影响当前 page 的预览显示
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

        // 乐观更新会话列表预览（在 invoke 之前，避免被 App.svelte 的 new_message 事件覆盖）
        sessionStore.updateSessionPreview(sessionId, content, Date.now());
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
                const msg = event.payload as { session_id: string; sender_type?: string; sender_id?: string; sender_name?: string; content?: string; id?: string; page_index?: number } & Record<string, unknown>;
                logger.debug('[DEBUG ChatView.listen new_message]', { sessionId: msg.session_id, contentPreview: msg.content?.slice(0, 50), pageIndex: msg.page_index });

                // 防御：忽略不属于当前查看会话的消息
                if (msg.session_id !== messageStore.currentSessionId) {
                    return;
                }

                // 检查消息是否属于当前查看的页面
                const session = sessionStore.sessions.find(s => s.id === msg.session_id);
                const currentPage = session?.current_chat_page ?? 0;
                const isCurrentPage = msg.page_index === undefined || msg.page_index === currentPage;

                // 只将当前页的消息追加到消息列表
                if (isCurrentPage) {
                    const exists = messageStore.messagesBySession.get(msg.session_id)?.some((m) => m.id === msg.id);
                    if (!exists) {
                        // 补全 sender_name（后端 emit 的消息可能缺少 sender_name）
                        if (!msg.sender_name && session) {
                            const p = session.participants.find(p => p.participant_id === msg.sender_id);
                            if (p) {
                                (msg as Record<string, unknown>).sender_name = p.name;
                            }
                            if (!msg.sender_name) {
                                (msg as Record<string, unknown>).sender_name = msg.sender_type === 'user' ? '用户' : '未知';
                            }
                        }
                        messageStore.addMessage(msg as unknown as import('$lib/types').Message);
                    }
                }

                // 当 agent 消息到达当前会话时，刷新 sessionConfig
                if (msg.sender_type === 'agent') {
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
        groupAvatar={selectedSession?.group_avatar ?? null}
        {mode}
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
                {@const header = getHeaderDisplay(selectedSession!)}
                <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                        {#if header.type === 'agent-agent' && header.agents.length >= 2}
                            <div class="relative w-full h-full">
                                <div class="absolute left-0 top-0 w-1/2 h-full overflow-hidden">
                                    {#if header.agents[0]?.avatar_path}
                                        <img src={resolveAvatarUrl(header.agents[0].avatar_path)} alt="" class="w-10 h-10 object-cover" style="object-position: left center;" />
                                    {:else}
                                        <div class="w-10 h-10 bg-primary/20 flex items-center justify-center text-primary text-xs font-bold" style="padding-right: 0.5rem;">
                                            {header.agents[0]?.name?.charAt(0) || 'A'}
                                        </div>
                                    {/if}
                                </div>
                                <div class="absolute right-0 top-0 w-1/2 h-full overflow-hidden border-l-2 border-white">
                                    {#if header.agents[1]?.avatar_path}
                                        <img src={resolveAvatarUrl(header.agents[1].avatar_path)} alt="" class="w-10 h-10 object-cover" style="object-position: right center;" />
                                    {:else}
                                        <div class="w-10 h-10 bg-secondary/20 flex items-center justify-center text-secondary text-xs font-bold" style="padding-left: 0.5rem;">
                                            {header.agents[1]?.name?.charAt(0) || 'B'}
                                        </div>
                                    {/if}
                                </div>
                            </div>
                        {:else if header.avatar}
                            <img
                                src={resolveAvatarUrl(header.avatar)}
                                alt={header.name}
                                class="w-full h-full object-cover"
                            />
                        {:else}
                            <MessageSquare size={20} />
                        {/if}
                    </div>
                    <div>
                        <h2 class="text-lg font-semibold">
                            {header.name}
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
            <div class="flex-1 overflow-y-auto"
                 bind:this={messageListEl}
                 data-testid="message-list"
                 onscroll={() => {
                     if (!messageListEl) return;
                     const currentId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
                     if (!currentId) return;
                     if (isNearBottom()) {
                         const next = new Map(scrollPositions);
                         next.delete(currentId);
                         scrollPositions = next;
                     } else {
                         const next = new Map(scrollPositions);
                         next.set(currentId, messageListEl.scrollTop);
                         scrollPositions = next;
                     }
                 }}
            >
                {#if messageStore.messages.length === 0 && displayedTypingAgents.length === 0}
                    <div class="flex items-center justify-center h-full text-text-secondary p-4">
                        <p>还没有消息，发送第一条消息吧</p>
                    </div>
                {:else}
                <div class="py-4 space-y-2">
                    {#each messageStore.messages as message (message.id)}
                        {@const rightSide = isOnRightSide(message, selectedSession)}
                        <div
                            class="flex px-4 {rightSide ? 'justify-end' : 'justify-start'}"
                        >
                            <MessageBubble
                                {message}
                                isMe={rightSide}
                                senderName={message.sender_name || '未知'}
                            />
                        </div>
                    {/each}
                    {#each displayedTypingAgents as agentId (agentId)}
                        {@const agent = selectedSession.participants.find(p => p.participant_id === agentId)}
                        {@const isRight = selectedSession && selectedSession.session_type === 'private' && !selectedSession.participants.some(p => p.participant_type === 'user') && selectedSession.participants.filter(p => p.participant_type === 'agent').map(p => p.participant_id).sort()[1] === agentId}
                        <div class="flex px-4 {isRight ? 'justify-end' : 'justify-start'}" data-testid="typing-indicator">
                            <div class="flex flex-col max-w-[80%] {isRight ? 'items-end' : 'items-start'}">
                                <div class="flex items-center gap-2 mb-1 {isRight ? 'flex-row-reverse' : ''}">
                                    <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                        {#if agent?.avatar_path}
                                            <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name || 'Agent'} class="w-full h-full object-cover" />
                                        {:else}
                                            <Bot size={16} />
                                        {/if}
                                    </div>
                                    <div class="flex flex-col justify-center h-8 {isRight ? 'items-end' : 'items-start'}">
                                        <span class="text-xs text-text-secondary leading-none">{agent?.name || 'Agent'}</span>
                                        <span class="text-[10px] text-text-secondary opacity-70 leading-none mt-0.5">正在输入中...</span>
                                    </div>
                                </div>
                                <div class="bg-surface border border-border rounded-2xl {isRight ? 'rounded-tr-sm' : 'rounded-tl-sm'} px-4 py-2 text-text-secondary text-sm">
                                    <span class="inline-block animate-bounce">.</span>
                                    <span class="inline-block animate-bounce" style="animation-delay: 0.2s">.</span>
                                    <span class="inline-block animate-bounce" style="animation-delay: 0.4s">.</span>
                                </div>
                            </div>
                        </div>
                    {/each}
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
            {#if selectedSession?.session_type === 'group' && selectedSession?.is_dissolved}
                <div class="shrink-0 border-t border-border p-4 bg-surface text-center text-sm text-text-secondary">
                    该群聊已解散，无法发送消息
                </div>
            {:else if isAgentAgentPrivate}
                <div class="shrink-0 border-t border-border p-4 bg-surface text-center text-sm text-text-secondary">
                    此会话为 Agent-Agent 私聊，不支持用户直接发送消息
                </div>
            {:else if isDeletedAgentPrivate}
                <div class="shrink-0 border-t border-border p-4 bg-surface text-center text-sm text-text-secondary">
                    该角色已删除，无法发送消息
                </div>
            {:else}
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
                                    <img src={resolveAvatarUrl(member.avatar_path)} alt={member.name} class="w-full h-full object-cover" />
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
