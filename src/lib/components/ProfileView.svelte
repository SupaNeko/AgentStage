<script lang="ts">
    import UserPersonaConfig from './UserPersonaConfig.svelte';
    import StickerPackManager from './StickerPackManager.svelte';

    const categories = [
        { id: 'user_persona', label: '用户角色配置' },
        { id: 'sticker_packs', label: '表情包管理' },
    ];

    let activeCategory = $state('user_persona');
</script>

<div class="flex h-full">
    <!-- Category List -->
    <div class="w-56 shrink-0 bg-surface border-r border-border flex flex-col profile-sidebar">
        <header class="flex items-center justify-between p-4 border-b border-border">
            <h2 class="text-base font-semibold">个人配置</h2>
        </header>
        <nav class="flex-1 p-2">
            {#each categories as cat}
                <button
                    class="w-full text-left px-3 py-2 rounded-lg text-sm transition-colors {activeCategory === cat.id ? 'bg-primary/10 text-primary font-medium' : 'text-text hover:bg-gray-100'}"
                    onclick={() => activeCategory = cat.id}
                >
                    {cat.label}
                </button>
            {/each}
        </nav>
    </div>

    <!-- Detail Area -->
    <div class="flex-1 min-w-0 detail-panel">
        {#if activeCategory === 'user_persona'}
            <UserPersonaConfig />
        {:else if activeCategory === 'sticker_packs'}
            <StickerPackManager />
        {/if}
    </div>
</div>
