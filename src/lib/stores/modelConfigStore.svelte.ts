import { invoke } from '@tauri-apps/api/core';
import type { ModelConfig } from '$lib/types';

class ModelConfigStore {
    configs = $state<ModelConfig[]>([]);
    loading = $state(false);

    async load() {
        this.loading = true;
        try {
            this.configs = await invoke<ModelConfig[]>('list_model_configs');
        } catch (e) {
            console.error('Failed to load model configs:', e);
        } finally {
            this.loading = false;
        }
    }

    async create(config: Omit<ModelConfig, 'id' | 'created_at' | 'updated_at'>) {
        const created = await invoke<ModelConfig>('create_model_config', { req: config });
        this.configs = [created, ...this.configs];
        return created;
    }

    async update(id: string, partial: Partial<ModelConfig>) {
        const updated = await invoke<ModelConfig>('update_model_config', { req: { id, ...partial } });
        this.configs = this.configs.map(c => c.id === id ? updated : c);
        return updated;
    }

    async delete(id: string) {
        await invoke('delete_model_config', { req: { id } });
        this.configs = this.configs.filter(c => c.id !== id);
    }

    async testConnection(id: string) {
        return await invoke<{ success: boolean; latency_ms: number; message: string }>(
            'test_model_config_connection',
            { req: { id } }
        );
    }

    getById(id: string): ModelConfig | undefined {
        return this.configs.find(c => c.id === id);
    }
}

export const modelConfigStore = new ModelConfigStore();
