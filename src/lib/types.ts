export interface ModelConfig {
    id: string;
    name: string;
    provider: string;
    model_name: string;
    base_url: string | null;
    api_key: string;
    temperature: number | null;
    max_tokens: number;
    top_p: number;
    presence_penalty: number;
    frequency_penalty: number;
    created_at: number;
    updated_at: number;
}

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
    model_config_id: string | null;
    model_name: string | null;
    temperature: number | null;
    long_term_memory?: string;
    memory_enabled?: boolean;
    proactive_enabled?: number;
    proactive_min_minutes?: number;
    proactive_max_minutes?: number;
    is_deleted: boolean;
    deleted_at: number | null;
    created_at: number;
    updated_at: number;
}

export interface SessionParticipant {
    participant_type: 'user' | 'agent';
    participant_id: string;
    name: string;
    avatar_path: string | null;
    is_deleted: boolean;
}

export interface Session {
    id: string;
    session_type: string;
    participants: SessionParticipant[];
    group_name?: string;
    group_avatar?: string | null;
    last_message_at: number | null;
    last_message_preview: string | null;
    unread_count: number;
    mute_enabled?: boolean;
    current_chat_page?: number;
    is_dissolved?: boolean;
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
    overflow_summary_threshold?: number;
    last_overflow_summary_index?: number;
}

export interface UpdateSessionConfigRequest {
    session_id: string;
    history_limit?: number;
    message_limit?: number;
    message_limit_enabled?: boolean;
    mute_enabled?: boolean;
    overflow_summary_threshold?: number;
    last_overflow_summary_index?: number;
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

export interface RelationshipItem {
    target_id: string;
    target_type: string;
    target_name: string;
    target_avatar: string | null;
    target_label: string;
    target_simplified_persona: string;
    relationship_text: string;
    memory_text: string;
    updated_at: number;
}

export interface GeneratePersonaResult {
    personality: string | null;
    scenario: string | null;
    example_messages: string | null;
    creator_notes: string | null;
    detailed_persona: string;
    simplified_persona: string;
}

export interface ScheduledTask {
    id: string;
    agent_id: string;
    description: string;
    task_type: 'single' | 'recurring';
    trigger_mode?: 'after_minutes' | 'datetime';
    after_minutes?: number;
    year?: number;
    month?: number;
    day?: number;
    hour?: number;
    minute?: number;
    interval_minutes?: number;
    next_trigger_at: number;
    created_at: number;
    is_active: number;
    target_session_id?: string;
}

export interface TimerFormData {
    description: string;
    task_type: 'single' | 'recurring';
    trigger_mode?: 'after_minutes' | 'datetime';
    after_minutes?: number;
    year?: number;
    month?: number;
    day?: number;
    hour?: number;
    minute?: number;
    interval_minutes?: number;
    target_session_id?: string;
}

export interface ThemeInfo {
    id: string;
    name: string;
    version: string;
    author: string;
    description: string;
    tags: string[];
    preview_path: string;
    source: 'builtin' | 'user';
}
