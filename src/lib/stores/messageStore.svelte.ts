import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Message } from '$lib/types';

export class MessageStore {
    messagesBySession = $state<Map<string, Message[]>>(new Map());
    currentSessionId = $state<string | null>(null);

    messages = $derived(
        this.currentSessionId ? (this.messagesBySession.get(this.currentSessionId) || []) : []
    );

    async loadMessages(sessionId: string, pageIndex?: number) {
        logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex });
        try {
            const req: Record<string, unknown> = { session_id: sessionId, limit: 50, offset: 0 };
            if (pageIndex !== undefined) {
                req.page_index = pageIndex;
            }
            const result = await invoke<Message[]>('get_session_messages', { req });
            logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex, count: result.length });
            const next = new Map(this.messagesBySession);
            next.set(sessionId, result.reverse());
            this.messagesBySession = next;
            this.currentSessionId = sessionId;
        } catch (err) {
            logger.debug('[DEBUG messageStore.loadMessages] error', { sessionId, pageIndex, error: err });
            const next = new Map(this.messagesBySession);
            next.set(sessionId, []);
            this.messagesBySession = next;
            this.currentSessionId = sessionId;
        }
    }

    addMessage(msg: Message) {
        logger.debug('[DEBUG messageStore.addMessage]', { id: msg.id, sessionId: msg.session_id, contentPreview: msg.content?.slice(0, 50) });
        const list = this.messagesBySession.get(msg.session_id) || [];
        const next = new Map(this.messagesBySession);
        next.set(msg.session_id, [...list, msg]);
        this.messagesBySession = next;
    }

    removeMessage(sessionId: string, msgId: string) {
        const list = this.messagesBySession.get(sessionId) || [];
        const next = new Map(this.messagesBySession);
        next.set(sessionId, list.filter(m => m.id !== msgId));
        this.messagesBySession = next;
    }

    setSessionId(id: string | null) {
        this.currentSessionId = id;
        if (id) {
            const next = new Map(this.messagesBySession);
            next.set(id, []);
            this.messagesBySession = next;
        }
    }
}

export const messageStore = new MessageStore();
