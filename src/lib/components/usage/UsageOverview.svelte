<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }
</script>

{#if usageStore.overview}
    {@const o = usageStore.overview}
    <div class="space-y-6">
        <!-- Stat Cards -->
        <div class="grid grid-cols-4 gap-4">
            <div class="bg-surface rounded-xl p-4 border border-border">
                <div class="text-sm text-text-secondary mb-1">总调用次数</div>
                <div class="text-2xl font-bold text-text">{formatNumber(o.total_calls)}</div>
            </div>
            <div class="bg-surface rounded-xl p-4 border border-border">
                <div class="text-sm text-text-secondary mb-1">Prompt 消耗</div>
                <div class="text-2xl font-bold text-text">{formatNumber(o.total_prompt_tokens)}</div>
            </div>
            <div class="bg-surface rounded-xl p-4 border border-border">
                <div class="text-sm text-text-secondary mb-1">Completion 消耗</div>
                <div class="text-2xl font-bold text-text">{formatNumber(o.total_completion_tokens)}</div>
            </div>
            <div class="bg-surface rounded-xl p-4 border border-border">
                <div class="text-sm text-text-secondary mb-1">总 Tokens</div>
                <div class="text-2xl font-bold text-text">{formatNumber(o.total_tokens)}</div>
            </div>
        </div>

        <!-- Trend Chart (Simple SVG) -->
        <div class="bg-surface rounded-xl p-4 border border-border">
            <h3 class="text-sm font-semibold text-text mb-4">用量趋势</h3>
            {#if o.daily_trend.length > 0}
                {@const maxTokens = Math.max(...o.daily_trend.map(d => d.tokens))}
                {@const maxCalls = Math.max(...o.daily_trend.map(d => d.calls))}
                <div class="flex items-end gap-2 h-48">
                    {#each o.daily_trend as day}
                        <div class="flex-1 flex flex-col items-center gap-1">
                            <div class="w-full flex flex-col items-center gap-0.5">
                                <!-- Token bar -->
                                <div
                                    class="w-full bg-primary/20 rounded-t"
                                    style="height: {maxTokens > 0 ? (day.tokens / maxTokens) * 120 : 0}px"
                                ></div>
                                <!-- Call bar -->
                                <div
                                    class="w-full bg-primary rounded-b"
                                    style="height: {maxCalls > 0 ? (day.calls / maxCalls) * 20 : 0}px"
                                ></div>
                            </div>
                            <span class="text-xs text-text-secondary truncate w-full text-center">{day.date.slice(5)}</span>
                        </div>
                    {/each}
                </div>
            {:else}
                <div class="text-center text-text-secondary py-12">暂无数据</div>
            {/if}
        </div>
    </div>
{:else if usageStore.loadingOverview}
    <div class="text-center text-text-secondary py-12">加载中...</div>
{:else}
    <div class="text-center text-text-secondary py-12">暂无数据</div>
{/if}
