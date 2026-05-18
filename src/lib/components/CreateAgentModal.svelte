<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Bot, Sparkles, Loader2, Wifi, Eye, EyeOff, Import } from 'lucide-svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import ImportModelConfigModal from './ImportModelConfigModal.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import { PROVIDER_DEFAULTS } from '$lib/modelConfig';
    import type { Agent, GeneratePersonaResult } from '$lib/types';

    let { open = $bindable(false), onSuccess }: { open: boolean; onSuccess?: () => void } = $props();

    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        personality: '',
        scenario: '',
        example_messages: '',
        creator_notes: '',
        model_provider: 'openai',
        model_name: PROVIDER_DEFAULTS.openai.modelName,
        api_key: '',
        base_url: PROVIDER_DEFAULTS.openai.baseUrl,
        temperature: 0.7,
        max_tokens: 2048,
        thinking_mode: false,
    });
    let avatarPath = $state<string | null>(null);
    let showGenerateFields = $state(false);
    let referenceCharacter = $state('');
    let additionalInfo = $state('');
    let generating = $state(false);
    let submitting = $state(false);
    let error = $state('');
    let testingApi = $state(false);
    let testResult = $state<{ success: boolean; latencyMs: number; message: string } | null>(null);
    let apiKeyVisible = $state(false);
    let showImportModal = $state(false);

    function handleImportModelConfig(sourceAgent: Agent) {
        form.model_provider = sourceAgent.model_provider || 'openai';
        form.model_name = sourceAgent.model_name || '';
        form.base_url = sourceAgent.base_url || '';
        form.temperature = sourceAgent.temperature;
        form.max_tokens = sourceAgent.max_tokens;
        form.thinking_mode = sourceAgent.thinking_mode ?? false;
        form.api_key = sourceAgent.api_key || '';
        testResult = null;
        toastStore.show(`已导入 "${sourceAgent.name}" 的模型配置`, 'success', 2000);
    }

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

    async function handleGeneratePersona() {
        const hasRef = referenceCharacter.trim().length > 0;
        const hasSupp = additionalInfo.trim().length > 0;
        if (!hasRef && !hasSupp) {
            toastStore.show('参考角色和补充信息至少填写一项', 'error', 3000);
            return;
        }
        if (!form.model_name || !form.api_key) {
            toastStore.show('请先在下方填写模型名称和 API Key', 'error', 3000);
            return;
        }

        generating = true;
        try {
            const result = await invoke<GeneratePersonaResult>('generate_persona', {
                req: {
                    agent_id: null,
                    model_config: {
                        model_provider: form.model_provider,
                        model_name: form.model_name,
                        base_url: form.base_url || null,
                        api_key: form.api_key,
                        temperature: form.temperature,
                        max_tokens: form.max_tokens,
                        thinking_mode: form.thinking_mode,
                    },
                    reference_character: referenceCharacter.trim() || null,
                    supplement: additionalInfo.trim() || null,
                },
            });
            logger.debug('[DEBUG CreateAgentModal] persona generated');
            form.detailed_persona = result.detailed_persona;
            form.simplified_persona = result.simplified_persona;
            form.personality = result.personality || '';
            form.scenario = result.scenario || '';
            form.example_messages = result.example_messages || '';
            form.creator_notes = result.creator_notes || '';
            toastStore.show('人设生成完成', 'success', 2000);
        } catch (err: any) {
            logger.error('Failed to generate persona:', err);
            toastStore.show('生成失败: ' + String(err), 'error', 5000);
        } finally {
            generating = false;
        }
    }

    async function handleSubmit(e: Event) {
        e.preventDefault();
        submitting = true;
        error = '';

        try {
            const req = {
                name: form.name,
                detailed_persona: form.detailed_persona,
                simplified_persona: form.simplified_persona,
                personality: form.personality || null,
                scenario: form.scenario || null,
                example_messages: form.example_messages || null,
                creator_notes: form.creator_notes || null,
                model_provider: form.model_provider,
                model_name: form.model_name,
                api_key: form.api_key,
                base_url: form.base_url || null,
                temperature: form.temperature,
                max_tokens: form.max_tokens,
                thinking_mode: form.thinking_mode,
            };
            await invoke('create_agent', { req });
            open = false;
            onSuccess?.();
            form = { name: '', detailed_persona: '', simplified_persona: '', personality: '', scenario: '', example_messages: '', creator_notes: '', model_provider: 'openai', model_name: PROVIDER_DEFAULTS.openai.modelName, api_key: '', base_url: PROVIDER_DEFAULTS.openai.baseUrl, temperature: 0.7, max_tokens: 2048, thinking_mode: false };
        } catch (err: any) {
            error = err.toString();
        } finally {
            submitting = false;
        }
    }
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={() => open = false} role="dialog" aria-modal="true">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">新建角色</h3>
            <button onclick={() => open = false} class="p-1 hover:bg-gray-100 rounded" aria-label="关闭">
                <X size={20} />
            </button>
        </div>

        <form onsubmit={handleSubmit} class="p-4 space-y-4">
            <div class="flex justify-center">
                <div
                    class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center text-primary"
                >
                    {#if avatarPath}
                        <img src={avatarPath} alt="头像" class="w-full h-full rounded-full object-cover" />
                    {:else}
                        <Bot size={28} />
                    {/if}
                </div>
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-name">角色名称 <span class="text-red-500">*</span></label>
                <input id="ca-name" type="text" bind:value={form.name} required maxlength={20}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>

            <div class="border-t border-border pt-3">
                <button
                    type="button"
                    onclick={() => showGenerateFields = !showGenerateFields}
                    class="flex items-center gap-2 text-sm text-primary hover:text-primary-dark transition-colors"
                >
                    <span>{showGenerateFields ? '▾' : '▸'}</span>
                    <span>人设自生成</span>
                </button>
                {#if showGenerateFields}
                    <div class="mt-3 space-y-3">
                        <div>
                            <label class="block text-sm font-medium mb-1" for="ca-ref">参考角色</label>
                            <input id="ca-ref" type="text" bind:value={referenceCharacter}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                                placeholder="例如：远坂凛" />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1" for="ca-additional">补充信息</label>
                            <textarea id="ca-additional" bind:value={additionalInfo} rows={3}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                                placeholder="输入额外的人设补充信息..."></textarea>
                        </div>
                        <button
                            type="button"
                            onclick={handleGeneratePersona}
                            disabled={generating || (!referenceCharacter.trim() && !additionalInfo.trim())}
                            class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-dark transition-colors disabled:opacity-50"
                        >
                            {#if generating}
                                <Loader2 size={16} class="animate-spin" />
                                <span>生成中...</span>
                            {:else}
                                <Sparkles size={16} />
                                <span>生成</span>
                            {/if}
                        </button>
                    </div>
                {/if}
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-detailed">详细人设 <span class="text-red-500">*</span></label>
                <textarea id="ca-detailed" bind:value={form.detailed_persona} required rows={4}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="你是 Fate/stay night 中的角色卫宫士郎，性格坚韧不拔，内心温柔但执拗，拥有强烈的正义感，口头禅是'人被杀就会死'。你是冬木市穗群原学园的学生，同时也是拥有投影魔术的见习魔术师..."></textarea>
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-simplified">简易人设 <span class="text-red-500">*</span></label>
                <textarea id="ca-simplified" bind:value={form.simplified_persona} required rows={2}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="出自 Fate/stay night 的角色卫宫士郎，冬木市的见习魔术师，性格正义感强烈。"></textarea>
                <p class="text-xs text-text-secondary mt-1">这是给其它角色看的角色名片（角色简介）</p>
            </div>

            <div>
                <div class="flex items-center justify-between mb-2">
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
                        <label class="block text-sm font-medium mb-1" for="ca-provider">模型提供商 <span class="text-red-500">*</span></label>
                        <select id="ca-provider" bind:value={form.model_provider} onchange={handleProviderChange}
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20">
                            <option value="openai">OpenAI</option>
                            <option value="anthropic">Anthropic</option>
                            <option value="google">Google</option>
                            <option value="kimi">Kimi (Moonshot)</option>
                            <option value="minimax">MiniMax</option>
                            <option value="custom">自定义</option>
                        </select>
                    </div>
                    <div>
                        <label class="block text-sm font-medium mb-1" for="ca-model">模型名称 <span class="text-red-500">*</span></label>
                        <input id="ca-model" type="text" bind:value={form.model_name} required
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                            placeholder="gpt-4o, claude-3-sonnet, kimi-k2..." />
                    </div>
                    <div class="col-span-2">
                        <label class="block text-sm font-medium mb-1" for="ca-baseurl">Base URL</label>
                        <input id="ca-baseurl" type="text" bind:value={form.base_url}
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                            placeholder="可选，默认使用官方地址" />
                    </div>
                    <div class="col-span-2 flex gap-3 items-end">
                        <div class="flex-1">
                            <label class="block text-sm font-medium mb-1" for="ca-apikey">API Key <span class="text-red-500">*</span></label>
                            <div class="relative">
                                <input id="ca-apikey" type={apiKeyVisible ? 'text' : 'password'} bind:value={form.api_key} required
                                    class="w-full px-3 py-2 pr-10 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
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
                    <label class="block text-sm font-medium mb-1" for="ca-temp">Temperature</label>
                    <input id="ca-temp" type="number" bind:value={form.temperature} min={0} max={2} step={0.1}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1" for="ca-maxtok">Max Tokens</label>
                    <input id="ca-maxtok" type="number" bind:value={form.max_tokens} min={1}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
                <div class="col-span-2 flex items-center gap-2">
                    <input id="ca-thinking" type="checkbox" bind:checked={form.thinking_mode}
                        class="w-4 h-4 rounded border-border text-primary focus:ring-primary/20" />
                    <label for="ca-thinking" class="text-sm">启用思考模式（如支持）</label>
                </div>
            </div>

            {#if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg">{error}</div>
            {/if}
            <div class="flex justify-end gap-3 pt-2">
                <button type="button" onclick={() => open = false}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">取消</button>
                <button type="submit" disabled={submitting}
                    class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                    {submitting ? '创建中...' : '创建'}
                </button>
            </div>
        </form>
    </div>
</div>
{/if}

<ImportModelConfigModal
    open={showImportModal}
    currentAgentId=""
    onImport={handleImportModelConfig}
/>
