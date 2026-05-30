<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import type { ModelUsageItem } from '$lib/types/usage';

    let expandedModelId = $state<string | null>(null);
    let modelAgentBreakdown = $state<Record<string, any[]>>({});

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }

    async function toggleExpand(model: ModelUsageItem) {
        if (expandedModelId === model.model_config_id) {
            expandedModelId = null;
            return;
        }
        expandedModelId = model.model_config_id;
        modelAgentBreakdown = {
            ...modelAgentBreakdown,
            [model.model_config_id]: await usageStore.loadModelAgentBreakdown(model.model_config_id)
        };
    }
</script>

{#if usageStore.byModel.length > 0}
    <div class="bg-surface rounded-xl border border-border overflow-hidden">
        <table class="w-full text-sm">
            <thead class="bg-gray-50 border-b border-border">
                <tr>
                    <th class="px-4 py-3 text-left font-medium text-text-secondary">模型名称</th>
                    <th class="px-4 py-3 text-left font-medium text-text-secondary">供应商</th>
                    <th class="px-4 py-3 text-right font-medium text-text-secondary">调用次数</th>
                    <th class="px-4 py-3 text-right font-medium text-text-secondary">Prompt</th>
                    <th class="px-4 py-3 text-right font-medium text-text-secondary">Completion</th>
                    <th class="px-4 py-3 text-right font-medium text-text-secondary">Total</th>
                </tr>
            </thead>
            <tbody>
                {#each usageStore.byModel as model}
                    <tr class="border-b border-border hover:bg-gray-50 cursor-pointer" onclick={() => toggleExpand(model)}>
                        <td class="px-4 py-3 text-text">
                            {expandedModelId === model.model_config_id ? '▼' : '▶'} {model.model_name}
                        </td>
                        <td class="px-4 py-3 text-text-secondary">{model.provider}</td>
                        <td class="px-4 py-3 text-right text-text">{formatNumber(model.calls)}</td>
                        <td class="px-4 py-3 text-right text-text">{formatNumber(model.prompt_tokens)}</td>
                        <td class="px-4 py-3 text-right text-text">{formatNumber(model.completion_tokens)}</td>
                        <td class="px-4 py-3 text-right text-text font-medium">{formatNumber(model.total_tokens)}</td>
                    </tr>
                    {#if expandedModelId === model.model_config_id}
                        <tr class="bg-gray-50">
                            <td colspan="6" class="px-4 py-3">
                                <div class="text-xs text-text-secondary mb-2">该模型下各角色用量</div>
                                <table class="w-full text-xs">
                                    <thead class="border-b border-border">
                                        <tr>
                                            <th class="px-2 py-1 text-left font-medium text-text-secondary">角色</th>
                                            <th class="px-2 py-1 text-right font-medium text-text-secondary">调用次数</th>
                                            <th class="px-2 py-1 text-right font-medium text-text-secondary">Prompt</th>
                                            <th class="px-2 py-1 text-right font-medium text-text-secondary">Completion</th>
                                            <th class="px-2 py-1 text-right font-medium text-text-secondary">Total</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {#each (modelAgentBreakdown[model.model_config_id] || []) as item}
                                            <tr class="border-b border-border/50">
                                                <td class="px-2 py-1 text-text">{item.agent_name}</td>
                                                <td class="px-2 py-1 text-right text-text">{formatNumber(item.calls)}</td>
                                                <td class="px-2 py-1 text-right text-text">{formatNumber(item.prompt_tokens)}</td>
                                                <td class="px-2 py-1 text-right text-text">{formatNumber(item.completion_tokens)}</td>
                                                <td class="px-2 py-1 text-right text-text font-medium">{formatNumber(item.total_tokens)}</td>
                                            </tr>
                                        {/each}
                                    </tbody>
                                </table>
                            </td>
                        </tr>
                    {/if}
                {/each}
            </tbody>
        </table>
    </div>
{:else if usageStore.loadingModel}
    <div class="text-center text-text-secondary py-12">加载中...</div>
{:else}
    <div class="text-center text-text-secondary py-12">暂无数据</div>
{/if}
