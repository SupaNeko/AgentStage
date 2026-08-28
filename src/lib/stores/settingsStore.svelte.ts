import { invoke } from '@tauri-apps/api/core';

export interface AppSettings {
    global_min_trigger_interval: number;
    private_message_limit_default: number;
    group_message_limit_default: number;
    private_limit_enabled_default: boolean;
    group_limit_enabled_default: boolean;
    enter_to_send: boolean;
    theme: string;
    font_size: string;
    language: string;
    launch_on_startup?: boolean;
    minimize_to_tray?: boolean;
    active_persona_id?: string | null;
    default_avatar_path?: string | null;
    quiet_hours_start?: number;
    quiet_hours_end?: number;
    summary_model_config_id: string | null;
    search_provider: string | null;
    search_api_key_set: boolean;
    virtual_time_enabled: boolean;
    virtual_time_base: number | null;
    virtual_time_set_at: number | null;
    virtual_time_rate: number;
}

class SettingsStore {
    settings = $state<AppSettings | null>(null);
    loading = $state(false);

    async load() {
        this.loading = true;
        try {
            this.settings = await invoke<AppSettings>('get_settings');
        } finally {
            this.loading = false;
        }
    }

    async update(partial: Partial<AppSettings> & { search_api_key?: string }) {
        const req = {
            global_min_trigger_interval: partial.global_min_trigger_interval,
            private_message_limit_default: partial.private_message_limit_default,
            group_message_limit_default: partial.group_message_limit_default,
            private_limit_enabled_default: partial.private_limit_enabled_default,
            group_limit_enabled_default: partial.group_limit_enabled_default,
            enter_to_send: partial.enter_to_send,
            theme: partial.theme,
            font_size: partial.font_size,
            language: partial.language,
            launch_on_startup: partial.launch_on_startup,
            minimize_to_tray: partial.minimize_to_tray,
            active_persona_id: partial.active_persona_id,
            default_avatar_path: partial.default_avatar_path,
            summary_model_config_id: partial.summary_model_config_id,
            search_provider: partial.search_provider,
            search_api_key: partial.search_api_key,
            virtual_time_enabled: partial.virtual_time_enabled,
            virtual_time_base: partial.virtual_time_base,
            virtual_time_rate: partial.virtual_time_rate,
        };
        const updated = await invoke<AppSettings>('update_settings', { req });
        this.settings = updated;
        return updated;
    }
}

export const settingsStore = new SettingsStore();
