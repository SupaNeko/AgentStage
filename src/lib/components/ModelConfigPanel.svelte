<script lang="ts">
    import { Plus, Pencil, Trash2, Wifi, Loader2, ChevronDown, ChevronUp, X } from 'lucide-svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { PROVIDER_DEFAULTS } from '$lib/modelConfig';
    import type { ModelConfig } from '$lib/types';

    let editingConfig = $state<Partial<ModelConfig> & { id?: string } | null>(null);
    let showAdvanced = $state(false);
    let testingId = $state<string | null>(null);
    let testResult = $state<{ configId: string; success: boolean; latencyMs: number; message: string } | null>(null);
    let apiKeyVisible = $state(false);
    let deletingId = $state<string | null>(null);

    const emptyConfig = (): Omit<ModelConfig, 'id' | 'created_at' | 'updated_at'> => ({
        name: '',
        provider: 'openai',
        model_name: PROVIDER_DEFAULTS.openai.modelName,
        base_url: PROVIDER_DEFAULTS.openai.baseUrl,
        api_key: '',
        temperature: null,
        max_tokens: null,
        top_p: null,
        presence_penalty: null,
        frequency_penalty: null,
    });

    function applyProviderDefaults(provider: string) {
        if (!editingConfig) return;
        const defaults = PROVIDER_DEFAULTS[provider] ?? PROVIDER_DEFAULTS.custom;
        editingConfig.model_name = defaults.modelName;
        editingConfig.base_url = defaults.baseUrl;
    }

    function handleAdd() {
        editingConfig = { ...emptyConfig() };
        showAdvanced = false;
        testResult = null;
        apiKeyVisible = false;
    }

    function handleEdit(config: ModelConfig) {
        editingConfig = { ...config };
        showAdvanced = false;
        testResult = null;
        apiKeyVisible = false;
    }

    function handleCancel() {
        editingConfig = null;
    }

    async function handleSave() {
        if (!editingConfig) return;
        if (!editingConfig.name?.trim()) {
            toastStore.show('名称不能为空', 'error', 3000);
            return;
        }
        if (!editingConfig.model_name?.trim()) {
            toastStore.show('模型名称不能为空', 'error', 3000);
            return;
        }
        if (!editingConfig.api_key?.trim()) {
            toastStore.show('API Key 不能为空', 'error', 3000);
            return;
        }

        const payload = {
            name: editingConfig.name.trim(),
            provider: editingConfig.provider ?? 'openai',
            model_name: editingConfig.model_name.trim(),
            base_url: editingConfig.base_url?.trim() || null,
            api_key: editingConfig.api_key.trim(),
            temperature: editingConfig.temperature ?? null,
            max_tokens: editingConfig.max_tokens ?? null,
            top_p: editingConfig.top_p ?? null,
            presence_penalty: editingConfig.presence_penalty ?? null,
            frequency_penalty: editingConfig.frequency_penalty ?? null,
        };

        try {
            if (editingConfig.id) {
                await modelConfigStore.update(editingConfig.id, payload);
                toastStore.show('已保存', 'success', 2000);
            } else {
                await modelConfigStore.create(payload);
                toastStore.show('已创建', 'success', 2000);
            }
            editingConfig = null;
        } catch (err: any) {
            toastStore.show('保存失败: ' + String(err), 'error', 5000);
        }
    }

    async function handleTestConnection(config: ModelConfig) {
        testingId = config.id;
        testResult = null;
        try {
            const result = await modelConfigStore.testConnection(config.id);
            testResult = {
                configId: config.id,
                success: result.success,
                latencyMs: result.latency_ms,
                message: result.message,
            };
            if (result.success) {
                toastStore.show(`连接成功 (${result.latency_ms}ms)`, 'success', 3000);
            } else {
                toastStore.show(`连接失败: ${result.message}`, 'error', 5000);
            }
        } catch (err: any) {
            toastStore.show('测试失败: ' + String(err), 'error', 5000);
        } finally {
            testingId = null;
        }
    }

    async function handleDelete(config: ModelConfig) {
        if (!confirm(`确定要删除模型配置 "${config.name}" 吗？`)) return;
        deletingId = config.id;
        try {
            await modelConfigStore.delete(config.id);
            toastStore.show('已删除', 'success', 2000);
        } catch (err: any) {
            const msg = String(err);
            if (msg.includes('被角色引用') || msg.includes('foreign key') || msg.includes('FOREIGN KEY')) {
                toastStore.show('该模型配置正被角色引用，无法删除', 'error', 5000);
            } else {
                toastStore.show('删除失败: ' + msg, 'error', 5000);
            }
        } finally {
            deletingId = null;
        }
    }

    function formatProviderLabel(provider: string): string {
        const labels: Record<string, string> = {
            openai: 'OpenAI',
            anthropic: 'Anthropic',
            google: 'Google',
            kimi: 'Kimi (Moonshot)',
            minimax: 'MiniMax',
            custom: '自定义',
        };
        return labels[provider] ?? provider;
    }
