import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import MessageBubble from './MessageBubble.svelte';
import type { Message } from '$lib/types';

describe('MessageBubble', () => {
    const baseMessage: Message = {
        id: 'm1',
        session_id: 's1',
        sender_type: 'user',
        sender_id: 'u1',
        content: 'Hello world',
        created_at: Date.now(),
        message_type: 'text',
        sender_name: 'User',
    };

    it('renders user message with correct styling', () => {
        render(MessageBubble, {
            props: {
                message: baseMessage,
                isMe: true,
                senderName: 'User',
            },
        });

        const bubble = screen.getByText('Hello world').closest('div');
        expect(bubble).toHaveClass('bg-primary');
        expect(bubble).toHaveClass('text-white');
        expect(bubble).toHaveClass('rounded-tr-sm');
    });

    it('renders agent message with name and correct styling', () => {
        const agentMessage: Message = {
            ...baseMessage,
            sender_type: 'agent',
            sender_name: 'Test Agent',
        };

        render(MessageBubble, {
            props: {
                message: agentMessage,
                isMe: false,
                senderName: 'Test Agent',
            },
        });

        expect(screen.getByText('Test Agent')).toBeInTheDocument();

        const bubble = screen.getByText('Hello world').closest('div');
        expect(bubble).toHaveClass('bg-surface');
        expect(bubble).toHaveClass('border');
        expect(bubble).toHaveClass('rounded-tl-sm');
    });
});
