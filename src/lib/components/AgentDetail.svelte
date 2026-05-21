<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, Trash2, Save, Loader2, MessageSquare, Sparkles, Wifi, Import, Eye, EyeOff } from 'lucide-svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import { resolveAvatarUrl } from '$lib/utils';
    import { PROVIDER_DEFAULTS } from '$lib/modelConfig';
    import type { Agent } from '$lib/types';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import PersonaGenerateModal from './PersonaGenerateModal.svelte';
    import AgentRelationshipPanel from './AgentRelationshipPanel.svelte';
    import ImportModelConfigModal from './ImportModelConfigModal.svelte';
    import AgentMemoryPanel from './AgentMemoryPanel.svelte';
    import AgentTimerPanel from './AgentTimerPanel.svelte';

    let agent = $state<Agent | null>(null);
    let loading = $state(false);
    let saving = $state(false);
    let error = $state('');
    let showAvatarModal = $state(false);
    let showGenerateModal = $state(false);
    let activeTab = $state<'config' | 'relationships' | 'memory' | 'timer'>('config');
    let testingApi = $state(false);
    let testResult = $state<{ success: boolean; latencyMs: number; message: string } | null>(null);
    let showImportModal = $state(false);
    let apiKeyVisible = $state(false);

    // Form state
    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        model_provider: 'openai',
        model_name: '',
        base_url: '',
        api_key: '',
        temperature: 0.7,
        max_tokens: 2048,
        thinking_mode: false,
        long_term_memory: '',
        memory_enabled: true,
    });

    function handleProviderChange() {
        const defaults = PROVIDER_DEFAULTS[form.model_provider] ?? PROVIDER_DEFAULTS.custom;
        form.model_name = defaults.modelName;
        form.base_url = defaults.baseUrl;
        testResult = null;
    }

    async function handleTestApi() {
        if (!form.api_key) {
            toastStore.show('请先填写 API Key', 'error', 3000);
            return;
        }
        if (!form.model_name) {
            toastStore.show('请先填写模型名称', 'error', 3000);
            return;
        }
        testingApi = true;
        testResult = null;
        try {
            const result = await invoke<{ success: boolean; latency_ms: number; message: string }>('test_api_connection', {
                req: {
                    model_provider: form.model_provider,
                    model_name: form.model_name,
                    base_url: form.base_url || null,
                    api_key: form.api_key,
                }
            });
            testResult = { success: result.success, latencyMs: result.latency_ms, message: result.message };
            if (result.success) {
                toastStore.show(`连接成功 (${result.latency_ms}ms)`, 'success', 3000);
            } else {
                toastStore.show(`连接失败: ${result.message}`, 'error', 5000);
            }
        } catch (err: any) {
            toastStore.show('测试失败: ' + String(err), 'error', 5000);
        } finally {
            testingApi = false;
        }
    }

    async function loadAgent(id: string) {
        logger.debug('[DEBUG AgentDetail.loadAgent]', { id });
        loading = true;
        error = '';
        try {
            const result = await invoke<Agent>('get_agent', { id });
            agent = result;
            logger.debug('[DEBUG AgentDetail.loadAgent] success', { id });
            if (result) {
                const provider = result.model_provider || 'openai';
                const defaults = PROVIDER_DEFAULTS[provider] ?? PROVIDER_DEFAULTS.custom;
                form = {
                    name: result.name,
                    detailed_persona: result.detailed_persona,
                    simplified_persona: result.simplified_persona,
                    model_provider: provider,
                    model_name: result.model_name || defaults.modelName,
                    base_url: result.base_url || defaults.baseUrl,
                    api_key: result.api_key || '',
                    temperature: result.temperature,
                    max_tokens: result.max_tokens,
                    thinking_mode: result.thinking_mode ?? false,
                    long_term_memory: result.long_term_memory || '',
                    memory_enabled: result.memory_enabled ?? true,
                };
            }
        } catch (err) {
            logger.debug('[DEBUG AgentDetail.loadAgent] failed', { id, error: err });
            logger.error('Failed to load agent:', err);
            error = '加载角色信息失败';
        } finally {
            loading = false;
        }
    }

    async function handleSave() {
        if (!agent) return;
        logger.debug('[DEBUG AgentDetail.handleSave]', { id: agent.id });
        saving = true;
        error = '';
        try {
            const updateReq: Record<string, unknown> = {
                id: agent.id,
                name: form.name,
                detailed_persona: form.detailed_persona,
                simplified_persona: form.simplified_persona,
                model_provider: form.model_provider,
                model_name: form.model_name,
                base_url: form.base_url || null,
                temperature: form.temperature,
                max_tokens: form.max_tokens,
                thinking_mode: form.thinking_mode,
                api_key: form.api_key,
            };
            const updated = await invoke<Agent>('update_agent', { req: updateReq });
            agent = updated;
            logger.debug('[DEBUG AgentDetail.handleSave] success', { id: agent.id });
            toastStore.show('已保存', 'success', 2000);
        } catch (err) {
            logger.debug('[DEBUG AgentDetail.handleSave] failed', { id: agent?.id, error: err });
            logger.error('Failed to update agent:', err);
            error = '保存失败: ' + String(err);
        } finally {
            saving = false;
        }
    }

    async function handleDelete() {
        if (!agent) return;
        logger.debug('[DEBUG AgentDetail.handleDelete]', { id: agent.id });
        if (!confirm(`确定要删除角色 "${agent.name}" 吗？此操作不可恢复。`)) return;
        try {
            await invoke('delete_agent', { req: { id: agent.id } });
            await agentStore.loadAgents();
            appState.selectAgent(null);
        } catch (err) {
            logger.error('Failed to delete agent:', err);
            error = '删除失败: ' + String(err);
        }
    }

    function handleImportModelConfig(sourceAgent: Agent) {
        form.model_provider = sourceAgent.model_provider || 'openai';
        form.model_name = sourceAgent.model_name || '';
        form.base_url = sourceAgent.base_url || '';
        form.temperature = sourceAgent.temperature;
        form.max_tokens = sourceAgent.max_tokens;
        form.thinking_mode = sourceAgent.thinking_mode ?? false;
        testResult = null;
        toastStore.show(`已导入 "${sourceAgent.name}" 的模型配置`, 'success', 2000);
    }

    async function handleStartChat() {
        if (!agent) return;
        logger.debug('[DEBUG AgentDetail.handleStartChat]', { agentId: agent.id });
        try {
            const session = await invoke<import('$lib/types').Session>('create_private_session', { req: { agent_id: agent.id } });
            logger.debug('[DEBUG AgentDetail.handleStartChat] success', { agentId: agent.id, sessionId: session.id });
            sessionStore.addSession(session);
            sessionStore.selectSession(session.id);
            appState.switchView('chat');
        } catch (err) {
            logger.error('Failed to create session:', err);
            error = '创建会话失败: ' + String(err);
        }
    }

    // Watch for selected agent changes
    $effect(() => {
        const id = appState.selectedAgentId;
        if (id) {
            loadAgent(id);
        } else {
            agent = null;
        }
    });
