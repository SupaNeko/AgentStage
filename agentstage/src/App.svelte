<script lang="ts">
    import './styles.css';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import AgentList from '$lib/components/AgentList.svelte';
    import { appState } from '$lib/stores/appState.svelte';
</script>

<div class="flex h-screen w-screen overflow-hidden bg-bg">
    {#if appState.sidebarOpen}
        <Sidebar />
    {/if}
    
    <main class="flex-1 flex flex-col min-w-0">
        {#if appState.currentView === 'agents'}
            <AgentList />
        {:else if appState.currentView === 'chat'}
            <div class="flex items-center justify-center h-full text-text-secondary">
                <p>会话列表将在这里显示</p>
            </div>
        {:else}
            <div class="flex items-center justify-center h-full text-text-secondary">
                <p>设置将在这里显示</p>
            </div>
        {/if}
    </main>
</div>
