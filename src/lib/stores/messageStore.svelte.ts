import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Message } from '$lib/types';

export class MessageStore {
    messages = $state<Message[]>([]);
    currentSessionId = $state<string | null>(null);

    async loadMessages(sessionId: string, pageIndex?: number) {
        logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex });
        try {
            const req: Record<string, unknown> = { session_id: sessionId, limit: 50, offset: 0 };
            if (pageIndex !== undefined) {
                req.page_index = pageIndex;
            }
            const result = await invoke<Message[]>('get_session_messages', { req });
            logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex, count: result.length });
            this.messages = result.reverse();
            this.currentSessionId = sessionId;
        } catch (err) {
            logger.debug('[DEBUG messageStore.loadMessages] error', { sessionId, pageIndex, error: err });
            this.messages = [];
            this.currentSessionId = sessionId;
        }
    }

    addMessage(msg: Message) {
        logger.debug('[DEBUG messageStore.addMessage]', { id: msg.id, contentPreview: msg.content?.slice(0, 50) });
        this.messages = [...this.messages, msg];
    }

    setSessionId(id: string | null) {
        this.currentSessionId = id;
        this.messages = [];
    }
}

export const messageStore = new MessageStore();
