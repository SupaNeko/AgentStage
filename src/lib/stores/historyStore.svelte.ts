import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Session, ChatPage } from '$lib/types';

export class HistoryStore {
    selectedSessionId = $state<string | null>(null);
    selectedPageIndex = $state<number | null>(null);
    chatPages = $state<ChatPage[]>([]);
    sessions = $state<Session[]>([]);
    loadingPages = $state(false);

    async loadSessions() {
        try {
            const all = await invoke<Session[]>('list_sessions');
            this.sessions = all;
            logger.debug('[DEBUG historyStore.loadSessions]', { count: all.length });
        } catch (err) {
            logger.error('Failed to load history sessions:', err);
        }
    }

    async loadChatPages(sessionId: string) {
        this.loadingPages = true;
        try {
            const pages = await invoke<ChatPage[]>('list_chat_pages', {
                req: { session_id: sessionId },
            });
            this.chatPages = pages;
            if (pages.length > 0) {
                this.selectedPageIndex = pages[0].page_index;
            } else {
                this.selectedPageIndex = 0;
            }
            logger.debug('[DEBUG historyStore.loadChatPages]', { sessionId, count: pages.length });
        } catch (err) {
            logger.error('Failed to load chat pages:', err);
            this.chatPages = [];
            this.selectedPageIndex = 0;
        } finally {
            this.loadingPages = false;
        }
    }

    selectSession(sessionId: string) {
        this.selectedSessionId = sessionId;
        this.loadChatPages(sessionId);
    }

    selectPage(pageIndex: number) {
        this.selectedPageIndex = pageIndex;
    }

    get groupedSessions() {
        const privateSessions = this.sessions.filter(s => s.session_type === 'private');
        const groupSessions = this.sessions.filter(s => s.session_type === 'group');
        return { private: privateSessions, group: groupSessions };
    }
}

export const historyStore = new HistoryStore();
