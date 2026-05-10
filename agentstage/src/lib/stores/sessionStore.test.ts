import { describe, it, expect, beforeEach } from 'vitest';
import { SessionStore } from './sessionStore.svelte';
import type { Session } from '$lib/types';

describe('SessionStore', () => {
    let store: SessionStore;

    beforeEach(() => {
        store = new SessionStore();
    });

    it('adds a session to the array', () => {
        const session: Session = {
            id: '1',
            session_type: 'single',
            last_message_at: null,
            last_message_preview: null,
            unread_count: 0,
            agent_name: 'Test Agent',
        };

        store.addSession(session);

        expect(store.sessions).toHaveLength(1);
        expect(store.sessions[0].id).toBe('1');
        expect(store.sessions[0].agent_name).toBe('Test Agent');
    });

    it('prepends new sessions', () => {
        const session1: Session = {
            id: '1',
            session_type: 'single',
            last_message_at: null,
            last_message_preview: null,
            unread_count: 0,
        };
        const session2: Session = {
            id: '2',
            session_type: 'single',
            last_message_at: null,
            last_message_preview: null,
            unread_count: 0,
        };

        store.addSession(session1);
        store.addSession(session2);

        expect(store.sessions).toHaveLength(2);
        expect(store.sessions[0].id).toBe('2');
        expect(store.sessions[1].id).toBe('1');
    });

    it('updates selectedSessionId via selectSession', () => {
        expect(store.selectedSessionId).toBeNull();

        store.selectSession('abc-123');

        expect(store.selectedSessionId).toBe('abc-123');
    });

    it('clears selectedSessionId when selectSession is called with null', () => {
        store.selectSession('abc-123');
        store.selectSession(null);

        expect(store.selectedSessionId).toBeNull();
    });

    it('updates preview and time via updateSessionPreview', () => {
        const session: Session = {
            id: '1',
            session_type: 'single',
            last_message_at: 1000,
            last_message_preview: 'Old preview',
            unread_count: 0,
        };

        store.addSession(session);
        store.updateSessionPreview('1', 'New preview', 2000);

        const updated = store.sessions.find(s => s.id === '1');
        expect(updated).toBeDefined();
        expect(updated!.last_message_preview).toBe('New preview');
        expect(updated!.last_message_at).toBe(2000);
    });

    it('does not affect other sessions when updating preview', () => {
        const session1: Session = {
            id: '1',
            session_type: 'single',
            last_message_at: 1000,
            last_message_preview: 'Preview 1',
            unread_count: 0,
        };
        const session2: Session = {
            id: '2',
            session_type: 'single',
            last_message_at: 2000,
            last_message_preview: 'Preview 2',
            unread_count: 0,
        };

        store.addSession(session1);
        store.addSession(session2);
        store.updateSessionPreview('1', 'Updated', 3000);

        const unchanged = store.sessions.find(s => s.id === '2');
        expect(unchanged!.last_message_preview).toBe('Preview 2');
        expect(unchanged!.last_message_at).toBe(2000);
    });
});
