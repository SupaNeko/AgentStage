import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { SessionStore } from './sessionStore.svelte';
import type { Session } from '$lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/logger', () => ({ logger: { debug: vi.fn(), error: vi.fn() } }));

const mockInvoke = vi.mocked(invoke);

function makeSession(overrides: Partial<Session> = {}): Session {
    return {
        id: '1',
        session_type: 'single',
        last_message_at: null,
        last_message_preview: null,
        unread_count: 0,
        participants: [
            { participant_type: 'user', participant_id: 'user', name: '用户', avatar_path: null, is_deleted: false },
            { participant_type: 'agent', participant_id: 'a1', name: 'Test Agent', avatar_path: null, is_deleted: false },
        ],
        ...overrides,
    };
}

describe('SessionStore', () => {
    let store: SessionStore;

    beforeEach(() => {
        store = new SessionStore();
    });

    it('adds a session to the array', () => {
        const session = makeSession();

        store.addSession(session);

        expect(store.sessions).toHaveLength(1);
        expect(store.sessions[0].id).toBe('1');
        const agentParticipant = store.sessions[0].participants.find(p => p.participant_type === 'agent');
        expect(agentParticipant?.name).toBe('Test Agent');
    });

    it('prepends new sessions', () => {
        const session1 = makeSession({ id: '1' });
        const session2 = makeSession({ id: '2' });

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
        const session = makeSession({ id: '1', last_message_at: 1000, last_message_preview: 'Old preview' });

        store.addSession(session);
        store.updateSessionPreview('1', 'New preview', 2000);

        const updated = store.sessions.find(s => s.id === '1');
        expect(updated).toBeDefined();
        expect(updated!.last_message_preview).toBe('New preview');
        expect(updated!.last_message_at).toBe(2000);
    });

    it('does not affect other sessions when updating preview', () => {
        const session1 = makeSession({ id: '1', last_message_at: 1000, last_message_preview: 'Preview 1' });
        const session2 = makeSession({ id: '2', last_message_at: 2000, last_message_preview: 'Preview 2' });

        store.addSession(session1);
        store.addSession(session2);
        store.updateSessionPreview('1', 'Updated', 3000);

        const unchanged = store.sessions.find(s => s.id === '2');
        expect(unchanged!.last_message_preview).toBe('Preview 2');
        expect(unchanged!.last_message_at).toBe(2000);
    });

    it('clears last_message_preview after resetSession', async () => {
        const session = makeSession({ id: '1', last_message_at: 1000, last_message_preview: 'Old message' });

        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'reset_session') {
                return 'new-page-id';
            }
            if (cmd === 'list_sessions') {
                // backend still returns old preview because messages are not deleted
                return [{ ...session }];
            }
            return undefined;
        });

        store.addSession(session);
        const pageId = await store.resetSession('1');

        expect(pageId).toBe('new-page-id');
        const updated = store.sessions.find(s => s.id === '1');
        expect(updated).toBeDefined();
        expect(updated!.last_message_preview).toBe('');
    });
});
