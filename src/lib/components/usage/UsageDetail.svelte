<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import { TRIGGER_TYPE_LABELS } from '$lib/types/usage';

    let page = $state(1);
    const pageSize = 50;
    let filterAgentId = $state('');
    let filterModelId = $state('');
    let filterSessionId = $state('');
    let filterTriggerType = $state('');

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }

    function formatDate(ts: number): string {
        return new Date(ts).toLocaleString('zh-CN');
    }

    async function applyFilters() {
        page = 1;
        await usageStore.loadRecords(page, pageSize, {
            agentId: filterAgentId || undefined,
            modelConfigId: filterModelId || undefined,
            sessionId: filterSessionId || undefined,
            triggerType: filterTriggerType || undefined,
        });
    }

    async function goToPage(p: number) {
        if (p < 1) return;
        if (usageStore.records && p > Math.ceil(usageStore.records.total / pageSize)) return;
        page = p;
        await usageStore.loadRecords(page, pageSize, {
            agentId: filterAgentId || undefined,
            modelConfigId: filterModelId || undefined,
            sessionId: filterSessionId || undefined,
            triggerType: filterTriggerType || undefined,
        });
    }

    // Initial load when component mounts
    $effect(() => {
        usageStore.loadRecords(page, pageSize);
    });
</script>

<div class="space-y-4">
    <!-- Filters -->
    <div class="flex flex-wrap gap-3 bg-surface rounded-xl p-4 border border-border">
        <input
            type="text"
            placeholder="角色ID"
            class="px-3 py-1.5 text-sm border border-border rounded-lg bg-bg text-text w-32"
            bind:value={filterAgentId}
        />
        <input
            type="text"
            placeholder="模型ID"
            class="px-3 py-1.5 text-sm border border-border rounded-lg bg-bg text-text w-32"
            bind:value={filterModelId}
        />
        <input
            type="text"
            placeholder="会话ID"
            class="px-3 py-1.5 text-sm border border-border rounded-lg bg-bg text-text w-32"
            bind:value={filterSessionId}
        />
        <select
            class="px-3 py-1.5 text-sm border border-border rounded-lg bg-bg text-text"
            bind:value={filterTriggerType}
        >
            <option value="">全部用途</option>
            {#each Object.entries(TRIGGER_TYPE_LABELS) as [value, label]}
                <option value={value}>{label}</option>
            {/each}
        </select>
        <button
            class="px-4 py-1.5 text-sm bg-primary text-white rounded-lg"
            onclick={applyFilters}
        >
            筛选
        </button>
    </div>

    {#if usageStore.records}
        <div class="bg-surface rounded-xl border border-border overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-gray-50 border-b border-border">
                    <tr>
                        <th class="px-4 py-2 text-left font-medium text-text-secondary">时间</th>
                        <th class="px-4 py-2 text-left font-medium text-text-secondary">角色</th>
                        <th class="px-4 py-2 text-left font-medium text-text-secondary">模型</th>
                        <th class="px-4 py-2 text-left font-medium text-text-secondary">会话</th>
                        <th class="px-4 py-2 text-left font-medium text-text-secondary">用途</th>
                        <th class="px-4 py-2 text-right font-medium text-text-secondary">轮次</th>
                        <th class="px-4 py-2 text-right font-medium text-text-secondary">Prompt</th>
                        <th class="px-4 py-2 text-right font-medium text-text-secondary">Completion</th>
                        <th class="px-4 py-2 text-right font-medium text-text-secondary">Total</th>
                    </tr>
                </thead>
                <tbody>
                    {#each usageStore.records.records as record}
                        <tr class="border-b border-border">
                            <td class="px-4 py-2 text-text whitespace-nowrap">{formatDate(record.created_at)}</td>
                            <td class="px-4 py-2 text-text">{record.agent_name}</td>
                            <td class="px-4 py-2 text-text">{record.model_name}</td>
                            <td class="px-4 py-2 text-text">{record.session_name || '-'}</td>
                            <td class="px-4 py-2 text-text">{TRIGGER_TYPE_LABELS[record.trigger_type] || record.trigger_type}</td>
                            <td class="px-4 py-2 text-right text-text">{record.call_round}</td>
                            <td class="px-4 py-2 text-right text-text">{formatNumber(record.prompt_tokens)}</td>
                            <td class="px-4 py-2 text-right text-text">{formatNumber(record.completion_tokens)}</td>
                            <td class="px-4 py-2 text-right text-text font-medium">{formatNumber(record.total_tokens)}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>

        <!-- Pagination -->
        {#if usageStore.records.total > pageSize}
            {@const totalPages = Math.ceil(usageStore.records.total / pageSize)}
            <div class="flex items-center justify-center gap-2 mt-4">
                <button
                    class="px-3 py-1 text-sm rounded border border-border bg-surface text-text disabled:opacity-50"
                    disabled={page <= 1}
                    onclick={() => goToPage(page - 1)}
                >
                    上一页
                </button>
                <span class="text-sm text-text">{page} / {totalPages}</span>
                <button
                    class="px-3 py-1 text-sm rounded border border-border bg-surface text-text disabled:opacity-50"
                    disabled={page >= totalPages}
                    onclick={() => goToPage(page + 1)}
                >
                    下一页
                </button>
            </div>
        {/if}
    {:else if usageStore.loadingRecords}
        <div class="text-center text-text-secondary py-12">加载中...</div>
    {:else}
        <div class="text-center text-text-secondary py-12">暂无数据</div>
    {/if}
</div>
