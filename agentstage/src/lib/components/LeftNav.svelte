<script lang="ts">
    import { appState } from '$lib/stores/appState.svelte';
    import { Bot, MessageSquare, History, Settings } from 'lucide-svelte';

    const navItems = [
        { id: 'agents' as const, label: '角色管理', icon: Bot },
        { id: 'chat' as const, label: '聊天', icon: MessageSquare },
        { id: 'history' as const, label: '历史会话', icon: History },
    ];
</script>

<aside class="w-16 bg-surface border-r border-border flex flex-col h-full shrink-0">
    <!-- Top nav items -->
    <nav class="flex-1 flex flex-col items-center py-4 gap-2">
        {#each navItems as item}
            <button
                class="w-12 h-12 flex items-center justify-center rounded-xl transition-colors relative group {appState.currentView === item.id ? 'bg-primary/10 text-primary' : 'hover:bg-gray-100 text-text-secondary'}"
                onclick={() => appState.switchView(item.id)}
                title={item.label}
            >
                <item.icon size={22} />
                <!-- Tooltip -->
                <span class="absolute left-14 bg-surface border border-border rounded-lg px-2 py-1 text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none shadow-sm z-50">
                    {item.label}
                </span>
            </button>
        {/each}
    </nav>

    <!-- Bottom settings button -->
    <div class="flex flex-col items-center py-4">
        <button
            class="w-12 h-12 flex items-center justify-center rounded-xl transition-colors relative group hover:bg-gray-100 text-text-secondary"
            onclick={() => appState.openSettings()}
            title="设置"
        >
            <Settings size={22} />
            <span class="absolute left-14 bg-surface border border-border rounded-lg px-2 py-1 text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none shadow-sm z-50">
                设置
            </span>
        </button>
    </div>
</aside>
