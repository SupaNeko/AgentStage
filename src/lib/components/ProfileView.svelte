<script lang="ts">
    import UserPersonaConfig from './UserPersonaConfig.svelte';

    // 当前只实现 user_persona 分类，后续可扩展
    const categories = [
        { id: 'user_persona', label: '用户角色配置' },
    ];

    let activeCategory = $state('user_persona');
</script>

<div class="flex h-full">
    <!-- Category List -->
    <div class="w-56 shrink-0 bg-surface border-r border-border flex flex-col">
        <div class="px-4 py-3 border-b border-border">
            <h2 class="font-semibold text-sm">个人配置</h2>
        </div>
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
    <div class="flex-1 min-w-0">
        {#if activeCategory === 'user_persona'}
            <UserPersonaConfig />
        {/if}
    </div>
</div>
