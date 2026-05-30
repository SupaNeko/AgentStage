import { invoke } from '@tauri-apps/api/core';
import type {
    UsageOverview, ModelUsageItem, AgentUsageItem, AgentModelUsageItem,
    SessionUsageItem, SessionAgentUsageItem, SessionModelUsageItem,
    SessionAgentModelUsageItem, TriggerUsageItem, PaginatedUsageRecords, TimeRange,
    ModelAgentUsageItem,
} from '$lib/types/usage';

class UsageStore {
    timeRange = $state<TimeRange>('last_7_days');
    overview = $state<UsageOverview | null>(null);
    byModel = $state<ModelUsageItem[]>([]);
    byAgent = $state<AgentUsageItem[]>([]);
    bySession = $state<SessionUsageItem[]>([]);
    byTrigger = $state<TriggerUsageItem[]>([]);
    records = $state<PaginatedUsageRecords | null>(null);
    loadingOverview = $state(false);
    loadingModel = $state(false);
    loadingAgent = $state(false);
    loadingSession = $state(false);
    loadingTrigger = $state(false);
    loadingRecords = $state(false);
    error = $state<string | null>(null);

    async loadOverview() {
        this.loadingOverview = true;
        try {
            this.overview = await invoke<UsageOverview>('get_usage_overview', {
                timeRange: this.timeRange,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingOverview = false;
        }
    }

    async loadByModel() {
        this.loadingModel = true;
        try {
            this.byModel = await invoke<ModelUsageItem[]>('get_usage_by_model', {
                timeRange: this.timeRange,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingModel = false;
        }
    }

    async loadByAgent() {
        this.loadingAgent = true;
        try {
            this.byAgent = await invoke<AgentUsageItem[]>('get_usage_by_agent', {
                timeRange: this.timeRange,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingAgent = false;
        }
    }

    async loadAgentModelBreakdown(agentId: string) {
        return await invoke<AgentModelUsageItem[]>('get_agent_model_breakdown', {
            agentId,
            timeRange: this.timeRange,
        });
    }

    async loadModelAgentBreakdown(modelConfigId: string) {
        return await invoke<ModelAgentUsageItem[]>('get_model_agent_breakdown', {
            modelConfigId,
            timeRange: this.timeRange,
        });
    }

    async loadBySession() {
        this.loadingSession = true;
        try {
            this.bySession = await invoke<SessionUsageItem[]>('get_usage_by_session', {
                timeRange: this.timeRange,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingSession = false;
        }
    }

    async loadSessionAgentBreakdown(sessionId: string) {
        return await invoke<SessionAgentUsageItem[]>('get_session_agent_breakdown', {
            sessionId,
            timeRange: this.timeRange,
        });
    }

    async loadSessionModelBreakdown(sessionId: string) {
        return await invoke<SessionModelUsageItem[]>('get_session_model_breakdown', {
            sessionId,
            timeRange: this.timeRange,
        });
    }

    async loadSessionAgentModelBreakdown(sessionId: string) {
        return await invoke<SessionAgentModelUsageItem[]>('get_session_agent_model_breakdown', {
            sessionId,
            timeRange: this.timeRange,
        });
    }

    async loadByTrigger() {
        this.loadingTrigger = true;
        try {
            this.byTrigger = await invoke<TriggerUsageItem[]>('get_usage_by_trigger', {
                timeRange: this.timeRange,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingTrigger = false;
        }
    }

    async loadRecords(page: number = 1, pageSize: number = 50, filters?: { agentId?: string; modelConfigId?: string; sessionId?: string; triggerType?: string }) {
        this.loadingRecords = true;
        try {
            this.records = await invoke<PaginatedUsageRecords>('get_usage_records', {
                timeRange: this.timeRange,
                page,
                pageSize,
                filters,
            });
        } catch (e) {
            this.error = String(e);
        } finally {
            this.loadingRecords = false;
        }
    }

    setTimeRange(range: TimeRange) {
        this.timeRange = range;
    }
}

export const usageStore = new UsageStore();
