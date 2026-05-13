import { render, screen, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { tick } from 'svelte';
import ChatView from './ChatView.svelte';
import { sessionStore } from '$lib/stores/sessionStore.svelte';
import { messageStore } from '$lib/stores/messageStore.svelte';
import type { Message, Session, SessionConfig } from '$lib/types';

const eventCallbacks = new Map<string, ((event: { payload: unknown }) => void)>();

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn((event: string, callback: (event: { payload: unknown }) => void) => {
        eventCallbacks.set(event, callback);
        return Promise.resolve(() => {});
    }),
}));

import { invoke } from '@tauri-apps/api/core';

describe('ChatView', () => {
    const mockInvoke = vi.mocked(invoke);

    beforeEach(() => {
        sessionStore.selectedSessionId = null;
        sessionStore.sessions = [];
        messageStore.messages = [];
        messageStore.currentSessionId = null;
        eventCallbacks.clear();
        vi.clearAllMocks();
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([]);
            }
            if (cmd === 'send_user_message') {
                return Promise.resolve(undefined);
            }
            if (cmd === 'get_session_config') {
                return Promise.resolve({
                    session_id: 's1',
                    history_limit: 30,
                    message_limit: 10,
                    message_limit_enabled: true,
                    mute_enabled: false,
                    agent_message_count: 0,
                } as SessionConfig);
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

    it('shows typing indicator when agent_typing event arrives', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_id: 'a1',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);
        await tick();

        const callback = eventCallbacks.get('agent_typing');
        expect(callback).toBeDefined();
        callback!({ payload: { agent_id: 'a1' } });
        await tick();

        expect(screen.getByText('正在输入中...')).toBeInTheDocument();
    });

    it('hides typing indicator when agent_completed event arrives', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_id: 'a1',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);
        await tick();

        const typingCallback = eventCallbacks.get('agent_typing');
        expect(typingCallback).toBeDefined();
        typingCallback!({ payload: { agent_id: 'a1' } });
        await tick();

        expect(screen.getByText('正在输入中...')).toBeInTheDocument();

        const completedCallback = eventCallbacks.get('agent_completed');
        expect(completedCallback).toBeDefined();
        completedCallback!({ payload: { agent_id: 'a1' } });
        await tick();

        expect(screen.queryByText('正在输入中...')).not.toBeInTheDocument();
    });

    it('shows agent avatar in typing indicator', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_id: 'a1',
            agent_name: 'Test Agent',
            agent_avatar: '/avatar.png',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);
        await tick();

        const typingCallback = eventCallbacks.get('agent_typing');
        expect(typingCallback).toBeDefined();
        typingCallback!({ payload: { agent_id: 'a1' } });
        await tick();

        const typingIndicator = screen.getByTestId('typing-indicator');
        expect(typingIndicator).toBeInTheDocument();
        const avatarImg = typingIndicator.querySelector('img');
        expect(avatarImg).not.toBeNull();
        expect(avatarImg).toHaveAttribute('src', '/avatar.png');
        expect(avatarImg).toHaveAttribute('alt', 'Test Agent');
    });

    it('shows message limit warning when limit is reached', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_id: 'a1',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([]);
            }
            if (cmd === 'send_user_message') {
                return Promise.resolve(undefined);
            }
            if (cmd === 'get_session_config') {
                return Promise.resolve({
                    session_id: 's1',
                    history_limit: 30,
                    message_limit: 10,
                    message_limit_enabled: true,
                    mute_enabled: false,
                    agent_message_count: 12,
                } as SessionConfig);
            }
            return Promise.resolve(undefined);
        });

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);
        await tick();
        await waitFor(() => {
            expect(screen.getByText('已达到消息上限，角色不再主动回复')).toBeInTheDocument();
        });
        expect(screen.getByText('重置限制')).toBeInTheDocument();
    });

    it('calls reset_message_count when reset button is clicked', async () => {
        const session: Session = {
            id: 's1',
            session_type: 'single',
            agent_id: 'a1',
            agent_name: 'Test Agent',
            unread_count: 0,
            last_message_at: null,
            last_message_preview: null,
        };

        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([]);
            }
            if (cmd === 'send_user_message') {
                return Promise.resolve(undefined);
            }
            if (cmd === 'get_session_config') {
                return Promise.resolve({
                    session_id: 's1',
                    history_limit: 30,
                    message_limit: 10,
                    message_limit_enabled: true,
                    mute_enabled: false,
                    agent_message_count: 12,
                } as SessionConfig);
            }
            if (cmd === 'reset_message_count') {
                return Promise.resolve(undefined);
            }
            return Promise.resolve(undefined);
        });

        sessionStore.sessions = [session];
        sessionStore.selectedSessionId = 's1';

        render(ChatView);
        await tick();
        await waitFor(() => {
            expect(screen.getByText('已达到消息上限，角色不再主动回复')).toBeInTheDocument();
        });

        const resetButton = screen.getByText('重置限制');
        resetButton.click();
        await tick();

        expect(mockInvoke).toHaveBeenCalledWith('reset_message_count', { sessionId: 's1' });
    });

    it('scrolls to bottom when session is selected and messages load', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_session_messages') {
                return Promise.resolve([
                    {
                        id: 'm2',
                        session_id: 's1',
                        sender_type: 'user',
                        sender_id: 'u1',
                        content: 'Hello',
                        created_at: Date.now(),
                        message_type: 'text',
                        sender_name: 'User',
                    },
                    {
                        id: 'm1',
                        session_id: 's1',
                        sender_type: 'agent',
                        sender_id: 'a1',
                        content: 'Hi there',
                        created_at: Date.now() - 1000,
                        message_type: 'text',
                        sender_name: 'Agent',
                    },
                ]);
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

        const container = screen.getByTestId('message-list');
        Object.defineProperty(container, 'scrollHeight', { value: 1000, writable: true });
        container.scrollTop = 0;

        await waitFor(() => {
            expect(screen.getByText('Hello')).toBeInTheDocument();
        });

        expect(container.scrollTop).toBe(1000);
    });

    it('scrolls to bottom when user sends a message', async () => {
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
        await tick();

        const container = screen.getByTestId('message-list');
        Object.defineProperty(container, 'scrollHeight', { value: 1000, writable: true });
        container.scrollTop = 0;

        messageStore.addMessage({
            id: 'm-user',
            session_id: 's1',
            sender_type: 'user',
            sender_id: 'u1',
            content: 'User message',
            created_at: Date.now(),
            message_type: 'text',
            sender_name: 'User',
        });
        await tick();

        expect(container.scrollTop).toBe(1000);
    });

    it('does not auto-scroll when agent message arrives', async () => {
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
        await tick();

        const container = screen.getByTestId('message-list');
        Object.defineProperty(container, 'scrollHeight', { value: 1000, writable: true });
        container.scrollTop = 100;

        messageStore.addMessage({
            id: 'm-agent',
            session_id: 's1',
            sender_type: 'agent',
            sender_id: 'a1',
            content: 'Agent message',
            created_at: Date.now(),
            message_type: 'text',
            sender_name: 'Agent',
        });
        await tick();

        expect(container.scrollTop).toBe(100);
    });
});
