import { render, screen, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { tick } from 'svelte';
import ChatView from './ChatView.svelte';
import { sessionStore } from '$lib/stores/sessionStore.svelte';
import { messageStore } from '$lib/stores/messageStore.svelte';
import type { Message, Session } from '$lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { invoke } from '@tauri-apps/api/core';

describe('ChatView', () => {
    const mockInvoke = vi.mocked(invoke);

    beforeEach(() => {
        sessionStore.selectedSessionId = null;
        sessionStore.sessions = [];
        messageStore.messages = [];
        messageStore.currentSessionId = null;
        vi.clearAllMocks();
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([]);
            }
            if (cmd === 'send_user_message') {
                return Promise.resolve(undefined);
            }
            return Promise.resolve(undefined);
        });
    });

    it('renders empty state when no session is selected', () => {
        render(ChatView);
        expect(screen.queryAllByText('选择一个会话开始聊天').length).toBeGreaterThanOrEqual(1);
    });

    it('renders messages when session is selected', async () => {
        const messages: Message[] = [
            {
                id: 'm1',
                session_id: 's1',
                sender_type: 'user',
                sender_id: 'u1',
                content: 'Hello user',
                created_at: Date.now() - 1000,
                message_type: 'text',
                sender_name: 'User',
            },
            {
                id: 'm2',
                session_id: 's1',
                sender_type: 'agent',
                sender_id: 'a1',
                content: 'Hello agent',
                created_at: Date.now(),
                message_type: 'text',
                sender_name: 'Test Agent',
            },
        ];

        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([...messages].reverse());
            }
            return Promise.resolve(undefined);
        });

        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);

        await waitFor(() => {
            expect(screen.getByText('Hello user')).toBeInTheDocument();
        });
        expect(screen.getByText('Hello agent')).toBeInTheDocument();
        expect(screen.getAllByText('Test Agent').length).toBeGreaterThanOrEqual(1);
    });

    it('shows empty message state when session has no messages', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';
        messageStore.messages = [];

        render(ChatView);
        await tick();

        expect(screen.getByText('还没有消息，发送第一条消息吧')).toBeInTheDocument();
    });
});