</script>

<div class="flex flex-col h-full bg-bg">
    {#if loading}
        <div class="flex items-center justify-center h-full text-text-secondary">
            <Loader2 size={24} class="animate-spin mr-2" />
            加载中...
        </div>
    {:else if !agent}
        <div class="flex flex-col items-center justify-center h-full text-text-secondary">
            <Bot size={48} class="mb-4 opacity-50" />
            <p>请在左侧选择一个角色查看详情</p>
            <p class="text-sm mt-1">或点击"新建"创建新角色</p>
        </div>
    {:else}
        <!-- Header -->
        <header class="flex items-center justify-between px-6 py-4 border-b border-border bg-surface">
            <div class="flex items-center gap-3">
                <button
                    onclick={() => showAvatarModal = true}
                    class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary hover:ring-2 hover:ring-primary/30 transition-all"
                >
                    {#if agent.avatar_path}
                        <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                    {:else}
                        <Bot size={20} />
                    {/if}
                </button>
                <div>
                    <h2 class="text-lg font-semibold">{agent.name}</h2>
                    <p class="text-xs text-text-secondary">{agent.model_name || '未配置模型'}</p>
                </div>
            </div>
            <div class="flex items-center gap-2">
                <button onclick={handleStartChat} class="flex items-center gap-1.5 px-3 py-1.5 bg-primary text-white text-sm hover:bg-primary-dark rounded-lg transition-colors">
                    <MessageSquare size={16} />
                    <span>开始聊天</span>
                </button>
                <button onclick={handleDelete} class="flex items-center gap-1.5 px-3 py-1.5 text-red-600 text-sm hover:bg-red-50 rounded-lg transition-colors">
                    <Trash2 size={16} />
                    <span>删除</span>
                </button>
            </div>
        </header>

        <!-- Tabs -->
        <div class="px-6 border-b border-border bg-surface">
            <div class="flex gap-4">
                <button
                    onclick={() => activeTab = 'config'}
                    class="py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'config' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text-primary'}"
                >
                    角色配置
                </button>
                <button
                    onclick={() => activeTab = 'relationships'}
                    class="py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'relationships' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text-primary'}"
                >
                    关系设定
                </button>
                <button
                    onclick={() => activeTab = 'memory'}
                    class="py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'memory' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text-primary'}"
                >
                    记忆
                </button>
                <button
                    onclick={() => activeTab = 'timer'}
                    class="py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'timer' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text-primary'}"
                >
                    定时任务
                </button>
            </div>
        </div>

        <!-- Content -->
        <div class="flex-1 overflow-y-auto px-6 py-4">
            {#if error}
                <div class="mb-4 p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {/if}

            {#if activeTab === 'config'}
                <div class="max-w-2xl space-y-5">
                    <!-- Basic Info -->
                    <div>
                        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">基本信息</h3>
                        <div class="space-y-3">
                            <div>
                                <label class="block text-sm font-medium mb-1">角色名称 <span class="text-red-500">*</span></label>
                                <input type="text" bind:value={form.name} maxlength={20}
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface" />
                            </div>
                        </div>
                    </div>

                    <!-- Persona -->
                    <div>
                        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">人设配置</h3>
                        <div class="space-y-3">
                            <div>
                                <label class="block text-sm font-medium mb-1">详细人设 <span class="text-red-500">*</span></label>
                                <textarea bind:value={form.detailed_persona} rows={5}
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface"
                                    placeholder="你是 Fate/stay night 中的角色卫宫士郎，性格坚韧不拔，内心温柔但执拗，拥有强烈的正义感..."></textarea>
                                <p class="text-xs text-text-secondary mt-1">角色自己看到的完整设定，直接注入 System Prompt</p>
                            </div>
                            <div>
                                <label class="block text-sm font-medium mb-1">简易人设 <span class="text-red-500">*</span></label>
                                <textarea bind:value={form.simplified_persona} rows={2}
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface"
                                    placeholder="出自 Fate/stay night 的角色卫宫士郎，冬木市的见习魔术师，性格正义感强烈。"></textarea>
                                <p class="text-xs text-text-secondary mt-1">给其它角色看的角色名片（角色简介）</p>
                            </div>
                        </div>
                    </div>

                    <!-- Model Config -->
                    <div>
                        <div class="flex items-center justify-between mb-3">
                            <h3 class="text-sm font-medium text-text-secondary uppercase tracking-wide">模型配置</h3>
                            <button
                                type="button"
                                onclick={() => showImportModal = true}
                                class="flex items-center gap-1.5 text-xs text-primary hover:text-primary-dark transition-colors"
                            >
                                <Import size={13} />
                                <span>从其他角色导入</span>
                            </button>
                        </div>
                    <div class="grid grid-cols-2 gap-4">
                        <div>
                            <label class="block text-sm font-medium mb-1">模型提供商 <span class="text-red-500">*</span></label>
                            <select bind:value={form.model_provider} onchange={handleProviderChange}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface">
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
                            <input type="text" bind:value={form.model_name}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                                placeholder="gpt-4o, claude-3-sonnet, kimi-k2..." />
                        </div>
                        <div class="col-span-2">
                            <label class="block text-sm font-medium mb-1">Base URL</label>
                            <input type="text" bind:value={form.base_url}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                                placeholder="可选，默认使用官方地址" />
                        </div>
                        <div class="col-span-2 flex gap-3 items-end">
                            <div class="flex-1">
                                <label class="block text-sm font-medium mb-1">API Key</label>
                                <div class="relative">
                                    <input type={apiKeyVisible ? 'text' : 'password'} bind:value={form.api_key}
                                        class="w-full px-3 py-2 pr-10 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface" />
                                    <button type="button"
                                        onclick={() => apiKeyVisible = !apiKeyVisible}
                                        class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary transition-colors"
                                        title={apiKeyVisible ? '隐藏' : '显示'}>
                                        {#if apiKeyVisible}
                                            <EyeOff size={16} />
                                        {:else}
                                            <Eye size={16} />
                                        {/if}
                                    </button>
                                </div>
                            </div>
                            <button type="button" onclick={handleTestApi} disabled={testingApi}
                                class="flex items-center gap-1.5 px-3 py-2 bg-surface border border-border text-text-primary rounded-lg hover:bg-gray-50 transition-colors disabled:opacity-50 text-sm whitespace-nowrap">
                                {#if testingApi}
                                    <Loader2 size={14} class="animate-spin" />
                                    <span>测试中...</span>
                                {:else}
                                    <Wifi size={14} />
                                    <span>测试连接</span>
                                {/if}
                            </button>
                        </div>
                        {#if testResult}
                            <div class="col-span-2">
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
                        <div>
                            <label class="block text-sm font-medium mb-1">Temperature</label>
                            <input type="number" bind:value={form.temperature} min={0} max={2} step={0.1}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface" />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1">Max Tokens</label>
                            <input type="number" bind:value={form.max_tokens} min={1} max={32768}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface" />
                        </div>
                        <div class="col-span-2 flex items-center gap-2">
                            <input id="ad-thinking" type="checkbox" bind:checked={form.thinking_mode}
                                class="w-4 h-4 rounded border-border text-primary focus:ring-primary" />
                            <label for="ad-thinking" class="text-sm">启用思考模式（如模型支持）</label>
                        </div>
                    </div>
                    </div>
                </div>
            {:else if activeTab === 'relationships'}
                <AgentRelationshipPanel agentId={agent.id} />
            {:else if activeTab === 'memory'}
                <AgentMemoryPanel
                    agentId={agent.id}
                    bind:longTermMemory={form.long_term_memory}
                    bind:memoryEnabled={form.memory_enabled}
                />
            {:else if activeTab === 'timer'}
                <AgentTimerPanel agentId={agent.id} />
            {/if}
        </div>

        <!-- Footer actions -->
        <div class="px-6 py-4 border-t border-border bg-surface flex justify-between items-center">
            {#if activeTab === 'config'}
                <button
                    onclick={() => showGenerateModal = true}
                    class="flex items-center gap-2 px-4 py-2 text-primary hover:bg-primary/5 rounded-lg transition-colors"
                >
                    <Sparkles size={16} />
                    <span>人设自生成</span>
                </button>
            {:else}
                <div></div>
            {/if}
            <div class="flex gap-3">
                <button onclick={() => appState.selectAgent(null)} class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">
                    取消
                </button>
                {#if activeTab === 'config'}
                    <button onclick={handleSave} disabled={saving}
                        class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                        {#if saving}
                            <Loader2 size={16} class="animate-spin" />
                            <span>保存中...</span>
                        {:else}
                            <Save size={16} />
                            <span>保存</span>
                        {/if}
                    </button>
                {/if}
            </div>
        </div>
    {/if}
</div>

<AvatarUploadModal
    open={showAvatarModal}
    targetType="agent"
    targetId={agent?.id ?? ''}
    currentAvatar={agent?.avatar_path ?? null}
    onClose={() => showAvatarModal = false}
    onUploaded={(path) => {
        if (agent) {
            agent.avatar_path = path;
        }
        showAvatarModal = false;
    }}
/>

{#if agent}
    <PersonaGenerateModal
        open={showGenerateModal}
        agentId={agent.id}
        onClose={() => showGenerateModal = false}
        onGenerated={(result) => {
            form.detailed_persona = result.detailed_persona;
            form.simplified_persona = result.simplified_persona;
        }}
    />
{/if}

{#if agent}
    <ImportModelConfigModal
        open={showImportModal}
        currentAgentId={agent.id}
        onImport={handleImportModelConfig}
    />
{/if}
