<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Bot, Sparkles, Loader2 } from 'lucide-svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import type { GeneratePersonaResult } from '$lib/types';

    let { open = $bindable(false), onSuccess }: { open: boolean; onSuccess?: () => void } = $props();

    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        personality: '',
        scenario: '',
        example_messages: '',
        creator_notes: '',
        model_config_id: null as string | null,
        temperature: null as number | null,
    });
    let avatarPath = $state<string | null>(null);
    let showGenerateFields = $state(false);
    let referenceCharacter = $state('');
    let additionalInfo = $state('');
    let generating = $state(false);
    let submitting = $state(false);
    let error = $state('');

    async function handleGeneratePersona() {
        const hasRef = referenceCharacter.trim().length > 0;
        const hasSupp = additionalInfo.trim().length > 0;
        if (!hasRef && !hasSupp) {
            toastStore.show('参考角色和补充信息至少填写一项', 'error', 3000);
            return;
        }
        if (!form.model_config_id) {
            toastStore.show('请先在下方选择模型配置', 'error', 3000);
            return;
        }
        const modelConfig = modelConfigStore.getById(form.model_config_id);
        if (!modelConfig) {
            toastStore.show('所选模型配置不存在', 'error', 3000);
            return;
        }

        generating = true;
        try {
            const result = await invoke<GeneratePersonaResult>('generate_persona', {
                req: {
                    agent_id: null,
                    model_config_id: form.model_config_id,
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
        if (!form.model_config_id) {
            error = '请选择模型配置';
            return;
        }
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
                model_config_id: form.model_config_id,
                temperature: form.temperature,
            };
            await invoke('create_agent', { req });
            open = false;
            onSuccess?.();
            form = { name: '', detailed_persona: '', simplified_persona: '', personality: '', scenario: '', example_messages: '', creator_notes: '', model_config_id: null, temperature: null };
        } catch (err: any) {
            error = err.toString();
        } finally {
            submitting = false;
        }
    }
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 modal-overlay" onclick={() => open = false} role="dialog" aria-modal="true">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto modal-card" onclick={(e) => e.stopPropagation()}>
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
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field" />
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
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                                placeholder="例如：远坂凛" />
                        </div>
                        <div>
                            <label class="block text-sm font-medium mb-1" for="ca-additional">补充信息</label>
                            <textarea id="ca-additional" bind:value={additionalInfo} rows={3}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none input-field"
                                placeholder="输入额外的人设补充信息..."></textarea>
                        </div>
                        <button
                            type="button"
                            onclick={handleGeneratePersona}
                            disabled={generating || (!referenceCharacter.trim() && !additionalInfo.trim())}
                            class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary"
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
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none input-field"
                    placeholder="你是 Fate/stay night 中的角色卫宫士郎，性格坚韧不拔，内心温柔但执拗，拥有强烈的正义感，口头禅是'人被杀就会死'。你是冬木市穗群原学园的学生，同时也是拥有投影魔术的见习魔术师..."></textarea>
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-simplified">简易人设 <span class="text-red-500">*</span></label>
                <textarea id="ca-simplified" bind:value={form.simplified_persona} required rows={2}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none input-field"
                    placeholder="出自 Fate/stay night 的角色卫宫士郎，冬木市的见习魔术师，性格正义感强烈。"></textarea>
                <p class="text-xs text-text-secondary mt-1">这是给其它角色看的角色名片（角色简介）</p>
            </div>

            <div>
                <h3 class="text-sm font-medium text-text-secondary uppercase tracking-wide mb-2">模型配置</h3>
                <div class="space-y-3">
                    <div>
                        <label class="block text-sm font-medium mb-1" for="ca-model-config">选择模型 <span class="text-red-500">*</span></label>
                        <select
                            id="ca-model-config"
                            bind:value={form.model_config_id}
                            required
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
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
                        <label class="block text-sm font-medium mb-1" for="ca-temp">Temperature</label>
                        <input
                            id="ca-temp"
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
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                        />
                    </div>
                </div>
            </div>

            {#if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg">{error}</div>
            {/if}
            <div class="flex justify-end gap-3 pt-2">
                <button type="button" onclick={() => open = false}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">取消</button>
                <button type="submit" disabled={submitting}
                    class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary">
                    {submitting ? '创建中...' : '创建'}
                </button>
            </div>
        </form>
    </div>
</div>
{/if}

<AvatarUploadModal
    open={false}
    targetType="agent"
    targetId=""
    currentAvatar={null}
    onClose={() => {}}
    onUploaded={(path) => { avatarPath = path; }}
/>
