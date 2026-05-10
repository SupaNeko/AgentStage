<script lang="ts">
    import './styles.css';
    import LeftNav from '$lib/components/LeftNav.svelte';
    import AgentList from '$lib/components/AgentList.svelte';
    import AgentDetail from '$lib/components/AgentDetail.svelte';
    import { appState } from '$lib/stores/appState.svelte';
</script>

<div class="flex h-screen w-screen overflow-hidden bg-bg">
    <!-- Left Navigation -->
    <LeftNav />

    <!-- Middle Panel -->
    <div class="w-72 shrink-0 bg-surface border-r border-border">
        {#if appState.currentView === 'agents'}
            <AgentList />
        {:else if appState.currentView === 'chat'}
            <div class="flex flex-col h-full">
                <header class="px-4 py-3 border-b border-border">
                    <h2 class="text-base font-semibold">会话列表</h2>
                </header>
                <div class="flex-1 flex items-center justify-center text-text-secondary text-sm p-4">
                    会话功能即将推出...
                </div>
            </div>
        {:else}
            <div class="flex flex-col h-full">
                <header class="px-4 py-3 border-b border-border">
                    <h2 class="text-base font-semibold">历史会话</h2>
                </header>
                <div class="flex-1 flex items-center justify-center text-text-secondary text-sm p-4">
                    历史会话功能即将推出...
                </div>
            </div>
        {/if}
    </div>

    <!-- Main Content Area -->
    <main class="flex-1 min-w-0 bg-bg">
        {#if appState.currentView === 'agents'}
            <AgentDetail />
        {:else if appState.currentView === 'chat'}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                <p>聊天功能即将推出...</p>
            </div>
        {:else}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                <p>历史会话功能即将推出...</p>
            </div>
        {/if}
    </main>
</div>

<!-- Settings Modal -->
{#if appState.settingsOpen}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={(e) => { if (e.target === e.currentTarget) appState.closeSettings(); }}>
        <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[80vh] overflow-y-auto">
            <div class="flex items-center justify-between p-4 border-b border-border">
                <h3 class="text-lg font-semibold">设置</h3>
                <button onclick={() => appState.closeSettings()} class="p-1 hover:bg-gray-100 rounded">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
                </button>
            </div>
            <div class="p-6 space-y-4">
                <p class="text-text-secondary text-sm">设置功能即将推出...</p>
            </div>
        </div>
    </div>
{/if}
