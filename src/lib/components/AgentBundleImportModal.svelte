<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, User, X } from 'lucide-svelte';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import type {
        AgentBundleImportPreviewResponse,
        AgentBundleImportResultResponse,
        ImportAgentBundleRequest,
        ModelConfig,
    } from '$lib/types';
    import { resolveAvatarUrl } from '$lib/utils';

    interface Props {
        open: boolean;
        fileContent: string;
        preview: AgentBundleImportPreviewResponse | null;
        onClose?: () => void;
        onSuccess?: () => void | Promise<void>;
    }

    type EditableImportAgent = {
        bundleId: string;
        name: string;
        modelConfigId: string | null;
        avatarDataUrl: string | null;
    };

    type EditableImportUserPersona = {
        bundleId: string;
        name: string;
        avatarDataUrl: string | null;
    };

    let { open = $bindable(false), fileContent, preview, onClose, onSuccess }: Props = $props();

    let submitting = $state(false);
    let batchModelId = $state('');
    let editableAgents = $state<EditableImportAgent[]>([]);
    let editableUserPersonas = $state<EditableImportUserPersona[]>([]);

    function syncPreviewState(nextPreview: AgentBundleImportPreviewResponse | null) {
        editableAgents = nextPreview?.agents.map((agent) => ({
            bundleId: agent.bundleId,
            name: agent.suggestedName,
            modelConfigId: null,
            avatarDataUrl: agent.avatarDataUrl,
        })) ?? [];
        editableUserPersonas = nextPreview?.userPersonas.map((persona) => ({
            bundleId: persona.bundleId,
            name: persona.suggestedName,
            avatarDataUrl: persona.avatarDataUrl,
        })) ?? [];
        batchModelId = '';
    }

    $effect(() => {
        syncPreviewState(preview);
    });

    function closeModal() {
        if (submitting) return;
        open = false;
        onClose?.();
    }

    function updateAgentName(bundleId: string, value: string) {
        const target = editableAgents.find((agent) => agent.bundleId === bundleId);
        if (target) {
            target.name = value;
            editableAgents = [...editableAgents];
        }
    }

    function updateAgentModel(bundleId: string, value: string) {
        const target = editableAgents.find((agent) => agent.bundleId === bundleId);
        if (target) {
            target.modelConfigId = value || null;
            editableAgents = [...editableAgents];
        }
    }

    function updateUserPersonaName(bundleId: string, value: string) {
        const target = editableUserPersonas.find((persona) => persona.bundleId === bundleId);
        if (target) {
            target.name = value;
            editableUserPersonas = [...editableUserPersonas];
        }
    }

    function applyBatchModel() {
        editableAgents = editableAgents.map((agent) => ({
            ...agent,
            modelConfigId: batchModelId || null,
        }));
    }

    function getModelLabel(config: ModelConfig) {
        return `${config.name} (${config.provider} / ${config.model_name})`;
    }

    function formatWarnings(warnings: string[]) {
        if (warnings.length === 0) return '';
        return ` 提示：${warnings.join('；')}`;
    }

    async function handleImport() {
        if (!preview) return;
        if (submitting) return;

        const agents = editableAgents.map((agent) => ({
            bundleId: agent.bundleId,
            name: agent.name.trim(),
            modelConfigId: agent.modelConfigId,
        }));
        const userPersonas = editableUserPersonas.map((persona) => ({
            bundleId: persona.bundleId,
            name: persona.name.trim(),
        }));

        if (agents.some((agent) => !agent.name) || userPersonas.some((persona) => !persona.name)) {
            toastStore.error('导入名称不能为空，请填写所有角色和用户人设名称。');
            return;
        }

        submitting = true;
        try {
            const req: ImportAgentBundleRequest = {
                fileContent,
                agents,
                userPersonas,
            };

            const result = await invoke<AgentBundleImportResultResponse>('import_agent_bundle', { req });
            await agentStore.loadAgents();
            await userPersonaStore.loadPersonas();
            await onSuccess?.();

            toastStore.success(
                `已导入 ${result.importedAgentCount} 个角色、${result.importedUserPersonaCount} 个用户人设。${formatWarnings(result.warnings)}`.trim()
            );
            if (result.renamed) {
                toastStore.info('部分名称已自动调整，建议导入后检查并按需修改。');
            }
            open = false;
            onClose?.();
        } catch (err) {
            const previewWarnings = preview.warnings.length ? `。导入预检提示：${preview.warnings.join('；')}` : '';
            toastStore.error(`导入失败：${String(err)}${previewWarnings}`);
        } finally {
            submitting = false;
        }
    }
</script>

