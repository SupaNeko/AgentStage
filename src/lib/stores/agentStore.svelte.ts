import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import { bumpAvatarVersion } from '$lib/utils';
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

    updateAgentAvatar(agentId: string, avatarPath: string | null) {
        const idx = this.agents.findIndex(a => a.id === agentId);
        if (idx !== -1) {
            if (avatarPath) bumpAvatarVersion(avatarPath);
            this.agents[idx] = { ...this.agents[idx], avatar_path: avatarPath };
            this.agents = [...this.agents];
        }
    }
}

export const agentStore = new AgentStore();
