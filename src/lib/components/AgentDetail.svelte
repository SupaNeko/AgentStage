<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, Trash2, Save, Loader2, MessageSquare } from 'lucide-svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import type { Agent } from '$lib/types';

    let agent = $state<Agent | null>(null);
    let loading = $state(false);
    let saving = $state(false);
    let error = $state('');

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
                    model_provider: result.model_provider || 'openai',
                    model_name: result.model_name || '',
                    base_url: result.base_url || '',
                    api_key: '', // Don't populate encrypted key
                    temperature: result.temperature,
                    max_tokens: result.max_tokens,
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
            };
            if (form.api_key.trim()) {
                updateReq.api_key = form.api_key;
            }
            const updated = await invoke<Agent>('update_agent', { req: updateReq });
            agent = updated;
            form.api_key = ''; // Clear API key field after save
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
                <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                    {#if agent.avatar_path}
                        <img src={agent.avatar_path} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                    {:else}
                        <Bot size={20} />
                    {/if}
                </div>
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

        <!-- Form -->
        <div class="flex-1 overflow-y-auto px-6 py-4">
            {#if error}
                <div class="mb-4 p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {/if}

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
                    <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">模型配置</h3>
                    <div class="grid grid-cols-2 gap-4">
                        <div>
                            <label class="block text-sm font-medium mb-1">模型提供商 <span class="text-red-500">*</span></label>
                            <select bind:value={form.model_provider}
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
                        <div>
                            <label class="block text-sm font-medium mb-1">API Key</label>
                            <input type="password" bind:value={form.api_key}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                                placeholder="留空表示不修改" />
                        </div>
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
                    </div>
                </div>
            </div>
        </div>

        <!-- Footer actions -->
        <div class="px-6 py-4 border-t border-border bg-surface flex justify-end gap-3">
            <button onclick={() => appState.selectAgent(null)} class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">
                取消
            </button>
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
        </div>
    {/if}
</div>