{#if open && preview}
    <div
        class="fixed inset-0 z-[90] flex items-center justify-center bg-black/50"
        onclick={(e) => { if (e.target === e.currentTarget) closeModal(); }}
        onkeydown={(e) => { if (e.key === 'Escape') closeModal(); }}
        role="dialog"
        aria-modal="true"
        tabindex="-1"
    >
        <div
            class="flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl"
        >
            <div class="flex items-center justify-between border-b border-border px-4 py-3">
                <div>
                    <h3 class="text-base font-semibold text-text">导入角色配置</h3>
                    <p class="mt-1 text-xs text-text-secondary">
                        AI 角色：{preview.agentCount} 个
                        <span class="mx-2 text-border">|</span>
                        用户人设：{preview.userPersonaCount} 个
                    </p>
                </div>
                <button
                    type="button"
                    class="rounded-md p-1.5 text-text-secondary transition-colors hover:bg-bg hover:text-text"
                    onclick={closeModal}
                    aria-label="关闭"
                >
                    <X size={18} />
                </button>
            </div>

            <div class="flex-1 space-y-4 overflow-y-auto px-4 py-4">
                {#if preview.warnings.length > 0}
                    <div class="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                        {preview.warnings.join('；')}
                    </div>
                {/if}

                <section class="space-y-2">
                    <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                        <div>
                            <h4 class="text-sm font-medium text-text">AI 角色</h4>
                            <p class="text-xs text-text-secondary">可调整名称，并为每个角色指定导入后的模型。</p>
                        </div>
                        <div class="flex items-center gap-2">
                            <select
                                bind:value={batchModelId}
                                class="rounded-lg border border-border bg-bg px-2 py-1.5 text-xs outline-none transition focus:ring-2 focus:ring-primary/20"
                            >
                                <option value="">批量设置模型</option>
                                {#each modelConfigStore.configs as config}
                                    <option value={config.id}>{config.name}</option>
                                {/each}
                            </select>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-2 py-1.5 text-xs text-text transition-colors hover:bg-bg"
                                onclick={applyBatchModel}
                            >
                                应用
                            </button>
                        </div>
                    </div>

                    {#if modelConfigStore.configs.length === 0}
                        <div class="rounded-md border border-border bg-bg px-3 py-2 text-xs text-text-secondary">
                            暂无可用模型，导入后可在角色配置中选择模型
                        </div>
                    {/if}

                    <div class="space-y-1.5">
                        {#each editableAgents as agent (agent.bundleId)}
                            <div class="grid gap-2 rounded-lg border border-border bg-bg px-3 py-2.5 md:grid-cols-[minmax(0,1fr)_200px]">
                                <div class="flex min-w-0 gap-2.5">
                                    <div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-primary">
                                        {#if agent.avatarDataUrl}
                                            <img src={resolveAvatarUrl(agent.avatarDataUrl)} alt={agent.name} class="h-full w-full object-cover" />
                                        {:else}
                                            <Bot size={16} />
                                        {/if}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <p class="truncate text-[11px] leading-4 text-text-secondary">原名称：{preview.agents.find((item) => item.bundleId === agent.bundleId)?.originalName}</p>
                                        <input
                                            type="text"
                                            value={agent.name}
                                            maxlength={40}
                                            class="w-full rounded-md border border-border bg-surface px-2.5 py-1.5 text-sm outline-none transition focus:ring-2 focus:ring-primary/20"
                                            oninput={(e) => updateAgentName(agent.bundleId, (e.currentTarget as HTMLInputElement).value)}
                                        />
                                    </div>
                                </div>
                                <select
                                    value={agent.modelConfigId ?? ''}
                                    class="w-full rounded-md border border-border bg-surface px-2.5 py-1.5 text-sm outline-none transition focus:ring-2 focus:ring-primary/20"
                                    oninput={(e) => updateAgentModel(agent.bundleId, (e.currentTarget as HTMLSelectElement).value)}
                                >
                                    <option value="">导入后再选择模型</option>
                                    {#each modelConfigStore.configs as config}
                                        <option value={config.id}>{getModelLabel(config)}</option>
                                    {/each}
                                </select>
                            </div>
                        {/each}
                    </div>
                </section>

                <section class="space-y-2">
                    <div>
                        <h4 class="text-sm font-medium text-text">用户人设</h4>
                        <p class="text-xs text-text-secondary">仅展示头像与名称，导入前可按需调整名称。</p>
                    </div>

                    <div class="space-y-1.5">
                        {#each editableUserPersonas as persona (persona.bundleId)}
                            <div class="flex min-w-0 gap-2.5 rounded-lg border border-border bg-bg px-3 py-2.5">
                                <div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-emerald-500/10 text-emerald-600">
                                    {#if persona.avatarDataUrl}
                                        <img src={resolveAvatarUrl(persona.avatarDataUrl)} alt={persona.name} class="h-full w-full object-cover" />
                                    {:else}
                                        <User size={16} />
                                    {/if}
                                </div>
                                <div class="min-w-0 flex-1">
                                    <p class="truncate text-[11px] leading-4 text-text-secondary">原名称：{preview.userPersonas.find((item) => item.bundleId === persona.bundleId)?.originalName}</p>
                                    <input
                                        type="text"
                                        value={persona.name}
                                        maxlength={40}
                                        class="w-full rounded-md border border-border bg-surface px-2.5 py-1.5 text-sm outline-none transition focus:ring-2 focus:ring-primary/20"
                                        oninput={(e) => updateUserPersonaName(persona.bundleId, (e.currentTarget as HTMLInputElement).value)}
                                    />
                                </div>
                            </div>
                        {/each}
                    </div>
                </section>
            </div>

            <div class="flex items-center justify-end gap-2 border-t border-border px-4 py-3">
                <button
                    type="button"
                    class="rounded-lg px-3 py-2 text-sm text-text-secondary transition-colors hover:bg-bg"
                    onclick={closeModal}
                >
                    取消
                </button>
                <button
                    type="button"
                    class="rounded-lg bg-primary px-4 py-2 text-sm text-white transition-colors hover:bg-primary-dark disabled:cursor-not-allowed disabled:opacity-60"
                    onclick={handleImport}
                    disabled={submitting}
                >
                    {submitting ? '导入中...' : '确认导入'}
                </button>
            </div>
        </div>
    </div>
{/if}
