<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { X, User } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import ModelConfigPanel from './ModelConfigPanel.svelte';
    import VoiceCachePanel from './VoiceCachePanel.svelte';
    import { themeStore } from '$lib/stores/themeStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';

    onMount(() => {
        themeStore.loadThemes();
        modelConfigStore.load();
    });

    let draft = $state({ global_min_trigger_interval: 30, summary_model_config_id: null as string | null });
    let saving = $state(false);
    let mouseDownOnOverlay = $state(false);
    let showAvatarModal = $state(false);
    let userAvatar = $state<string | null>(null);
    let activeTab = $state('general');

    let quietHoursEnabled = $state(false);
    let quietStart = $state('00:00');
    let quietEnd = $state('08:00');

    // 搜索 API
    let searchProvider = $state('');
    let searchApiKey = $state('');
    let searchKeySet = $state(false);
    let testingSearch = $state(false);
    let searchTestResult = $state<{ ok: boolean; msg: string } | null>(null);

    // 虚拟时间
    let virtualTimeEnabled = $state(false);
    let virtualTimeInput = $state('');
    let virtualTimeRate = $state(1);
    let vtStoredBase = $state<number | null>(null);
    let vtStoredSetAt = $state<number | null>(null);
    let vtPreview = $state('');

    function msToDatetimeLocal(ms: number): string {
        const d = new Date(ms);
        const pad = (n: number) => String(n).padStart(2, '0');
        return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
    }

    function formatDateTime(d: Date): string {
        const pad = (n: number) => String(n).padStart(2, '0');
        return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
    }

    // 虚拟时间预览：与后端公式一致 virtual = base + (real_now - set_at) * rate
    function previewVirtualNow(): Date | null {
        const now = Date.now();
        const rate = Math.max(1, Math.floor(Number(virtualTimeRate)) || 1);
        let base: number | null = null;
        let setAt = now;
        if (virtualTimeInput) {
            const v = new Date(virtualTimeInput).getTime();
            if (!isNaN(v)) base = v;
        }
        if (base == null) return null;
        // 输入未改动时沿用已存的 set_at，预览才与后端一致
        if (vtStoredBase != null && vtStoredSetAt != null && base === vtStoredBase) {
            setAt = vtStoredSetAt;
        }
        return new Date(base + (now - setAt) * rate);
    }

    $effect(() => {
        if (!virtualTimeEnabled) {
            vtPreview = '';
            return;
        }
        // 依赖 virtualTimeInput / virtualTimeRate 等状态，变化时重建计时器
        void virtualTimeInput;
        void virtualTimeRate;
        const update = () => {
            const d = previewVirtualNow();
            vtPreview = d ? formatDateTime(d) : '请先设定时间';
        };
        update();
        const timer = setInterval(update, 1000);
        return () => clearInterval(timer);
    });

    async function testSearchConnection() {
        if (!searchProvider) return;
        testingSearch = true;
        searchTestResult = null;
        try {
            const msg = await invoke<string>('test_search_api', {
                provider: searchProvider,
                apiKey: searchApiKey || null,
            });
            searchTestResult = { ok: true, msg };
        } catch (err) {
            searchTestResult = { ok: false, msg: String(err) };
        } finally {
            testingSearch = false;
        }
    }

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
                summary_model_config_id: settingsStore.settings.summary_model_config_id,
            };
            quietHoursEnabled = (settingsStore.settings.quiet_hours_start ?? -1) >= 0;
            quietStart = minutesToTime(settingsStore.settings.quiet_hours_start ?? 0);
            quietEnd = minutesToTime(settingsStore.settings.quiet_hours_end ?? 480);
            searchProvider = settingsStore.settings.search_provider ?? '';
            searchKeySet = settingsStore.settings.search_api_key_set ?? false;
            virtualTimeEnabled = settingsStore.settings.virtual_time_enabled ?? false;
            virtualTimeRate = settingsStore.settings.virtual_time_rate ?? 1;
            vtStoredBase = settingsStore.settings.virtual_time_base ?? null;
            vtStoredSetAt = settingsStore.settings.virtual_time_set_at ?? null;
            virtualTimeInput = vtStoredBase != null ? msToDatetimeLocal(vtStoredBase) : '';
        }
    });

    async function handleSave() {
        saving = true;
        try {
            // 虚拟时间：把当前正在显示的虚拟时间作为新 base 提交，保证只改流速时时间不跳变
            let vtBase: number | undefined;
            if (virtualTimeEnabled) {
                vtBase = previewVirtualNow()?.getTime() ?? Date.now();
            }
            await settingsStore.update({
                global_min_trigger_interval: draft.global_min_trigger_interval,
                summary_model_config_id: draft.summary_model_config_id,
                search_provider: searchProvider || null,
                ...(searchApiKey ? { search_api_key: searchApiKey } : {}),
                virtual_time_enabled: virtualTimeEnabled,
                virtual_time_base: vtBase,
                virtual_time_rate: Math.max(1, Math.floor(Number(virtualTimeRate)) || 1),
            });
            searchApiKey = ''; // 保存成功后清空输入框，避免明文驻留
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
            toastStore.success('已保存', 2000);
        } catch (err) {
            toastStore.error(`保存失败：${err}`);
        } finally {
            saving = false;
        }
    }

    let { onclose }: { onclose: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 modal-overlay"
    onmousedown={(e) => { mouseDownOnOverlay = e.target === e.currentTarget; }}
    onclick={(e) => { if (mouseDownOnOverlay && e.target === e.currentTarget) onclose(); mouseDownOnOverlay = false; }}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[80vh] flex flex-col modal-card" onmousedown={() => mouseDownOnOverlay = false}>
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
            <button class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'models' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}" onclick={() => activeTab = 'models'}>模型</button>
            <button class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'voice' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}" onclick={() => activeTab = 'voice'}>语音缓存</button>
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

                    <!-- 搜索 API -->
                    <div class="border-t border-border pt-4">
                        <h3 class="font-semibold mb-1">搜索 API</h3>
                        <p class="text-xs text-text-secondary mb-3">配置后，AI 生成人设时可勾选“启用搜索”，让 AI 联网搜集资料（多轮搜索）。</p>
                        <label class="block text-sm font-medium mb-1">搜索厂商</label>
                        <select
                            bind:value={searchProvider}
                            class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                        >
                            <option value="">未配置</option>
                            <option value="bocha">博查</option>
                            <option value="zhipu">智谱</option>
                            <option value="kimi">Kimi（Moonshot）</option>
                        </select>
                        {#if searchProvider}
                            <label class="block text-sm font-medium mb-1 mt-3">API Key</label>
                            <input
                                type="password"
                                bind:value={searchApiKey}
                                placeholder={searchKeySet ? '已保存（输入新 Key 可覆盖）' : '请输入 API Key'}
                                class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                            />
                            {#if searchProvider !== (settingsStore.settings?.search_provider ?? '')}
                                <p class="text-xs text-amber-500 mt-1">已切换厂商，需重新填写该厂商的 Key</p>
                            {/if}
                            <div class="flex items-center gap-2 mt-2">
                                <button
                                    onclick={testSearchConnection}
                                    disabled={testingSearch || (!searchApiKey && !searchKeySet)}
                                    class="px-3 py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors disabled:opacity-50"
                                >
                                    {testingSearch ? '测试中...' : '测试连接'}
                                </button>
                                {#if searchTestResult}
                                    <span class="text-xs {searchTestResult.ok ? 'text-green-500' : 'text-red-500'}">{searchTestResult.msg}</span>
                                {/if}
                            </div>
                        {/if}
                    </div>

                    <!-- 虚拟时间 -->
                    <div class="border-t border-border pt-4">
                        <h3 class="font-semibold mb-1">虚拟时间</h3>
                        <p class="text-xs text-text-secondary mb-3">启用后，注入给 AI 角色的时间以虚拟时间为准（仅影响对话提示词中的时间，定时器等仍按真实时间运行）。</p>
                        <label class="flex items-center gap-2 mb-2">
                            <input type="checkbox" bind:checked={virtualTimeEnabled} />
                            <span>启用虚拟时间</span>
                        </label>
                        {#if virtualTimeEnabled}
                            <label class="block text-sm font-medium mb-1">设定时间</label>
                            <input
                                type="datetime-local"
                                bind:value={virtualTimeInput}
                                class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                            />
                            <label class="block text-sm font-medium mb-1 mt-3">时间流速</label>
                            <div class="flex items-center gap-2">
                                <span class="text-sm">现实 1 分钟 = 虚拟</span>
                                <input
                                    type="number"
                                    min="1"
                                    step="1"
                                    bind:value={virtualTimeRate}
                                    class="w-20 px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                                />
                                <span class="text-sm">分钟</span>
                            </div>
                            <p class="text-xs text-text-secondary mt-3">当前虚拟时间：<span class="font-mono">{vtPreview || '—'}</span></p>
                        {/if}
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
            {:else if activeTab === 'models'}
                <ModelConfigPanel />
                <div class="px-6 pb-6">
                    <div class="mt-6 pt-6 border-t border-border">
                        <h4 class="font-medium mb-2">标题总结模型</h4>
                        <select
                            value={draft.summary_model_config_id ?? ''}
                            onchange={(e) => draft.summary_model_config_id = e.currentTarget.value || null}
                            class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                        >
                            <option value="">自动选择（第一个可用模型）</option>
                            {#each modelConfigStore.configs as cfg}
                                <option value={cfg.id}>{cfg.name} ({cfg.model_name})</option>
                            {/each}
                        </select>
                        <p class="text-xs text-text-secondary mt-1">
                            重置会话时，用于总结聊天记录生成历史页面标题。不选则自动使用第一个配置了 API Key 的模型。
                        </p>
                    </div>
                </div>
            {:else if activeTab === 'voice'}
                <div class="p-6">
                    <h3 class="text-lg font-semibold text-text mb-2">语音缓存</h3>
                    <p class="text-xs text-text-secondary mb-4">
                        语音文件保存在 data\vits_cache\ 目录下，按会话分子目录存放。
                    </p>
                    <VoiceCachePanel />
                </div>
            {:else if activeTab === 'appearance'}
                <div class="p-6">
                    <h3 class="text-lg font-semibold text-text mb-4">选择主题</h3>

                    {#if themeStore.themes.length === 0}
                        <div class="text-sm text-text-secondary py-8 text-center">
                            暂无可用主题
                        </div>
                    {:else}
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
                    {/if}
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
