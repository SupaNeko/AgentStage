<script lang="ts">
    import { appState } from '$lib/stores/appState.svelte';
    import { Bot, MessageSquare, Settings } from 'lucide-svelte';
    
    const navItems = [
        { id: 'agents' as const, label: 'Agent 管理', icon: Bot },
        { id: 'chat' as const, label: '会话', icon: MessageSquare },
        { id: 'settings' as const, label: '设置', icon: Settings },
    ];
</script>

<aside class="w-64 bg-surface border-r border-border flex flex-col h-full">
    <div class="p-4 border-b border-border">
        <h1 class="text-xl font-bold text-primary">AgentStage</h1>
    </div>
    
    <nav class="flex-1 p-2 space-y-1">
        {#each navItems as item}
            <button
                class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-colors {appState.currentView === item.id ? 'bg-primary/10 text-primary' : 'hover:bg-gray-100 text-text-secondary'}"
                onclick={() => appState.currentView = item.id}
            >
                <item.icon size={20} />
                <span>{item.label}</span>
            </button>
        {/each}
    </nav>
</aside>
