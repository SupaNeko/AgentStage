<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, Trash2, Save, Loader2, MessageSquare, Sparkles } from 'lucide-svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import { resolveAvatarUrl } from '$lib/utils';
    import type { Agent } from '$lib/types';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import PersonaGenerateModal from './PersonaGenerateModal.svelte';
    import AgentRelationshipPanel from './AgentRelationshipPanel.svelte';
    import AgentMemoryPanel from './AgentMemoryPanel.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import AgentTimerPanel from './AgentTimerPanel.svelte';

    let agent = $state<Agent | null>(null);
    let loading = $state(false);
    let saving = $state(false);
    let error = $state('');
    let showAvatarModal = $state(false);
    let showGenerateModal = $state(false);
    let showDeleteConfirm = $state(false);
    let activeTab = $state<'config' | 'relationships' | 'memory' | 'timer'>('config');

    // Proactive session state
    let proactiveEnabled = $state(false);
    let proactiveMinMinutes = $state(10);
    let proactiveMaxMinutes = $state(30);

    // Form state
    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        model_config_id: null as string | null,
        temperature: null as number | null,
        long_term_memory: '',
        memory_enabled: true,
    });

    async function loadAgent(id: string) {
        logger.debug('[DEBUG AgentDetail.loadAgent]', { id });
        loading = true;
        error = '';
        try {
            const result = await invoke<Agent>('get_agent', { id });
            agent = result;
            logger.debug('[DEBUG AgentDetail.loadAgent] success', { id });
            if (result) {
                form = {
                    name: result.name,
                    detailed_persona: result.detailed_persona,
                    simplified_persona: result.simplified_persona,
                    model_config_id: result.model_config_id,
                    temperature: result.temperature,
                    long_term_memory: result.long_term_memory || '',
                    memory_enabled: result.memory_enabled ?? true,
                };
                proactiveEnabled = !!result.proactive_enabled;
                proactiveMinMinutes = result.proactive_min_minutes ?? 10;
                proactiveMaxMinutes = result.proactive_max_minutes ?? 30;
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
                model_config_id: form.model_config_id,
                temperature: form.temperature,
            };
            const updated = await invoke<Agent>('update_agent', { req: updateReq });
            agent = updated;
            await agentStore.loadAgents();
            await invoke('update_agent_proactive', {
                agentId: agent.id,
                proactiveEnabled: proactiveEnabled ? 1 : 0,
                proactiveMinMinutes: proactiveMinMinutes,
                proactiveMaxMinutes: proactiveMaxMinutes,
            });
            logger.debug('[DEBUG AgentDetail.handleSave] success', { id: agent.id });
            toastStore.success('已保存', 2000);
        } catch (err) {
            logger.debug('[DEBUG AgentDetail.handleSave] failed', { id: agent?.id, error: err });
            logger.error('Failed to update agent:', err);
            error = '保存失败: ' + String(err);
        } finally {
            saving = false;
        }
    }

    function handleDelete() {
        if (!agent) return;
        logger.debug('[DEBUG AgentDetail.handleDelete]', { id: agent.id });
        showDeleteConfirm = true;
    }

    async function doDelete() {
        if (!agent) return;
        showDeleteConfirm = false;
        try {
            await invoke('delete_agent', { req: { id: agent.id } });
            await agentStore.loadAgents();
            appState.selectAgent(null);
        } catch (err) {
            logger.error('Failed to delete agent:', err);
            error = '删除失败: ' + String(err);
        }
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

<div class="flex flex-col h-full bg-bg detail-panel">
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
                <button onclick={handleStartChat} class="flex items-center gap-1.5 px-3 py-1.5 bg-primary text-white text-sm hover:bg-primary-dark rounded-lg transition-colors btn-primary">
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
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface input-field" />
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
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface input-field"
                                    placeholder="你是 Fate/stay night 中的角色卫宫士郎，性格坚韧不拔，内心温柔但执拗，拥有强烈的正义感..."></textarea>
                                <p class="text-xs text-text-secondary mt-1">角色自己看到的完整设定，直接注入 System Prompt</p>
                            </div>
                            <div>
                                <label class="block text-sm font-medium mb-1">简易人设 <span class="text-red-500">*</span></label>
                                <textarea bind:value={form.simplified_persona} rows={2}
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface input-field"
                                    placeholder="出自 Fate/stay night 的角色卫宫士郎，冬木市的见习魔术师，性格正义感强烈。"></textarea>
                                <p class="text-xs text-text-secondary mt-1">给其它角色看的角色名片（角色简介）</p>
                            </div>
                        </div>
                    </div>

                    <!-- Model Config -->
                    <div>
                        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">模型配置</h3>
                        <div class="space-y-3">
                            <div>
                                <label class="block text-sm font-medium mb-1">选择模型 <span class="text-red-500">*</span></label>
                                <select
                                    bind:value={form.model_config_id}
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface input-field"
                                >
                                    <option value={null}>请选择模型配置</option>
                                    {#each modelConfigStore.configs as config}
                                        <option value={config.id}>{config.name} ({config.provider} / {config.model_name})</option>
                                    {/each}
                                </select>
                                {#if modelConfigStore.configs.length === 0 && !modelConfigStore.loading}
                                    <p class="text-xs text-text-secondary mt-1">
                                        暂无模型配置，请先在设置-模型中添加
                                    </p>
                                {/if}
                            </div>
                            <div>
                                <label class="block text-sm font-medium mb-1">Temperature</label>
                                <input
                                    type="number"
                                    value={form.temperature ?? ''}
                                    oninput={(e) => {
                                        const val = (e.target as HTMLInputElement).value;
                                        form.temperature = val === '' ? null : parseFloat(val);
                                    }}
                                    min={0}
                                    max={2}
                                    step={0.1}
                                    placeholder="使用模型默认值"
                                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface input-field"
                                />
                            </div>
                        </div>
                    </div>

                    <!-- Proactive Session -->
                    <div class="mt-6 border-t border-border pt-4">
                        <h3 class="font-semibold mb-3">主动会话机制</h3>
                        <label class="flex items-center gap-2 mb-3">
                            <input type="checkbox" bind:checked={proactiveEnabled} />
                            <span>启用主动会话</span>
                        </label>
                        {#if proactiveEnabled}
                            <div class="flex gap-2 items-center">
                                <span class="text-sm">触发时间区间（分钟）</span>
                                <input type="number" min={1} bind:value={proactiveMinMinutes} class="w-20 px-2 py-1 bg-bg border border-border rounded input-field" />
                                <span>~</span>
                                <input type="number" min={1} bind:value={proactiveMaxMinutes} class="w-20 px-2 py-1 bg-bg border border-border rounded input-field" />
                            </div>
                            <p class="text-xs text-text-secondary mt-1">角色每次发消息后，会在此区间内随机一个时间，若期间未再发言则触发一次。</p>
                        {/if}
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
                        class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary">
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
    <ConfirmDialog
        open={showDeleteConfirm}
        title="删除角色"
        content={`确定要删除角色 "${agent.name}" 吗？此操作不可恢复。`}
        confirmText="确认删除"
        confirmClass="bg-red-500 text-white hover:bg-red-600"
        onConfirm={doDelete}
        onCancel={() => showDeleteConfirm = false}
    />
{/if}
