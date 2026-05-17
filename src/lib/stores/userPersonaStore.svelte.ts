import { invoke } from '@tauri-apps/api/core';
import { settingsStore } from './settingsStore.svelte';
import { logger } from '$lib/logger';

export interface UserPersona {
    id: string;
    name: string;
    description?: string;
    avatar_path: string | null;
    created_at: number;
    updated_at: number;
}

export interface CurrentUserPersona {
    id?: string;
    name: string;
    description: string;
    avatar_path: string | null;
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
            logger.error('Failed to load user personas:', e);
        } finally {
            this.loading = false;
        }
    }

    async loadCurrentPersona() {
        try {
            this.currentPersona = await invoke<CurrentUserPersona>('get_current_user_persona');
        } catch (e) {
            logger.error('Failed to load current persona:', e);
        }
    }

    async createPersona(data: { name: string; description?: string; avatar_path?: string }) {
        try {
            const persona = await invoke<UserPersona>('create_user_persona', { req: data });
            this.personas = [...this.personas, persona];
            return persona;
        } catch (e) {
            logger.error('Failed to create persona:', e);
            throw e;
        }
    }

    async updatePersona(data: { id: string; name?: string; description?: string; avatar_path?: string }) {
        try {
            const persona = await invoke<UserPersona>('update_user_persona', { req: data });
            this.personas = this.personas.map(p => p.id === persona.id ? persona : p);
            if (this.currentPersona?.id === persona.id) {
                await this.loadCurrentPersona();
            }
            return persona;
        } catch (e) {
            logger.error('Failed to update persona:', e);
            throw e;
        }
    }

    async deletePersona(id: string) {
        try {
            await invoke('delete_user_persona', { id });
            this.personas = this.personas.filter(p => p.id !== id);
            if (this.currentPersona?.id === id) {
                await this.activatePersona(null);
            }
        } catch (e) {
            logger.error('Failed to delete persona:', e);
            throw e;
        }
    }

    async activatePersona(id: string | null) {
        try {
            await invoke('activate_user_persona', { id });
        } catch (e) {
            logger.error('Failed to activate persona:', e);
            throw e;
        }
        try {
            await settingsStore.load();
        } catch (e) {
            logger.error('Failed to reload settings:', e);
        }
        await this.loadCurrentPersona();
    }
}

export const userPersonaStore = new UserPersonaStore();
