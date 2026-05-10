import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, beforeEach } from 'vitest';
import { tick } from 'svelte';
import SessionList from './SessionList.svelte';
import { sessionStore } from '$lib/stores/sessionStore.svelte';
import type { Session } from '$lib/types';

describe('SessionList', () => {
    beforeEach(() => {
        sessionStore.sessions = [];
        sessionStore.selectedSessionId = null;
    });

    it('renders empty state when no sessions exist', () => {
        render(SessionList);

        expect(screen.getByText('会话列表')).toBeInTheDocument();
        expect(screen.getByText('还没有会话，去角色列表创建一个吧')).toBeInTheDocument();
    });

    it('renders session names when sessions are added', async () => {
        render(SessionList);

        const session1: Session = {
            id: 's1',
            session_type: 'single',
            last_message_at: Date.now(),
            last_message_preview: 'Hello there',
            unread_count: 2,
            agent_name: 'Agent One',
        };
        const session2: Session = {
            id: 's2',
            session_type: 'single',
            last_message_at: Date.now(),
            last_message_preview: 'General Kenobi',
            unread_count: 0,
            agent_name: 'Agent Two',
        };

        sessionStore.sessions = [session1, session2];
        await tick();

        expect(screen.getByText('Agent One')).toBeInTheDocument();
        expect(screen.getByText('Agent Two')).toBeInTheDocument();
        expect(screen.getByText('Hello there')).toBeInTheDocument();
        expect(screen.getByText('General Kenobi')).toBeInTheDocument();
    });

    it('updates selectedSessionId when a session is clicked', async () => {
        render(SessionList);

        const session: Session = {
            id: 's1',
            session_type: 'single',
            last_message_at: Date.now(),
            last_message_preview: 'Click me',
            unread_count: 1,
            agent_name: 'Clickable Agent',
        };

        sessionStore.sessions = [session];
        await tick();

        const button = screen.getByText('Clickable Agent').closest('button');
        expect(button).toBeTruthy();

        await fireEvent.click(button!);

        expect(sessionStore.selectedSessionId).toBe('s1');
    });

    it('shows unread badge when unread_count > 0', async () => {
        render(SessionList);

        const session: Session = {
            id: 's1',
            session_type: 'single',
            last_message_at: Date.now(),
            last_message_preview: 'New message',
            unread_count: 5,
            agent_name: 'Unread Agent',
        };

        sessionStore.sessions = [session];
        await tick();

        expect(screen.getByText('5')).toBeInTheDocument();
    });

    it('does not show unread badge when unread_count is 0', async () => {
        render(SessionList);

        const session: Session = {
            id: 's1',
            session_type: 'single',
            last_message_at: Date.now(),
            last_message_preview: 'Read message',
            unread_count: 0,
            agent_name: 'Read Agent',
        };

        sessionStore.sessions = [session];
        await tick();

        expect(screen.queryByText('0')).not.toBeInTheDocument();
    });
});
