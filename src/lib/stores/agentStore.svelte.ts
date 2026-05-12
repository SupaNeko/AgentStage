import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Agent } from '$lib/types';

export class AgentStore {
    agents = $state<Agent[]>([]);

    async loadAgents() {
        try {
            this.agents = await invoke<Agent[]>('list_agents');
            logger.debug('[DEBUG agentStore.loadAgents]', { count: this.agents.length });
        } catch (err) {
            logger.error('Failed to load agents:', err);
        }
    }
}

export const agentStore = new AgentStore();
