<script lang="ts">
    import './styles.css';
    import { onMount } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { invoke } from '@tauri-apps/api/core';
    import LeftNav from '$lib/components/LeftNav.svelte';
    import AgentList from '$lib/components/AgentList.svelte';
    import AgentDetail from '$lib/components/AgentDetail.svelte';
    import SessionList from '$lib/components/SessionList.svelte';
    import ChatView from '$lib/components/ChatView.svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { messageStore } from '$lib/stores/messageStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import SettingsPanel from '$lib/components/SettingsPanel.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import HistorySessionList from '$lib/components/HistorySessionList.svelte';
    import ProfileView from '$lib/components/ProfileView.svelte';
    import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
    import { ProgressBarStatus } from '@tauri-apps/api/window';

    let isWindowFocused = true;
    let hasNotification = false;

    onMount(() => {
        settingsStore.load();
        const unlistenFns: (() => void)[] = [];
        const win = getCurrentWebviewWindow();

        // Track window focus state
        win.isFocused().then(focused => {
            isWindowFocused = focused;
        });
        
        win.onFocusChanged(({ payload: focused }) => {
            isWindowFocused = focused;
            if (focused && hasNotification) {
                // User activated the app: cancel notification state
                win.setProgressBar({ status: ProgressBarStatus.None }).catch(() => {});
                invoke('clear_flash').catch(() => {});
                hasNotification = false;
            }
        }).then((fn) => unlistenFns.push(fn));

        listen('new_message', (event) => {
            const msg = event.payload as { session_id: string; content?: string; created_at?: number; id?: string; page_index?: number };
            logger.debug('[DEBUG App.listen new_message]', { sessionId: msg.session_id, contentPreview: msg.content?.slice(0, 50), pageIndex: msg.page_index });
            // 更新会话列表（未读数、预览）
            sessionStore.sessions = sessionStore.sessions.map((s) => {
                if (s.id !== msg.session_id) return s;
                
                // 未读语义：当前页面有新消息
                const isCurrentPage = msg.page_index !== undefined 
                                      && msg.page_index === s.current_chat_page;
                
                // 当前会话且当前页面 = 用户正在看，不增加未读
                const isCurrentlyViewing = msg.session_id === sessionStore.selectedSessionId
                                            && appState.currentView === 'chat'
                                            && isCurrentPage;
                
                return {
                    ...s,
                    unread_count: (isCurrentPage && !isCurrentlyViewing) 
                        ? s.unread_count + 1 
                        : s.unread_count,
                    last_message_preview: msg.content || s.last_message_preview,
                    last_message_at: msg.created_at || Date.now(),
                };
            });

            // 窗口未聚焦或不是当前查看的会话时，任务栏闪烁提醒
            const isCurrentSession = msg.session_id === sessionStore.selectedSessionId && appState.currentView === 'chat';
            if (!isWindowFocused || !isCurrentSession) {
                if (!hasNotification) {
                    hasNotification = true;
                    // Flash taskbar 3 times
                    invoke('flash_taskbar', { count: 3 }).catch(() => {});
                    // Keep taskbar button lit (indeterminate progress state)
                    win.setProgressBar({ status: ProgressBarStatus.Indeterminate }).catch(() => {});
                }
            }

            // 如果会话不存在（如 Agent-Agent 新建会话），刷新列表
            const exists = sessionStore.sessions.some(s => s.id === msg.session_id);
            if (!exists) {
                sessionStore.loadSessions();
            }
        }).then((fn) => unlistenFns.push(fn));

        listen('system_notice', (event) => {
            const payload = event.payload as { content?: string };
            logger.debug('[DEBUG App.listen system_notice]', { content: payload.content });
            toastStore.show(payload.content || '系统通知', 'info', true, 10000);
        }).then((fn) => unlistenFns.push(fn));

        listen('agent_error', (event) => {
            const payload = event.payload as { error?: string; message?: string };
            logger.error('[DEBUG App.listen agent_error]', { error: payload.error, message: payload.message });
            toastStore.show(payload.error || payload.message || '角色回复失败，将在稍后重试', 'error');
        }).then((fn) => unlistenFns.push(fn));

        listen('agent_completed', (event) => {
            const payload = event.payload as { agent_id?: string; session_id?: string };
            logger.debug('[DEBUG App.listen agent_completed]', { agentId: payload.agent_id });
            // 消息追加由 ChatView 的 new_message 事件处理，此处不再兜底刷新
        }).then((fn) => unlistenFns.push(fn));

        return () => {
            unlistenFns.forEach((fn) => fn());
        };
    });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-bg">
    <!-- Left Navigation -->
    <LeftNav />

    <!-- Middle Panel -->
    {#if appState.currentView !== 'profile'}
        <div class="w-72 shrink-0 bg-surface border-r border-border">
            {#if appState.currentView === 'agents'}
                <AgentList />
            {:else if appState.currentView === 'chat'}
                <SessionList />
            {:else}
                <HistorySessionList />
            {/if}
        </div>
    {/if}

    <!-- Main Content Area -->
    <main class="flex-1 min-w-0 bg-bg">
        {#if appState.currentView === 'agents'}
            <AgentDetail />
        {:else if appState.currentView === 'chat'}
            <ChatView />
        {:else if appState.currentView === 'profile'}
            <ProfileView />
        {:else}
            <ChatView mode="history" />
        {/if}
    </main>
</div>

<!-- Toast Notifications -->
<div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 flex flex-col gap-2 pointer-events-none">
    {#each toastStore.items as toast (toast.id)}
        <div
            class="pointer-events-auto relative overflow-hidden rounded-lg shadow-lg text-sm font-medium flex flex-col transition-all animate-in slide-in-from-top-2 {toast.type === 'error' ? 'bg-red-500 text-white' : toast.type === 'success' ? 'bg-green-500 text-white' : 'bg-surface text-text border border-border'}"
        >
            <div class="flex items-center gap-2 px-4 py-2.5">
                {#if toast.type === 'error'}
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" x2="9" y1="9" y2="15"/><line x1="9" x2="15" y1="9" y2="15"/></svg>
                {:else if toast.type === 'success'}
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
                {/if}
                <span>{toast.message}</span>
                <button onclick={() => toastStore.remove(toast.id)} class="ml-1 opacity-70 hover:opacity-100">
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
                </button>
            </div>
            {#if toast.autoDismiss}
                <div class="h-0.5 w-full bg-gray-200/30">
                    <div class="h-full {toast.type === 'error' ? 'bg-red-300' : toast.type === 'success' ? 'bg-green-300' : 'bg-blue-400'} transition-all duration-100 ease-linear" style="width: {toast.progress}%"></div>
                </div>
            {/if}
        </div>
    {/each}
</div>

{#if appState.settingsOpen}
    <SettingsPanel onclose={() => appState.closeSettings()} />
{/if}
