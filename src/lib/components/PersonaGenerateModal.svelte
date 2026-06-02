<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Loader2, Sparkles } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import type { GeneratePersonaResult } from '$lib/types';

    interface Props {
        open: boolean;
        agentId: string;
        onClose: () => void;
        onGenerated: (result: GeneratePersonaResult) => void;
    }

    let { open, agentId, onClose, onGenerated }: Props = $props();
    let showCloseConfirm = $state(false);

    let referenceCharacter = $state('');
    let supplement = $state('');
    let generating = $state(false);
    let mouseDownOnOverlay = $state(false);

    function handleClose() {
        if (generating) {
            showCloseConfirm = true;
            return;
        }
        onClose();
    }

    function doClose() {
        showCloseConfirm = false;
        onClose();
    }

    async function handleGenerate() {
        const hasRef = referenceCharacter.trim().length > 0;
        const hasSupp = supplement.trim().length > 0;
        if (!hasRef && !hasSupp) {
            toastStore.error('参考角色和补充信息至少填写一项');
            return;
        }

        generating = true;
        try {
            const result = await invoke<GeneratePersonaResult>('generate_persona', {
                req: {
                    agent_id: agentId ?? null,
                    reference_character: referenceCharacter.trim() || null,
                    supplement: supplement.trim() || null,
                },
            });
            logger.debug('[DEBUG PersonaGenerateModal] generated', { agentId });
            onGenerated(result);
            onClose();
        } catch (err: any) {
            logger.error('Failed to generate persona:', err);
            toastStore.error('生成失败: ' + String(err));
        } finally {
            generating = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center modal-overlay"
        onmousedown={(e) => { mouseDownOnOverlay = e.target === e.currentTarget; }}
        onclick={(e) => { if (mouseDownOnOverlay && e.target === e.currentTarget) handleClose(); mouseDownOnOverlay = false; }}
        role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-[28rem] shadow-xl modal-card" onmousedown={() => mouseDownOnOverlay = false} onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-2">
                    <Sparkles size={18} class="text-primary" />
                    <h3 class="font-semibold">人设自生成</h3>
                </div>
                <button onclick={handleClose} class="p-1 hover:bg-bg rounded" aria-label="关闭">
                    <X size={18} />
                </button>
            </div>

            <div class="space-y-4">
                <div>
                    <label class="block text-sm font-medium mb-1">参考角色 <span class="text-text-secondary">（可选）</span></label>
                    <input
                        type="text"
                        bind:value={referenceCharacter}
                        disabled={generating}
                        placeholder="如：Fate/stay night 中的 Saber"
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                    />
                </div>

                <div>
                    <label class="block text-sm font-medium mb-1">补充信息 <span class="text-text-secondary">（可选）</span></label>
                    <textarea
                        bind:value={supplement}
                        disabled={generating}
                        rows={4}
                        placeholder="可填写任意相关内容：设定、要求、台词、聊天记录等..."
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface disabled:opacity-50 input-field"
                    ></textarea>
                </div>

                <p class="text-xs text-text-secondary">
                    参考角色和补充信息至少填写一项
                </p>
            </div>

            <div class="flex justify-end gap-3 mt-6">
                <button
                    onclick={handleClose}
                    disabled={generating}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors disabled:opacity-50"
                >
                    取消
                </button>
                <button
                    onclick={handleGenerate}
                    disabled={generating || (!referenceCharacter.trim() && !supplement.trim())}
                    class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary"
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
        </div>
    </div>
{/if}

<ConfirmDialog
    open={showCloseConfirm}
    title="退出确认"
    content="退出将会打断生成，确定要退出吗？"
    confirmText="确认退出"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={doClose}
    onCancel={() => showCloseConfirm = false}
/>
