export interface Agent {
    id: string;
    name: string;
    avatar_path: string | null;
    detailed_persona: string;
    simplified_persona: string;
    model_provider: string | null;
    model_name: string | null;
    created_at: number;
}

export interface Session {
    id: string;
    session_type: string;
    last_message_at: number | null;
    last_message_preview: string | null;
    unread_count: number;
    agent_id?: string;
    agent_name?: string;
    agent_avatar?: string;
    group_name?: string;
    group_avatar?: string;
    mute_enabled?: boolean;
}

export interface Message {
    id: string;
    session_id: string;
    sender_type: string;
    sender_id: string;
    sender_name?: string;
    content: string;
    created_at: number;
    message_type: string;
}
