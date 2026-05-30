<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import type { TimeRange } from '$lib/types/usage';
    import UsageOverview from './usage/UsageOverview.svelte';
    import UsageByModel from './usage/UsageByModel.svelte';
    import UsageByAgent from './usage/UsageByAgent.svelte';
    import UsageBySession from './usage/UsageBySession.svelte';
    import UsageByTrigger from './usage/UsageByTrigger.svelte';
    import UsageDetail from './usage/UsageDetail.svelte';

    let activeTab = $state<'overview' | 'model' | 'agent' | 'session' | 'trigger' | 'detail'>('overview');
    let timeRange = $state<TimeRange>('last_7_days');

    const timeOptions: { value: TimeRange; label: string }[] = [
        { value: 'today', label: '今日' },
        { value: 'last_7_days', label: '近7天' },
        { value: 'last_30_days', label: '近30天' },
        { value: 'this_month', label: '本月' },
        { value: 'all', label: '全部' },
    ];

    const tabs = [
        { id: 'overview' as const, label: '概览' },
        { id: 'model' as const, label: '按模型' },
        { id: 'agent' as const, label: '按角色' },
        { id: 'session' as const, label: '按会话' },
        { id: 'trigger' as const, label: '按用途' },
        { id: 'detail' as const, label: '明细' },
    ];

    function handleTimeRangeChange(range: TimeRange) {
        timeRange = range;
        usageStore.setTimeRange(range);
        reloadActiveTab();
    }

    function reloadActiveTab() {
        switch (activeTab) {
            case 'overview': usageStore.loadOverview(); break;
            case 'model': usageStore.loadByModel(); break;
            case 'agent': usageStore.loadByAgent(); break;
            case 'session': usageStore.loadBySession(); break;
            case 'trigger': usageStore.loadByTrigger(); break;
            case 'detail': usageStore.loadRecords(); break;
        }
    }

    $effect(() => {
        reloadActiveTab();
    });
</script>

<div class="flex flex-col h-full bg-bg">
    <!-- Header -->
    <div class="border-b border-border px-6 py-4 flex items-center justify-between">
        <h1 class="text-lg font-semibold text-text">模型用量监控</h1>
        <select
            class="bg-surface border border-border rounded-lg px-3 py-1.5 text-sm text-text"
            value={timeRange}
            onchange={(e) => handleTimeRangeChange(e.currentTarget.value as TimeRange)}
        >
            {#each timeOptions as opt}
                <option value={opt.value}>{opt.label}</option>
            {/each}
        </select>
    </div>

    <!-- Tabs -->
    <div class="border-b border-border px-6">
        <div class="flex gap-1">
            {#each tabs as tab}
                <button
                    class="px-4 py-2.5 text-sm font-medium border-b-2 transition-colors {activeTab === tab.id ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}"
                    onclick={() => { activeTab = tab.id; }}
                >
                    {tab.label}
                </button>
            {/each}
        </div>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-auto p-6">
        {#if activeTab === 'overview'}
            <UsageOverview />
        {:else if activeTab === 'model'}
            <UsageByModel />
        {:else if activeTab === 'agent'}
            <UsageByAgent />
        {:else if activeTab === 'session'}
            <UsageBySession />
        {:else if activeTab === 'trigger'}
            <UsageByTrigger />
        {:else if activeTab === 'detail'}
            <UsageDetail />
        {/if}
    </div>
</div>
