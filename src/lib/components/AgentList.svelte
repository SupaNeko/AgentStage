<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, CheckSquare, Download, FileUp, Plus, Search, Square, User, X } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import type { UserPersona } from '$lib/stores/userPersonaStore.svelte';
    import type {
        Agent,
        AgentBundleExportPreviewResponse,
        AgentBundleExportResultResponse,
        AgentBundleImportPreviewResponse,
        ExportAgentBundleRequest,
        PreviewAgentBundleExportRequest,
        PreviewAgentBundleImportRequest,
    } from '$lib/types';
    import { resolveAvatarUrl } from '$lib/utils';
    import AgentBundleImportModal from './AgentBundleImportModal.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import CreateAgentModal from './CreateAgentModal.svelte';

    let loading = $state(true);
    let modalOpen = $state(false);
    let searchQuery = $state('');
    let showExportModal = $state(false);
    let exportSearchQuery = $state('');
    let selectedAgentIds = $state<string[]>([]);
    let selectedUserPersonaIds = $state<string[]>([]);
    let showExportConfirm = $state(false);
    let exportPreviewing = $state(false);
    let exporting = $state(false);
    let importPreview = $state<AgentBundleImportPreviewResponse | null>(null);
    let importModalOpen = $state(false);
    let importFileContent = $state('');
    let importingPreview = $state(false);
    let fileInput: HTMLInputElement | null = null;
    let exportConfirmContent = $state('');

    async function loadData() {
        loading = true;
        try {
            await Promise.all([
                agentStore.loadAgents(),
                userPersonaStore.loadPersonas(),
                modelConfigStore.load(),
            ]);
        } catch (err) {
            console.error('Failed to load list data:', err);
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        loadData();
    });

    const filteredAgents = $derived.by(() => {
        const query = searchQuery.trim().toLowerCase();
        if (!query) return agentStore.agents;
        return agentStore.agents.filter((agent) => agent.name.toLowerCase().includes(query));
    });

    const filteredExportAgents = $derived.by(() => {
        const query = exportSearchQuery.trim().toLowerCase();
        if (!query) return agentStore.agents;
        return agentStore.agents.filter((agent) => agent.name.toLowerCase().includes(query));
    });

    const filteredExportPersonas = $derived.by(() => {
        const query = exportSearchQuery.trim().toLowerCase();
        if (!query) return userPersonaStore.personas;
        return userPersonaStore.personas.filter((persona) => persona.name.toLowerCase().includes(query));
    });

    const exportSelectionCount = $derived(selectedAgentIds.length + selectedUserPersonaIds.length);
    const exportBusy = $derived(exportPreviewing || exporting);

    function resetExportModal() {
        showExportModal = false;
        selectedAgentIds = [];
        selectedUserPersonaIds = [];
        exportSearchQuery = '';
    }

    function openImportPicker() {
        fileInput?.click();
    }

    function toggleAgentSelection(agentId: string) {
        selectedAgentIds = selectedAgentIds.includes(agentId)
            ? selectedAgentIds.filter((id) => id !== agentId)
            : [...selectedAgentIds, agentId];
    }

    function toggleUserPersonaSelection(personaId: string) {
        selectedUserPersonaIds = selectedUserPersonaIds.includes(personaId)
            ? selectedUserPersonaIds.filter((id) => id !== personaId)
            : [...selectedUserPersonaIds, personaId];
    }

    function buildExportConfirmContent(preview: AgentBundleExportPreviewResponse) {
        const parts: string[] = [];
        if (preview.omittedRelationshipCount > 0) {
            parts.push(`角色关系描述 ${preview.omittedRelationshipCount} 条`);
        }
        if (preview.omittedRelationshipMemoryCount > 0) {
            parts.push(`关系记忆 ${preview.omittedRelationshipMemoryCount} 条`);
        }
        if (preview.omittedFriendshipCount > 0) {
            parts.push(`好友关系 ${preview.omittedFriendshipCount} 条`);
        }

        const summary = parts.length > 0 ? `未包含对象关联的 ${parts.join('、')} 不会导出。` : '';
        const warningText = preview.warnings.length > 0 ? ` 提示：${preview.warnings.join('；')}` : '';
        return `${summary}继续导出吗？${warningText}`.trim();
    }

    async function handleImportFileChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;

        importingPreview = true;
        try {
            importFileContent = await file.text();
            const req: PreviewAgentBundleImportRequest = { fileContent: importFileContent };
            importPreview = await invoke<AgentBundleImportPreviewResponse>('preview_agent_bundle_import', { req });
            importModalOpen = true;
            if (importPreview.warnings.length > 0) {
                toastStore.info(`导入预检提示：${importPreview.warnings.join('；')}`);
            }
        } catch (err) {
            importPreview = null;
            importFileContent = '';
            toastStore.error(`读取或预检导入包失败：${String(err)}`);
        } finally {
            input.value = '';
            importingPreview = false;
        }
    }

    async function runExport(confirmOmissions: boolean, options: { allowDuringPreview?: boolean } = {}) {
        if (exporting || (exportPreviewing && !options.allowDuringPreview)) return;

        const req: ExportAgentBundleRequest = {
            agentIds: selectedAgentIds,
            userPersonaIds: selectedUserPersonaIds,
            confirmOmissions,
        };

        exporting = true;
        try {
            const result = await invoke<AgentBundleExportResultResponse>('export_agent_bundle', { req });
            const warningText = result.warnings.length > 0 ? ` 提示：${result.warnings.join('；')}` : '';
            toastStore.success(`已导出到 ${result.exportedPath ?? '所选位置'}。${warningText}`.trim());
            resetExportModal();
        } catch (err) {
            toastStore.error(`导出失败：${String(err)}`);
        } finally {
            exporting = false;
            showExportConfirm = false;
        }
    }

    async function handleExport() {
        if (exportBusy) return;

        if (exportSelectionCount === 0) {
            toastStore.error('请至少选择 1 项再导出。');
            return;
        }

        const req: PreviewAgentBundleExportRequest = {
            agentIds: selectedAgentIds,
            userPersonaIds: selectedUserPersonaIds,
        };

        exportPreviewing = true;
        try {
            const preview = await invoke<AgentBundleExportPreviewResponse>('preview_agent_bundle_export', { req });
            const needsConfirm = preview.requiresConfirmation
                || preview.omittedRelationshipCount > 0
                || preview.omittedRelationshipMemoryCount > 0
                || preview.omittedFriendshipCount > 0;

            if (needsConfirm) {
                exportConfirmContent = buildExportConfirmContent(preview);
                showExportConfirm = true;
                return;
            }

            await runExport(false, { allowDuringPreview: true });
        } catch (err) {
            toastStore.error(`导出预检失败：${String(err)}`);
        } finally {
            exportPreviewing = false;
        }
    }

    function handleImportModalClose() {
        importModalOpen = false;
        importPreview = null;
        importFileContent = '';
    }

    function isAgentSelected(agentId: string) {
        return selectedAgentIds.includes(agentId);
    }

    function isUserPersonaSelected(personaId: string) {
        return selectedUserPersonaIds.includes(personaId);
    }
