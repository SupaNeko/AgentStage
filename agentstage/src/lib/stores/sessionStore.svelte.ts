import { invoke } from '@tauri-apps/api/core';
import type { Session } from '$lib/types';

export class SessionStore {
    sessions = $state<Session[]>([]);
    selectedSessionId = $state<string | null>(null);

    async loadSessions() {
        try {
            this.sessions = await invoke<Session[]>('list_sessions');
        } catch (err) {
            console.error('Failed to load sessions:', err);
        }
    }

    selectSession(id: string | null) {
        this.selectedSessionId = id;
    }

    addSession(session: Session) {
        this.sessions = [session, ...this.sessions];
    }

    updateSessionPreview(sessionId: string, preview: string, time: number) {
        this.sessions = this.sessions.map(s =>
            s.id === sessionId
                ? { ...s, last_message_preview: preview, last_message_at: time }
                : s
        );
    }
}

export const sessionStore = new SessionStore();
