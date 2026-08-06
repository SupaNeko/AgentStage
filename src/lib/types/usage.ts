export interface UsageOverview {
    total_calls: number;
    total_prompt_tokens: number;
    total_completion_tokens: number;
    total_tokens: number;
    daily_trend: DailyTrend[];
}

export interface DailyTrend {
    date: string;
    calls: number;
    tokens: number;
}

export interface ModelUsageItem {
    model_config_id: string;
    model_name: string;
    provider: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface AgentUsageItem {
    agent_id: string;
    agent_name: string;
    avatar_path: string | null;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface AgentModelUsageItem {
    model_config_id: string;
    model_name: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface ModelAgentUsageItem {
    agent_id: string;
    agent_name: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface SessionUsageItem {
    session_id: string;
    session_name: string;
    session_type: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface SessionAgentUsageItem {
    agent_id: string;
    agent_name: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface SessionModelUsageItem {
    model_config_id: string;
    model_name: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface SessionAgentModelUsageItem {
    agent_id: string;
    agent_name: string;
    model_config_id: string;
    model_name: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface TriggerUsageItem {
    trigger_type: string;
    calls: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface UsageFilters {
    agent_id?: string;
    model_config_id?: string;
    session_id?: string;
    trigger_type?: string;
}

export interface UsageRecordDetail {
    id: string;
    agent_name: string;
    model_name: string;
    session_name: string | null;
    trigger_type: string;
    call_round: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    created_at: number;
}

export interface PaginatedUsageRecords {
    records: UsageRecordDetail[];
    total: number;
    page: number;
    page_size: number;
}

export type TimeRange = 'today' | 'last_7_days' | 'last_30_days' | 'this_month' | 'all';

export const TRIGGER_TYPE_LABELS: Record<string, string> = {
    user_message: '用户消息触发',
    background_scan: '后台扫描',
    timer: '定时任务',
    proactive: '主动会话',
    persona_generation: '人设生成',
    tts_translate: 'TTS 翻译',
};
