import { invoke } from '@tauri-apps/api/core';
import type { Message } from '$lib/types';

export class MessageStore {
    messages = $state<Message[]>([]);
    currentSessionId = $state<string | null>(null);

    async loadMessages(sessionId: string) {
        try {
            const result = await invoke<Message[]>('get_session_messages', {
                sessionId,
                limit: 50,
                offset: 0,
            });
            this.messages = result.reverse();
            this.currentSessionId = sessionId;
        } catch (err) {
            console.error('Failed to load messages:', err);
            this.messages = [];
            this.currentSessionId = sessionId;
        }
    }

    addMessage(msg: Message) {
        this.messages = [...this.messages, msg];
    }

    setSessionId(id: string | null) {
        this.currentSessionId = id;
        this.messages = [];
    }
}

export const messageStore = new MessageStore();
