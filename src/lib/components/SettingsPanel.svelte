<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { X, User } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { themeStore } from '$lib/stores/themeStore.svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';

    let draft = $state({ global_min_trigger_interval: 30 });
    let saving = $state(false);
    let showAvatarModal = $state(false);
    let userAvatar = $state<string | null>(null);
    let activeTab = $state('general');

    let quietHoursEnabled = $state(false);
    let quietStart = $state('00:00');
    let quietEnd = $state('08:00');

    function minutesToTime(minutes: number): string {
        const h = Math.floor(minutes / 60).toString().padStart(2, '0');
        const m = (minutes % 60).toString().padStart(2, '0');
        return `${h}:${m}`;
    }

    function timeToMinutes(time: string): number {
        const [h, m] = time.split(':').map(Number);
        return h * 60 + m;
    }

    $effect(() => {
        if (settingsStore.settings) {
            draft = {
                global_min_trigger_interval: settingsStore.settings.global_min_trigger_interval,
            };
            quietHoursEnabled = (settingsStore.settings.quiet_hours_start ?? -1) >= 0;
            quietStart = minutesToTime(settingsStore.settings.quiet_hours_start ?? 0);
            quietEnd = minutesToTime(settingsStore.settings.quiet_hours_end ?? 480);
        }
    });

    async function handleSave() {
        saving = true;
        try {
            await settingsStore.update({
                global_min_trigger_interval: draft.global_min_trigger_interval,
            });
            if (quietHoursEnabled) {
                await invoke('update_quiet_hours', {
                    quietHoursStart: timeToMinutes(quietStart),
                    quietHoursEnd: timeToMinutes(quietEnd),
                });
            } else {
                await invoke('update_quiet_hours', {
                    quietHoursStart: -1,
                    quietHoursEnd: -1,
                });
            }
            if (settingsStore.settings) {
                settingsStore.settings.quiet_hours_start = quietHoursEnabled ? timeToMinutes(quietStart) : -1;
                settingsStore.settings.quiet_hours_end = quietHoursEnabled ? timeToMinutes(quietEnd) : -1;
            }
            toastStore.show('已保存', 'success', 2000);
        } catch (err) {
            toastStore.show(`保存失败：${err}`, 'error');
        } finally {
            saving = false;
        }
    }

    let { onclose }: { onclose: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 modal-overlay" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[80vh] flex flex-col modal-card">
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">设置</h3>
            <button onclick={onclose} class="p-1 hover:bg-gray-100 rounded">
                <X size={20} />
            </button>
        </div>
        <div class="flex border-b border-border">
            <button class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'general' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}" onclick={() => activeTab = 'general'}>通用</button>
            <button class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'trigger' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}" onclick={() => activeTab = 'trigger'}>触发设置</button>
            <button class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'appearance' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}" onclick={() => activeTab = 'appearance'}>主题</button>
        </div>
        <div class="flex-1 overflow-y-auto">
            {#if activeTab === 'general'}
                <div class="p-6 space-y-6">
                    <div class="flex flex-col items-center gap-2">
                        <button
                            onclick={() => showAvatarModal = true}
                            class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center text-primary hover:ring-2 hover:ring-primary/30 transition-all"
                        >
                            {#if userAvatar}
                                <img src={resolveAvatarUrl(userAvatar)} alt="用户头像" class="w-full h-full rounded-full object-cover" />
                            {:else}
                                <User size={28} />
                            {/if}
                        </button>
                        <span class="text-xs text-text-secondary">点击更换头像</span>
                    </div>
                </div>
            {:else if activeTab === 'trigger'}
                <div class="p-6 space-y-6">
                    <div>
                        <label class="block text-sm font-medium mb-1">角色触发消息间隔（秒）</label>
                        <input
                            type="number"
                            min="0"
                            bind:value={draft.global_min_trigger_interval}
                            class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                        />
                        <p class="text-xs text-text-secondary mt-1">0 = 不限制，>0 = 防止角色被连续调用的最小间隔秒数</p>
                    </div>

                    <div>
                        <h3 class="font-semibold mb-2">安静时段</h3>
                        <label class="flex items-center gap-2 mb-2">
                            <input type="checkbox" bind:checked={quietHoursEnabled} />
                            <span>启用安静时段</span>
                        </label>
                        {#if quietHoursEnabled}
                            <div class="flex gap-2 items-center">
                                <input type="time" bind:value={quietStart} class="px-2 py-1 bg-bg border border-border rounded input-field" />
                                <span>~</span>
                                <input type="time" bind:value={quietEnd} class="px-2 py-1 bg-bg border border-border rounded input-field" />
                            </div>
                            <p class="text-xs text-text-secondary mt-1">在此期间，所有主动会话和定时任务均不会触发（到达后顺延）。</p>
                        {/if}
                    </div>
                </div>
            {:else if activeTab === 'appearance'}
                <div class="p-6">
                    <h3 class="text-lg font-semibold text-text mb-1">选择主题</h3>
                    <p class="text-sm text-text-secondary mb-4">切换后立即生效</p>
                    
                    <div class="grid grid-cols-2 gap-4">
                        {#each themeStore.themes as theme}
                            <button
                                class="relative rounded-lg overflow-hidden border-2 transition-all cursor-pointer text-left
                                       {themeStore.activeThemeId === theme.id
                                           ? 'border-primary shadow-md'
                                           : 'border-border hover:border-primary/40'}"
                                onclick={() => themeStore.applyTheme(theme.id)}
                            >
                                <!-- Preview image or gradient placeholder -->
                                <div class="h-20 bg-surface flex items-center justify-center">
                                    {#if theme.preview_path}
                                        <img
                                            src={convertFileSrc(theme.preview_path)}
                                            alt={theme.name}
                                            class="w-full h-full object-cover"
                                        />
                                    {:else}
                                        <div class="w-full h-full bg-gradient-to-br from-bg to-surface"></div>
                                    {/if}
                                </div>
                                <!-- Info row -->
                                <div class="p-3 bg-surface">
                                    <div class="flex items-center justify-between">
                                        <span class="text-sm font-medium text-text">{theme.name}</span>
                                        {#if themeStore.activeThemeId === theme.id}
                                            <span class="w-5 h-5 rounded-full bg-primary flex items-center justify-center">
                                                <span class="text-white text-xs">✓</span>
                                            </span>
                                        {/if}
                                    </div>
                                    <span class="text-xs text-text-secondary">
                                        {theme.source === 'builtin' ? '内置' : '用户'}
                                    </span>
                                </div>
                            </button>
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
        <div class="p-4 border-t border-border flex justify-end">
            <button
                onclick={handleSave}
                disabled={saving}
                class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary"
            >
                {saving ? '保存中...' : '保存'}
            </button>
        </div>
    </div>
</div>

<AvatarUploadModal
    open={showAvatarModal}
    targetType="user_default"
    targetId="user"
    currentAvatar={userAvatar}
    onClose={() => showAvatarModal = false}
    onUploaded={(path) => {
        userAvatar = path;
        showAvatarModal = false;
    }}
/>
