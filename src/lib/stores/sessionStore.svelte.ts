import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Session } from '$lib/types';

export class SessionStore {
    sessions = $state<Session[]>([]);
    selectedSessionId = $state<string | null>(null);

    async loadSessions() {
        try {
            const fresh = await invoke<Session[]>('list_sessions');
            // 保留已有的 unread_count，因为后端不维护实时未读计数
            const existingUnread = new Map(this.sessions.map(s => [s.id, s.unread_count]));
            this.sessions = fresh
                .map(s => ({
                    ...s,
                    unread_count: existingUnread.get(s.id) ?? s.unread_count,
                }))
                .filter(s => {
                    if (s.session_type === 'group' && s.is_dissolved) return false;
                    if (s.session_type === 'private') {
                        const deletedAgent = s.participants.find(p => p.participant_type === 'agent' && p.is_deleted);
                        if (deletedAgent) return false;
                    }
                    return true;
                });
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
        const exists = this.sessions.some(s => s.id === session.id);
        if (exists) {
            // 已有会话则更新并置顶，避免重复
            this.sessions = [session, ...this.sessions.filter(s => s.id !== session.id)];
        } else {
            this.sessions = [session, ...this.sessions];
        }
    }

    updateSessionPreview(sessionId: string, preview: string, time: number) {
        const idx = this.sessions.findIndex(s => s.id === sessionId);
        if (idx !== -1) {
            this.sessions[idx].last_message_preview = preview;
            this.sessions[idx].last_message_at = time;
            this.sessions = [...this.sessions];
            logger.debug('[DEBUG sessionStore.updateSessionPreview] updated', { sessionId, preview: preview.slice(0, 50) });
        } else {
            logger.debug('[DEBUG sessionStore.updateSessionPreview] not found', { sessionId });
        }
    }

    incrementUnreadCount(sessionId: string) {
        const idx = this.sessions.findIndex(s => s.id === sessionId);
        if (idx !== -1) {
            this.sessions[idx].unread_count = this.sessions[idx].unread_count + 1;
            this.sessions = [...this.sessions];
        }
    }

    clearUnreadCount(sessionId: string) {
        const idx = this.sessions.findIndex(s => s.id === sessionId);
        if (idx !== -1) {
            this.sessions[idx].unread_count = 0;
            this.sessions = [...this.sessions];
        }
    }

    async resetSession(sessionId: string): Promise<string> {
        try {
            const pageId = await invoke<string>('reset_session', { req: { session_id: sessionId } });
            await this.loadSessions();
            const idx = this.sessions.findIndex(s => s.id === sessionId);
            if (idx !== -1) {
                this.sessions[idx].last_message_preview = '';
                this.sessions[idx].unread_count = 0;
                this.sessions = [...this.sessions];
            }
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
