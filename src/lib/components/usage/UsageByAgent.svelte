<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import type { AgentUsageItem, AgentModelUsageItem } from '$lib/types/usage';

    let selectedAgentId = $state<string>('');
    let agentModelData = $state<AgentModelUsageItem[]>([]);

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }

    async function selectAgent(agentId: string) {
        selectedAgentId = agentId;
        agentModelData = await usageStore.loadAgentModelBreakdown(agentId);
    }

    $effect(() => {
        if (usageStore.byAgent.length > 0 && !selectedAgentId) {
            selectAgent(usageStore.byAgent[0].agent_id);
        }
    });
</script>

<div class="space-y-4">
    <!-- Agent Selector -->
    <select
        class="bg-surface border border-border rounded-lg px-3 py-2 text-sm text-text w-64"
        value={selectedAgentId}
        onchange={(e) => selectAgent(e.currentTarget.value)}
    >
        {#each usageStore.byAgent as agent}
            <option value={agent.agent_id}>{agent.agent_name}</option>
        {/each}
    </select>

    {#if selectedAgentId && usageStore.byAgent.length > 0}
        {@const agent = usageStore.byAgent.find(a => a.agent_id === selectedAgentId)}
        {#if agent}
            <!-- Stats -->
            <div class="grid grid-cols-4 gap-4">
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">调用次数</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(agent.calls)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Prompt</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(agent.prompt_tokens)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Completion</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(agent.completion_tokens)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Total</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(agent.total_tokens)}</div>
                </div>
            </div>

            <!-- Model Breakdown -->
            <div class="bg-surface rounded-xl border border-border overflow-hidden">
                <div class="px-4 py-3 border-b border-border font-medium text-text">按模型分布</div>
                <table class="w-full text-sm">
                    <thead class="bg-gray-50 border-b border-border">
                        <tr>
                            <th class="px-4 py-2 text-left font-medium text-text-secondary">模型</th>
                            <th class="px-4 py-2 text-right font-medium text-text-secondary">调用次数</th>
                            <th class="px-4 py-2 text-right font-medium text-text-secondary">Prompt</th>
                            <th class="px-4 py-2 text-right font-medium text-text-secondary">Completion</th>
                            <th class="px-4 py-2 text-right font-medium text-text-secondary">Total</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each agentModelData as item}
                            <tr class="border-b border-border">
                                <td class="px-4 py-2 text-text">{item.model_name}</td>
                                <td class="px-4 py-2 text-right text-text">{formatNumber(item.calls)}</td>
                                <td class="px-4 py-2 text-right text-text">{formatNumber(item.prompt_tokens)}</td>
                                <td class="px-4 py-2 text-right text-text">{formatNumber(item.completion_tokens)}</td>
                                <td class="px-4 py-2 text-right text-text font-medium">{formatNumber(item.total_tokens)}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
    {:else if usageStore.loadingAgent}
        <div class="text-center text-text-secondary py-12">加载中...</div>
    {:else}
        <div class="text-center text-text-secondary py-12">暂无数据</div>
    {/if}
</div>
