<script lang="ts">
    import { stickerStore } from '$lib/stores/stickerStore.svelte';
    import { X } from 'lucide-svelte';

    interface Props {
        onPick: (tag: string) => void;
        onClose: () => void;
    }

    let { onPick, onClose }: Props = $props();
    let activePackId = $state<string>('');

    $effect(() => {
        if (stickerStore.packs.length === 0) {
            stickerStore.load();
        }
    });

    $effect(() => {
        if (stickerStore.packs.length > 0 && !activePackId) {
            activePackId = stickerStore.packs[0].id;
        }
    });

    function handlePick(packName: string, stickerName: string) {
        onPick(`<sticker>${packName}_${stickerName}</sticker>`);
    }

    const activePack = $derived(stickerStore.packs.find((p) => p.id === activePackId));
</script>

<div class="shrink-0 h-72 flex flex-col border-b border-border bg-surface panel-animate">
    <!-- 头部 -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-border shrink-0">
        <span class="text-sm font-medium text-text">选择表情</span>
        <button
            onclick={onClose}
            class="p-1 text-text-secondary hover:text-text hover:bg-bg rounded-lg transition-colors"
            title="关闭"
            type="button"
        >
            <X size={16} />
        </button>
    </div>

    <!-- 表情网格 -->
    <div class="flex-1 overflow-y-auto p-3 min-h-0">
        {#if activePack && activePack.stickers.length > 0}
            <div class="grid grid-cols-7 gap-2">
                {#each activePack.stickers as sticker}
                    <button
                        onclick={() => handlePick(activePack.name, sticker.name)}
                        class="aspect-square p-1.5 hover:bg-bg rounded-xl transition-colors flex items-center justify-center"
                        title={sticker.name}
                        type="button"
                    >
                        <img
                            src={stickerStore.imageUrl(sticker.filePath)}
                            alt={sticker.name}
                            class="w-full h-full object-contain"
                            loading="lazy"
                        />
                    </button>
                {/each}
            </div>
        {:else}
            <div class="flex items-center justify-center h-full text-text-secondary text-sm">
                该表情包暂无表情
            </div>
        {/if}
    </div>

    <!-- 底部标签栏 -->
    <div class="shrink-0 border-t border-border bg-bg/50">
        <div class="flex gap-1 overflow-x-auto px-3 py-2 scrollbar-hide">
            {#each stickerStore.packs as pack}
                {@const isActive = pack.id === activePackId}
                <button
                    onclick={() => (activePackId = pack.id)}
                    class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs whitespace-nowrap transition-colors shrink-0 {isActive
                        ? 'bg-primary text-white'
                        : 'bg-surface text-text-secondary hover:text-text border border-border'}"
                    type="button"
                >
                    {#if pack.stickers[0]}
                        <img
                            src={stickerStore.imageUrl(pack.stickers[0].filePath)}
                            alt={pack.name}
                            class="w-4 h-4 object-contain"
                        />
                    {/if}
                    <span>{pack.name}</span>
                </button>
            {/each}
        </div>
    </div>
</div>

<style>
    .panel-animate {
        animation: slideUpPanel 0.25s ease-out;
    }
    @keyframes slideUpPanel {
        from {
            max-height: 0;
            opacity: 0;
        }
        to {
            max-height: 18rem; /* h-72 */
            opacity: 1;
        }
    }
    .scrollbar-hide {
        -ms-overflow-style: none;
        scrollbar-width: none;
    }
    .scrollbar-hide::-webkit-scrollbar {
        display: none;
    }
</style>
