<script lang="ts">
    import { usageStore } from '$lib/stores/usageStore.svelte';
    import type { SessionUsageItem, SessionAgentUsageItem, SessionModelUsageItem, SessionAgentModelUsageItem } from '$lib/types/usage';

    let selectedSessionId = $state<string>('');
    let activeSubTab = $state<'overview' | 'agent' | 'model' | 'matrix'>('overview');
    let sessionAgentData = $state<SessionAgentUsageItem[]>([]);
    let sessionModelData = $state<SessionModelUsageItem[]>([]);
    let sessionMatrixData = $state<SessionAgentModelUsageItem[]>([]);

    function formatNumber(n: number): string {
        return n.toLocaleString('zh-CN');
    }

    async function selectSession(sessionId: string) {
        selectedSessionId = sessionId;
        await loadSubTab();
    }

    async function loadSubTab() {
        if (!selectedSessionId) return;
        switch (activeSubTab) {
            case 'agent':
                sessionAgentData = await usageStore.loadSessionAgentBreakdown(selectedSessionId);
                break;
            case 'model':
                sessionModelData = await usageStore.loadSessionModelBreakdown(selectedSessionId);
                break;
            case 'matrix':
                sessionMatrixData = await usageStore.loadSessionAgentModelBreakdown(selectedSessionId);
                break;
        }
    }

    $effect(() => {
        if (usageStore.bySession.length > 0 && !selectedSessionId) {
            selectSession(usageStore.bySession[0].session_id);
        }
    });

    $effect(() => {
        loadSubTab();
    });
</script>

<div class="space-y-4">
    <!-- Session Selector -->
    <select
        class="bg-surface border border-border rounded-lg px-3 py-2 text-sm text-text w-80"
        value={selectedSessionId}
        onchange={(e) => selectSession(e.currentTarget.value)}
    >
        {#each usageStore.bySession as session}
            <option value={session.session_id}>
                {session.session_name} ({session.session_type === 'private' ? '私聊' : '群聊'})
            </option>
        {/each}
    </select>

    {#if selectedSessionId && usageStore.bySession.length > 0}
        {@const session = usageStore.bySession.find(s => s.session_id === selectedSessionId)}
        {#if session}
            <!-- Stats -->
            <div class="grid grid-cols-4 gap-4">
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">调用次数</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(session.calls)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Prompt</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(session.prompt_tokens)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Completion</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(session.completion_tokens)}</div>
                </div>
                <div class="bg-surface rounded-xl p-4 border border-border">
                    <div class="text-sm text-text-secondary mb-1">Total</div>
                    <div class="text-2xl font-bold text-text">{formatNumber(session.total_tokens)}</div>
                </div>
            </div>

            <!-- Sub Tabs -->
            <div class="flex gap-1 border-b border-border">
                {#each [{id: 'overview', label: '概览'}, {id: 'agent', label: '按角色'}, {id: 'model', label: '按模型'}, {id: 'matrix', label: '角色×模型'}] as tab}
                    <button
                        class="px-4 py-2 text-sm font-medium border-b-2 transition-colors {activeSubTab === tab.id ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text'}"
                        onclick={() => { activeSubTab = tab.id as any; }}
                    >
                        {tab.label}
                    </button>
                {/each}
            </div>

            <!-- Sub Tab Content -->
            <div class="bg-surface rounded-xl border border-border overflow-hidden">
                {#if activeSubTab === 'overview'}
                    <div class="px-4 py-8 text-center text-text-secondary">基础统计已显示在上方卡片</div>
                {:else if activeSubTab === 'agent'}
                    <table class="w-full text-sm">
                        <thead class="bg-gray-50 border-b border-border">
                            <tr>
                                <th class="px-4 py-2 text-left font-medium text-text-secondary">角色</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">调用次数</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">Total Tokens</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each sessionAgentData as item}
                                <tr class="border-b border-border">
                                    <td class="px-4 py-2 text-text">{item.agent_name}</td>
                                    <td class="px-4 py-2 text-right text-text">{formatNumber(item.calls)}</td>
                                    <td class="px-4 py-2 text-right text-text font-medium">{formatNumber(item.total_tokens)}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {:else if activeSubTab === 'model'}
                    <table class="w-full text-sm">
                        <thead class="bg-gray-50 border-b border-border">
                            <tr>
                                <th class="px-4 py-2 text-left font-medium text-text-secondary">模型</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">调用次数</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">Total Tokens</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each sessionModelData as item}
                                <tr class="border-b border-border">
                                    <td class="px-4 py-2 text-text">{item.model_name}</td>
                                    <td class="px-4 py-2 text-right text-text">{formatNumber(item.calls)}</td>
                                    <td class="px-4 py-2 text-right text-text font-medium">{formatNumber(item.total_tokens)}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {:else if activeSubTab === 'matrix'}
                    <table class="w-full text-sm">
                        <thead class="bg-gray-50 border-b border-border">
                            <tr>
                                <th class="px-4 py-2 text-left font-medium text-text-secondary">角色</th>
                                <th class="px-4 py-2 text-left font-medium text-text-secondary">模型</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">调用次数</th>
                                <th class="px-4 py-2 text-right font-medium text-text-secondary">Total Tokens</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each sessionMatrixData as item}
                                <tr class="border-b border-border">
                                    <td class="px-4 py-2 text-text">{item.agent_name}</td>
                                    <td class="px-4 py-2 text-text">{item.model_name}</td>
                                    <td class="px-4 py-2 text-right text-text">{formatNumber(item.calls)}</td>
                                    <td class="px-4 py-2 text-right text-text font-medium">{formatNumber(item.total_tokens)}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {/if}
            </div>
        {/if}
    {:else if usageStore.loadingSession}
        <div class="text-center text-text-secondary py-12">加载中...</div>
    {:else}
        <div class="text-center text-text-secondary py-12">暂无数据</div>
    {/if}
</div>
