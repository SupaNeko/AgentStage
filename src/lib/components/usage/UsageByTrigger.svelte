<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import { TRIGGER_TYPE_LABELS } from '$lib/types/usage';

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }

    function getPercentage(value: number, total: number): string {
        if (total === 0) return '0%';
        return ((value / total) * 100).toFixed(1) + '%';
    }
</script>

{#if usageStore.byTrigger.length > 0}
    {@const totalTokens = usageStore.byTrigger.reduce((sum, t) => sum + t.total_tokens, 0)}
    {@const totalCalls = usageStore.byTrigger.reduce((sum, t) => sum + t.calls, 0)}
    <div class="space-y-6">
        <!-- Pie charts -->
        <div class="bg-surface rounded-xl p-4 border border-border">
            <div class="flex gap-8">
                <!-- Calls pie -->
                <div class="flex-1">
                    <h3 class="text-sm font-semibold text-text mb-4 text-center">调用次数占比</h3>
                    <div class="flex items-center gap-6">
                        <svg viewBox="0 0 100 100" class="w-32 h-32 shrink-0">
                            {#each usageStore.byTrigger as trigger, i}
                                {@const prevTotal = usageStore.byTrigger.slice(0, i).reduce((s, t) => s + t.calls, 0)}
                                {@const startAngle = totalCalls > 0 ? (prevTotal / totalCalls) * 360 : 0}
                                {@const endAngle = totalCalls > 0 ? ((prevTotal + trigger.calls) / totalCalls) * 360 : 0}
                                {@const startRad = (startAngle - 90) * Math.PI / 180}
                                {@const endRad = (endAngle - 90) * Math.PI / 180}
                                {@const x1 = 50 + 35 * Math.cos(startRad)}
                                {@const y1 = 50 + 35 * Math.sin(startRad)}
                                {@const x2 = 50 + 35 * Math.cos(endRad)}
                                {@const y2 = 50 + 35 * Math.sin(endRad)}
                                {@const largeArc = endAngle - startAngle > 180 ? 1 : 0}
                                <path
                                    d="M 50 50 L {x1} {y1} A 35 35 0 {largeArc} 1 {x2} {y2} Z"
                                    fill={['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][i % 5]}
                                    stroke="white"
                                    stroke-width="1"
                                />
                            {/each}
                        </svg>
                        <div class="space-y-1.5">
                            {#each usageStore.byTrigger as trigger, i}
                                <div class="flex items-center gap-2 text-xs">
                                    <div class="w-2.5 h-2.5 rounded-full" style="background: {['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][i % 5]}"></div>
                                    <span class="text-text">{TRIGGER_TYPE_LABELS[trigger.trigger_type] || trigger.trigger_type}</span>
                                    <span class="text-text-secondary">{formatNumber(trigger.calls)} 次 ({getPercentage(trigger.calls, totalCalls)})</span>
                                </div>
                            {/each}
                        </div>
                    </div>
                </div>

                <!-- Tokens pie -->
                <div class="flex-1">
                    <h3 class="text-sm font-semibold text-text mb-4 text-center">Token 消耗占比</h3>
                    <div class="flex items-center gap-6">
                        <svg viewBox="0 0 100 100" class="w-32 h-32 shrink-0">
                            {#each usageStore.byTrigger as trigger, i}
                                {@const prevTotal = usageStore.byTrigger.slice(0, i).reduce((s, t) => s + t.total_tokens, 0)}
                                {@const startAngle = totalTokens > 0 ? (prevTotal / totalTokens) * 360 : 0}
                                {@const endAngle = totalTokens > 0 ? ((prevTotal + trigger.total_tokens) / totalTokens) * 360 : 0}
                                {@const startRad = (startAngle - 90) * Math.PI / 180}
                                {@const endRad = (endAngle - 90) * Math.PI / 180}
                                {@const x1 = 50 + 35 * Math.cos(startRad)}
                                {@const y1 = 50 + 35 * Math.sin(startRad)}
                                {@const x2 = 50 + 35 * Math.cos(endRad)}
                                {@const y2 = 50 + 35 * Math.sin(endRad)}
                                {@const largeArc = endAngle - startAngle > 180 ? 1 : 0}
                                <path
                                    d="M 50 50 L {x1} {y1} A 35 35 0 {largeArc} 1 {x2} {y2} Z"
                                    fill={['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][i % 5]}
                                    stroke="white"
                                    stroke-width="1"
                                />
                            {/each}
                        </svg>
                        <div class="space-y-1.5">
                            {#each usageStore.byTrigger as trigger, i}
                                <div class="flex items-center gap-2 text-xs">
                                    <div class="w-2.5 h-2.5 rounded-full" style="background: {['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][i % 5]}"></div>
                                    <span class="text-text">{TRIGGER_TYPE_LABELS[trigger.trigger_type] || trigger.trigger_type}</span>
                                    <span class="text-text-secondary">{formatNumber(trigger.total_tokens)} tokens ({getPercentage(trigger.total_tokens, totalTokens)})</span>
                                </div>
                            {/each}
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Table -->
        <div class="bg-surface rounded-xl border border-border overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-gray-50 border-b border-border">
                    <tr>
                        <th class="px-4 py-3 text-left font-medium text-text-secondary">用途</th>
                        <th class="px-4 py-3 text-right font-medium text-text-secondary">调用次数</th>
                        <th class="px-4 py-3 text-right font-medium text-text-secondary">Prompt</th>
                        <th class="px-4 py-3 text-right font-medium text-text-secondary">Completion</th>
                        <th class="px-4 py-3 text-right font-medium text-text-secondary">Total</th>
                        <th class="px-4 py-3 text-right font-medium text-text-secondary">占比</th>
                    </tr>
                </thead>
                <tbody>
                    {#each usageStore.byTrigger as trigger}
                        <tr class="border-b border-border">
                            <td class="px-4 py-3 text-text">{TRIGGER_TYPE_LABELS[trigger.trigger_type] || trigger.trigger_type}</td>
                            <td class="px-4 py-3 text-right text-text">{formatNumber(trigger.calls)}</td>
                            <td class="px-4 py-3 text-right text-text">{formatNumber(trigger.prompt_tokens)}</td>
                            <td class="px-4 py-3 text-right text-text">{formatNumber(trigger.completion_tokens)}</td>
                            <td class="px-4 py-3 text-right text-text font-medium">{formatNumber(trigger.total_tokens)}</td>
                            <td class="px-4 py-3 text-right text-text">{getPercentage(trigger.total_tokens, totalTokens)}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    </div>
{:else if usageStore.loadingTrigger}
    <div class="text-center text-text-secondary py-12">加载中...</div>
{:else}
    <div class="text-center text-text-secondary py-12">暂无数据</div>
{/if}