</script>

<div class="p-6 space-y-6">
    {#if editingConfig}
        <!-- Edit/Create Form -->
        <div class="bg-surface border border-border rounded-xl p-4 space-y-4">
            <div class="flex items-center justify-between">
                <h4 class="font-semibold">{editingConfig.id ? '编辑模型配置' : '添加模型配置'}</h4>
                <button onclick={handleCancel} class="p-1 hover:bg-gray-100 rounded transition-colors">
                    <X size={18} />
                </button>
            </div>

            <div class="grid grid-cols-2 gap-4">
                <div class="col-span-2">
                    <label class="block text-sm font-medium mb-1">名称 <span class="text-red-500">*</span></label>
                    <input
                        type="text"
                        bind:value={editingConfig.name}
                        placeholder="例如：OpenAI GPT-4o"
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                    />
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1">提供商 <span class="text-red-500">*</span></label>
                    <select
                        value={editingConfig.provider ?? 'custom'}
                        onchange={(e) => {
                            if (!editingConfig) return;
                            const newProvider = e.currentTarget.value;
                            editingConfig.provider = newProvider;
                            applyProviderDefaults(newProvider);
                        }}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                    >
                        <option value="openai">OpenAI</option>
                        <option value="anthropic">Anthropic</option>
                        <option value="google">Google</option>
                        <option value="kimi">Kimi (Moonshot)</option>
                        <option value="minimax">MiniMax</option>
                        <option value="custom">自定义</option>
                    </select>
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1">模型名称 <span class="text-red-500">*</span></label>
                    <input
                        type="text"
                        bind:value={editingConfig.model_name}
                        placeholder="gpt-4o, claude-3-sonnet, kimi-k2..."
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                    />
                </div>
                <div class="col-span-2">
                    <label class="block text-sm font-medium mb-1">Base URL</label>
                    <input
                        type="text"
                        bind:value={editingConfig.base_url}
                        placeholder="可选，默认使用官方地址"
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                    />
                </div>
                <div class="col-span-2">
                    <label class="block text-sm font-medium mb-1">API Key <span class="text-red-500">*</span></label>
                    <div class="relative">
                        <input
                            type={apiKeyVisible ? 'text' : 'password'}
                            bind:value={editingConfig.api_key}
                            placeholder="sk-..."
                            class="w-full px-3 py-2 pr-10 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                        />
                        <button
                            type="button"
                            onclick={() => apiKeyVisible = !apiKeyVisible}
                            class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary transition-colors"
                            title={apiKeyVisible ? '隐藏' : '显示'}
                        >
                            {#if apiKeyVisible}
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9.88 9.88a3 3 0 1 0 4.24 4.24"/><path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><line x1="2" x2="22" y1="2" y2="22"/></svg>
                            {:else}
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
                            {/if}
                        </button>
                    </div>
                </div>
            </div>

            <!-- Advanced -->
            <div>
                <button
                    type="button"
                    onclick={() => showAdvanced = !showAdvanced}
                    class="flex items-center gap-1 text-sm text-text-secondary hover:text-text-primary transition-colors"
                >
                    {#if showAdvanced}
                        <ChevronUp size={16} />
                    {:else}
                        <ChevronDown size={16} />
                    {/if}
                    <span>高级</span>
                </button>
                {#if showAdvanced}
                    <div class="mt-3 grid grid-cols-2 gap-4">
                        <div>
                            <label class="block text-sm font-medium mb-1">Temperature</label>
                            <input
                                type="number"
                                bind:value={editingConfig.temperature}
                                min={0}
                                max={2}
                                step={0.1}
                                placeholder="默认"
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                            />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1">Max Tokens</label>
                            <input
                                type="number"
                                bind:value={editingConfig.max_tokens}
                                min={1}
                                placeholder="默认"
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                            />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1">Top P</label>
                            <input
                                type="number"
                                bind:value={editingConfig.top_p}
                                min={0}
                                max={1}
                                step={0.01}
                                placeholder="默认"
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                            />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1">Presence Penalty</label>
                            <input
                                type="number"
                                bind:value={editingConfig.presence_penalty}
                                min={-2}
                                max={2}
                                step={0.1}
                                placeholder="默认"
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                            />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1">Frequency Penalty</label>
                            <input
                                type="number"
                                bind:value={editingConfig.frequency_penalty}
                                min={-2}
                                max={2}
                                step={0.1}
                                placeholder="默认"
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-bg input-field"
                            />
                        </div>
                    </div>
                {/if}
            </div>

            <div class="flex justify-end gap-3 pt-2">
                <button
                    type="button"
                    onclick={handleCancel}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors"
                >
                    取消
                </button>
                <button
                    type="button"
                    onclick={handleSave}
                    class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors btn-primary"
                >
                    {editingConfig.id ? '保存' : '创建'}
                </button>
            </div>
        </div>
    {:else}
        <!-- List View -->
        <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold">模型配置</h3>
            <button
                onclick={handleAdd}
                class="flex items-center gap-1.5 px-3 py-1.5 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors"
            >
                <Plus size={16} />
                <span>添加模型</span>
            </button>
        </div>

        {#if modelConfigStore.loading}
            <div class="flex items-center justify-center py-12 text-text-secondary">
                <Loader2 size={20} class="animate-spin mr-2" />
                加载中...
            </div>
        {:else if modelConfigStore.configs.length === 0}
            <div class="text-center py-12 text-text-secondary text-sm">
                <p>还没有配置任何模型</p>
                <p class="text-xs mt-1">点击"添加模型"开始配置</p>
            </div>
        {:else}
            <div class="space-y-3">
                {#each modelConfigStore.configs as config (config.id)}
                    <div class="border border-border rounded-lg p-4 bg-surface hover:border-primary/30 transition-colors">
                        <div class="flex items-center justify-between">
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center gap-2">
                                    <span class="font-medium text-sm">{config.name}</span>
                                    <span class="text-xs px-1.5 py-0.5 bg-bg border border-border rounded text-text-secondary">
                                        {formatProviderLabel(config.provider)}
                                    </span>
                                </div>
                                <p class="text-xs text-text-secondary mt-1 truncate">
                                    {config.model_name}
                                    {#if config.base_url}
                                        · {config.base_url}
                                    {/if}
                                </p>
                            </div>
                            <div class="flex items-center gap-2 ml-4 shrink-0">
                                <button
                                    onclick={() => handleTestConnection(config)}
                                    disabled={testingId === config.id}
                                    class="flex items-center gap-1 px-2 py-1.5 text-xs border border-border rounded-lg hover:bg-bg transition-colors disabled:opacity-50"
                                    title="测试连接"
                                >
                                    {#if testingId === config.id}
                                        <Loader2 size={12} class="animate-spin" />
                                    {:else}
                                        <Wifi size={12} />
                                    {/if}
                                    <span>测试</span>
                                </button>
                                <button
                                    onclick={() => handleEdit(config)}
                                    class="p-1.5 text-text-secondary hover:text-text-primary hover:bg-bg rounded-lg transition-colors"
                                    title="编辑"
                                >
                                    <Pencil size={14} />
                                </button>
                                <button
                                    onclick={() => handleDelete(config)}
                                    disabled={deletingId === config.id}
                                    class="p-1.5 text-text-secondary hover:text-red-600 hover:bg-red-50 rounded-lg transition-colors disabled:opacity-50"
                                    title="删除"
                                >
                                    {#if deletingId === config.id}
                                        <Loader2 size={14} class="animate-spin" />
                                    {:else}
                                        <Trash2 size={14} />
                                    {/if}
                                </button>
                            </div>
                        </div>
                        {#if testResult && testResult.configId === config.id}
                            <div class="mt-2">
                                {#if testResult.success}
                                    <div class="flex items-center gap-2 text-sm text-green-600 bg-green-50 px-3 py-2 rounded-lg">
                                        <span>✅</span>
                                        <span>连接成功 ({testResult.latencyMs}ms)</span>
                                    </div>
                                {:else}
                                    <div class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-3 py-2 rounded-lg">
                                        <span>❌</span>
                                        <span>{testResult.message}</span>
                                    </div>
                                {/if}
                            </div>
                        {/if}
                    </div>
                {/each}
            </div>
        {/if}
    {/if}
</div>
