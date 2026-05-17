import { invoke } from '@tauri-apps/api/core';
import { settingsStore } from './settingsStore.svelte';
import { logger } from '$lib/logger';

export interface UserPersona {
    id: string;
    name: string;
    description?: string;
    avatar_path?: string;
}

export interface CurrentUserPersona {
    id?: string;
    name: string;
    description: string;
    avatar_path?: string;
    is_custom: boolean;
}

class UserPersonaStore {
    personas = $state<UserPersona[]>([]);
    currentPersona = $state<CurrentUserPersona | null>(null);
    loading = $state(false);

    async loadPersonas() {
        this.loading = true;
        try {
            this.personas = await invoke<UserPersona[]>('list_user_personas');
        } catch (e) {
            logger.error('Failed to load user personas', e);
        } finally {
            this.loading = false;
        }
    }

    async loadCurrentPersona() {
        try {
            this.currentPersona = await invoke<CurrentUserPersona>('get_current_user_persona');
        } catch (e) {
            logger.error('Failed to load current persona', e);
        }
    }

    async createPersona(data: { name: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('create_user_persona', { req: data });
        this.personas = [...this.personas, persona];
        return persona;
    }

    async updatePersona(data: { id: string; name?: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('update_user_persona', { req: data });
        this.personas = this.personas.map(p => p.id === persona.id ? persona : p);
        if (this.currentPersona?.id === persona.id) {
            await this.loadCurrentPersona();
        }
        return persona;
    }

    async deletePersona(id: string) {
        await invoke('delete_user_persona', { id });
        this.personas = this.personas.filter(p => p.id !== id);
        if (this.currentPersona?.id === id) {
            await this.activatePersona(null);
        }
    }

    async activatePersona(id: string | null) {
        await invoke('activate_user_persona', { id });
        await settingsStore.load();
        await this.loadCurrentPersona();
    }
}

export const userPersonaStore = new UserPersonaStore();
