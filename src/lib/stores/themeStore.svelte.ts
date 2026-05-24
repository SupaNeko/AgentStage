import { invoke } from '@tauri-apps/api/core';
import type { ThemeInfo } from '$lib/types';
import { settingsStore } from './settingsStore.svelte';

class ThemeStore {
    themes = $state<ThemeInfo[]>([]);
    activeThemeId = $state<string>(settingsStore.settings?.theme ?? 'default');

    async loadThemes() {
        try {
            this.themes = await invoke<ThemeInfo[]>('list_themes');
        } catch (e) {
            console.error('Failed to load themes:', e);
        }
    }

    async applyTheme(themeId: string) {
        try {
            const css = await invoke<string>('read_theme_css', { themeId });
            let el = document.getElementById('theme-active');
            if (!el) {
                el = document.createElement('style');
                el.id = 'theme-active';
                document.head.appendChild(el);
            }
            el.textContent = css;
            this.activeThemeId = themeId;

            // Persist choice via settingsStore (handles correct req shape)
            await settingsStore.update({ theme: themeId });
        } catch (e) {
            console.error('Failed to apply theme:', e);
        }
    }
}

export const themeStore = new ThemeStore();
