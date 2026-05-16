export interface Agent {
    id: string;
    name: string;
    avatar_path: string | null;
    detailed_persona: string;
    simplified_persona: string;
    personality: string | null;
    scenario: string | null;
    example_messages: string | null;
    first_message: string | null;
    creator_notes: string | null;
    tags: string | null;
    model_provider: string | null;
    model_name: string | null;
    base_url: string | null;
    temperature: number;
    max_tokens: number;
    top_p: number;
    presence_penalty: number;
    frequency_penalty: number;
    is_deleted: boolean;
    deleted_at: number | null;
    created_at: number;
    updated_at: number;
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
    current_chat_page?: number;
}

export interface Message {
    id: string;
    session_id: string;
    sender_type: string;
    sender_id: string;
    sender_name?: string;
    sender_avatar?: string | null;
    content: string;
    created_at: number;
    message_type: string;
    page_index?: number;
}

export interface GroupMember {
    participant_type: 'user' | 'agent';
    participant_id: string;
    name: string;
    avatar_path: string | null;
}

export interface SessionConfig {
    session_id: string;
    history_limit: number;
    message_limit: number;
    message_limit_enabled: boolean;
    mute_enabled: boolean;
    agent_message_count: number;
}

export interface UpdateSessionConfigRequest {
    session_id: string;
    history_limit?: number;
    message_limit?: number;
    message_limit_enabled?: boolean;
    mute_enabled?: boolean;
}

export interface ChatPage {
    id: string;
    session_id: string;
    page_index: number;
    name: string;
    is_active: boolean;
    message_count: number;
    created_at: number;
    updated_at: number;
}
