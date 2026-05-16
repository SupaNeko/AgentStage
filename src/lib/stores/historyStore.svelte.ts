import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Session, ChatPage } from '$lib/types';

export class HistoryStore {
    selectedSessionId = $state<string | null>(null);
    selectedPageIndex = $state<number | null>(null);
    chatPages = $state<ChatPage[]>([]);
    sessions = $state<Session[]>([]);
    loadingPages = $state(false);
    // 记录每个 session 上次手动选择的 pageIndex，切换会话时恢复
    sessionPageIndex = $state<Map<string, number>>(new Map());

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
                // 恢复该 session 上次选择的 pageIndex；若已失效则回退到最新
                const saved = this.sessionPageIndex.get(sessionId);
                if (saved !== undefined && pages.some(p => p.page_index === saved)) {
                    this.selectedPageIndex = saved;
                } else {
                    this.selectedPageIndex = pages[0].page_index;
                }
            } else {
                this.selectedPageIndex = 0;
            }
            logger.debug('[DEBUG historyStore.loadChatPages]', { sessionId, count: pages.length, selectedPageIndex: this.selectedPageIndex });
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
        this.selectedPageIndex = null; // 先清空，避免 ChatView 用旧 pageIndex 加载消息
        this.chatPages = [];
        this.loadChatPages(sessionId);
    }

    selectPage(pageIndex: number) {
        this.selectedPageIndex = pageIndex;
        if (this.selectedSessionId) {
            const next = new Map(this.sessionPageIndex);
            next.set(this.selectedSessionId, pageIndex);
            this.sessionPageIndex = next;
        }
    }

    get groupedSessions() {
        const privateSessions = this.sessions.filter(s => s.session_type === 'private');
        const groupSessions = this.sessions.filter(s => s.session_type === 'group');
        return { private: privateSessions, group: groupSessions };
    }
}

export const historyStore = new HistoryStore();