</script>

<div class="mid-panel flex h-full w-full flex-col border-r border-border bg-surface">
    <input
        bind:this={fileInput}
        type="file"
        accept=".agentstage"
        class="hidden"
        onchange={handleImportFileChange}
    />

    <header class="border-b border-border p-4">
        <h2 class="text-base font-semibold text-text mb-2">角色列表</h2>
        <div class="flex items-center gap-2">
            <button
                type="button"
                class="rounded-md border border-border px-2 py-1 text-xs text-text transition-colors hover:bg-bg disabled:cursor-not-allowed disabled:opacity-60"
                onclick={openImportPicker}
                disabled={importingPreview}
            >
                <span class="inline-flex items-center gap-1">
                    <FileUp size={13} />
                    <span>{importingPreview ? '读取中...' : '导入'}</span>
                </span>
            </button>
            <button
                type="button"
                class="rounded-md border border-border px-2 py-1 text-xs text-text transition-colors hover:bg-bg"
                onclick={() => { showExportModal = true; appState.selectAgent(null); }}
            >
                <span class="inline-flex items-center gap-1">
                    <Download size={13} />
                    <span>导出</span>
                </span>
            </button>
            <button
                type="button"
                class="rounded-md bg-primary px-2 py-1 text-xs text-white transition-colors hover:bg-primary-dark"
                onclick={() => { modalOpen = true; }}
            >
                <span class="inline-flex items-center gap-1">
                    <Plus size={13} />
                    <span>新建</span>
                </span>
            </button>
        </div>
    </header>

    <div class="border-b border-border px-4 py-3">
        <div class="relative">
            <Search size={16} class="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary" />
            <input
                type="text"
                placeholder="搜索角色..."
                bind:value={searchQuery}
                class="input-field w-full rounded-lg border border-border bg-bg py-2 pl-9 pr-3 text-sm focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
        </div>
    </div>

    <div class="flex-1 overflow-y-auto">
        {#if loading}
            <div class="flex h-full items-center justify-center text-sm text-text-secondary">加载中...</div>
        {:else if filteredAgents.length === 0}
            <div class="flex h-full flex-col items-center justify-center p-4 text-text-secondary">
                <Bot size={40} class="mb-3 opacity-50" />
                <p class="text-sm">{searchQuery ? '未找到匹配的角色' : '还没有创建任何角色'}</p>
                {#if !searchQuery}
                    <p class="mt-1 text-xs">点击"新建"开始创建</p>
                {/if}
            </div>
        {:else}
            <div class="divide-y divide-border">
                {#each filteredAgents as agent}
                    <button
                        type="button"
                        class="bboard-item flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {appState.selectedAgentId === agent.id ? 'border-l-2 border-l-primary bg-primary/5' : ''}"
                        onclick={() => appState.selectAgent(agent.id)}
                    >
                        <div class="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-primary">
                            {#if agent.avatar_path}
                                <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="h-full w-full object-cover" />
                            {:else}
                                <Bot size={20} />
                            {/if}
                        </div>
                        <div class="min-w-0 flex-1">
                            <h3 class="truncate text-sm font-medium text-text">{agent.name}</h3>
                            <p class="truncate text-xs text-text-secondary">{agent.model_name || '未配置模型'}</p>
                        </div>
                    </button>
                {/each}
            </div>
        {/if}
    </div>
</div>

<CreateAgentModal bind:open={modalOpen} onSuccess={loadData} />

<AgentBundleImportModal
    bind:open={importModalOpen}
    fileContent={importFileContent}
    preview={importPreview}
    onClose={handleImportModalClose}
    onSuccess={loadData}
/>

<ConfirmDialog
    open={showExportConfirm}
    title="未包含的角色关系将会被忽略"
    content={exportConfirmContent}
    confirmText={exportBusy ? '导出中...' : '继续导出'}
    onConfirm={() => { if (!exportBusy) runExport(true); }}
    onCancel={() => { if (!exportBusy) showExportConfirm = false; }}
/>

{#if showExportModal}
    <div
        class="fixed inset-0 z-[90] flex items-center justify-center bg-black/50"
        onclick={(e) => { if (e.target === e.currentTarget) resetExportModal(); }}
        onkeydown={(e) => { if (e.key === 'Escape') resetExportModal(); }}
        role="dialog"
        aria-modal="true"
        tabindex="-1"
    >
        <div
            class="flex max-h-[90vh] w-full max-w-xl flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl"
        >
            <div class="flex items-center justify-between border-b border-border px-4 py-3">
                <div>
                    <h3 class="text-base font-semibold text-text">选择要导出的配置</h3>
                    <p class="mt-0.5 text-xs text-text-secondary">可同时导出多个角色和用户人设</p>
                </div>
                <button
                    type="button"
                    class="rounded-md p-1.5 text-text-secondary transition-colors hover:bg-bg hover:text-text"
                    onclick={resetExportModal}
                    aria-label="关闭"
                >
                    <X size={18} />
                </button>
            </div>

            <div class="border-b border-border px-4 py-2">
                <div class="relative">
                    <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary" />
                    <input
                        type="text"
                        placeholder="搜索角色或用户人设..."
                        bind:value={exportSearchQuery}
                        class="w-full rounded-lg border border-border bg-bg py-1.5 pl-8 pr-3 text-sm focus:outline-none focus:ring-2 focus:ring-primary/20"
                    />
                </div>
            </div>

            <div class="flex-1 space-y-4 overflow-y-auto px-4 py-3">
                <section>
                    <div class="mb-2 flex items-center gap-2 px-1">
                        <Bot size={14} class="text-text-secondary" />
                        <h4 class="text-sm font-medium text-text">AI 角色</h4>
                        <span class="text-xs text-text-secondary">{filteredExportAgents.length}</span>
                    </div>

                    {#if filteredExportAgents.length === 0}
                        <div class="rounded-lg border border-dashed border-border px-3 py-4 text-center text-xs text-text-secondary">
                            没有匹配的角色
                        </div>
                    {:else}
                        <div class="divide-y divide-border overflow-hidden rounded-lg border border-border">
                            {#each filteredExportAgents as agent}
                                {@const selected = isAgentSelected(agent.id)}
                                <button
                                    type="button"
                                    class="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-bg {selected ? 'bg-primary/5' : ''}"
                                    onclick={() => toggleAgentSelection(agent.id)}
                                >
                                    {#if selected}
                                        <CheckSquare size={16} class="shrink-0 text-primary" />
                                    {:else}
                                        <Square size={16} class="shrink-0 text-text-secondary" />
                                    {/if}
                                    <div class="flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-primary">
                                        {#if agent.avatar_path}
                                            <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="h-full w-full object-cover" />
                                        {:else}
                                            <Bot size={16} />
                                        {/if}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <h4 class="truncate text-sm font-medium text-text">{agent.name}</h4>
                                        <p class="truncate text-xs text-text-secondary">{agent.model_name || '未配置模型'}</p>
                                    </div>
                                </button>
                            {/each}
                        </div>
                    {/if}
                </section>

                <section>
                    <div class="mb-2 flex items-center gap-2 px-1">
                        <User size={14} class="text-text-secondary" />
                        <h4 class="text-sm font-medium text-text">用户人设</h4>
                        <span class="text-xs text-text-secondary">{filteredExportPersonas.length}</span>
                    </div>

                    {#if filteredExportPersonas.length === 0}
                        <div class="rounded-lg border border-dashed border-border px-3 py-4 text-center text-xs text-text-secondary">
                            没有匹配的用户人设
                        </div>
                    {:else}
                        <div class="divide-y divide-border overflow-hidden rounded-lg border border-border">
                            {#each filteredExportPersonas as persona}
                                {@const selected = isUserPersonaSelected(persona.id)}
                                <button
                                    type="button"
                                    class="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-bg {selected ? 'bg-primary/5' : ''}"
                                    onclick={() => toggleUserPersonaSelection(persona.id)}
                                >
                                    {#if selected}
                                        <CheckSquare size={16} class="shrink-0 text-primary" />
                                    {:else}
                                        <Square size={16} class="shrink-0 text-text-secondary" />
                                    {/if}
                                    <div class="flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-full bg-emerald-500/10 text-emerald-600">
                                        {#if persona.avatar_path}
                                            <img src={resolveAvatarUrl(persona.avatar_path)} alt={persona.name} class="h-full w-full object-cover" />
                                        {:else}
                                            <User size={16} />
                                        {/if}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <h4 class="truncate text-sm font-medium text-text">{persona.name}</h4>
                                        <p class="truncate text-xs text-text-secondary">用户人设</p>
                                    </div>
                                </button>
                            {/each}
                        </div>
                    {/if}
                </section>
            </div>

            <div class="flex items-center justify-between gap-3 border-t border-border px-4 py-3">
                <p class="min-w-0 text-xs text-text-secondary">已选择 {exportSelectionCount} 项</p>
                <div class="flex shrink-0 items-center gap-2">
                    <button
                        type="button"
                        class="rounded-lg px-3 py-1.5 text-sm text-text-secondary transition-colors hover:bg-bg"
                        onclick={resetExportModal}
                    >
                        取消
                    </button>
                    <button
                        type="button"
                        class="rounded-lg bg-primary px-4 py-1.5 text-sm text-white transition-colors hover:bg-primary-dark disabled:cursor-not-allowed disabled:opacity-60"
                        onclick={handleExport}
                        disabled={exportSelectionCount === 0 || exportBusy}
                    >
                        {exportBusy ? '导出中...' : `导出 ${exportSelectionCount} 项`}
                    </button>
                </div>
            </div>
        </div>
    </div>
{/if}
