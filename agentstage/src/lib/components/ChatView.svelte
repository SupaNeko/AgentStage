<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';
    import { messageStore } from '$lib/stores/messageStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import MessageBubble from './MessageBubble.svelte';
    import { Send, MessageSquare } from 'lucide-svelte';

    let inputText = $state('');
    let sending = $state(false);

    let selectedSession = $derived(
        sessionStore.sessions.find((s) => s.id === sessionStore.selectedSessionId)
    );

    $effect(() => {
        const id = sessionStore.selectedSessionId;
        if (id) {
            messageStore.loadMessages(id);
        } else {
            messageStore.setSessionId(null);
        }
    });

    async function handleSend() {
        const content = inputText.trim();
        if (!content || !sessionStore.selectedSessionId) return;

        sending = true;
        inputText = '';

        try {
            await invoke('send_user_message', {
                sessionId: sessionStore.selectedSessionId,
                content,
            });
            await messageStore.loadMessages(sessionStore.selectedSessionId);
        } catch (err) {
            console.error('Failed to send message:', err);
        } finally {
            sending = false;
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    }

    onMount(() => {
        let unlisten: (() => void) | undefined;

        listen('new_message', (event) => {
            const msg = event.payload as { session_id: string } & Record<string, unknown>;
            if (msg.session_id === sessionStore.selectedSessionId) {
                messageStore.addMessage(msg as unknown as import('$lib/types').Message);
            }
        }).then((fn) => {
            unlisten = fn;
        });

        return () => {
            if (unlisten) unlisten();
        };
    });
</script>

<div class="flex flex-col h-full bg-bg">
    <!-- Header -->
    <header class="flex items-center px-6 py-4 border-b border-border bg-surface shrink-0">
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
        <div class="flex-1 overflow-y-auto">
            {#if messageStore.messages.length === 0}
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
                                senderName={message.sender_name || selectedSession.agent_name || 'Agent'}
                            />
                        </div>
                    {/each}
                </div>
            {/if}
        </div>

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
