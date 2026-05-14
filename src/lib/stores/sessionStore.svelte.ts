import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Session } from '$lib/types';

export class SessionStore {
    sessions = $state<Session[]>([]);
    selectedSessionId = $state<string | null>(null);

    async loadSessions() {
        try {
            this.sessions = await invoke<Session[]>('list_sessions');
            logger.debug('[DEBUG sessionStore.loadSessions]', { count: this.sessions.length });
        } catch (err) {
            logger.debug('[DEBUG sessionStore.loadSessions] error', { error: err });
            logger.error('Failed to load sessions:', err);
        }
    }

    selectSession(id: string | null) {
        logger.debug('[DEBUG sessionStore.selectSession]', { id });
        this.selectedSessionId = id;
    }

    addSession(session: Session) {
        logger.debug('[DEBUG sessionStore.addSession]', { id: session.id });
        this.sessions = [session, ...this.sessions];
    }

    updateSessionPreview(sessionId: string, preview: string, time: number) {
        this.sessions = this.sessions.map(s =>
            s.id === sessionId
                ? { ...s, last_message_preview: preview, last_message_at: time }
                : s
        );
    }

    async resetSession(sessionId: string): Promise<string> {
        try {
            const pageId = await invoke<string>('reset_session', { req: { session_id: sessionId } });
            await this.loadSessions();
            this.sessions = this.sessions.map(s =>
                s.id === sessionId
                    ? { ...s, last_message_preview: '' }
                    : s
            );
            // 清空前端消息列表，强制重新加载
            const { messageStore } = await import('$lib/stores/messageStore.svelte');
            messageStore.setSessionId(sessionId);
            return pageId;
        } catch (err) {
            logger.error('Failed to reset session:', err);
            throw err;
        }
    }

    async disbandGroup(sessionId: string): Promise<boolean> {
        try {
            const result = await invoke<boolean>('disband_group', { req: { session_id: sessionId } });
            if (result) {
                this.sessions = this.sessions.filter(s => s.id !== sessionId);
                if (this.selectedSessionId === sessionId) {
                    this.selectedSessionId = null;
                }
            }
            return result;
        } catch (err) {
            logger.error('Failed to disband group:', err);
            throw err;
        }
    }

    removeSession(sessionId: string) {
        this.sessions = this.sessions.filter(s => s.id !== sessionId);
        if (this.selectedSessionId === sessionId) {
            this.selectedSessionId = null;
        }
    }
}

export const sessionStore = new SessionStore();
