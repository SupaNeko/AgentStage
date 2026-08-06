<script lang="ts">
    import { voiceStore } from '$lib/stores/voiceStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { Play, Trash2 } from 'lucide-svelte';
    import { formatTime } from '$lib/utils';
    import type { VoiceCacheItem } from '$lib/types';

    let { agentId = null }: { agentId?: string | null } = $props();

    let items = $state<VoiceCacheItem[]>([]);
    let loading = $state(false);

    const totalSize = $derived(items.reduce((sum, i) => sum + (i.file_size || 0), 0));

    async function load() {
        loading = true;
        try {
            items = await voiceStore.listCache(agentId ?? undefined);
        } catch (e) {
            toastStore.error('加载语音缓存失败: ' + e);
        } finally {
            loading = false;
        }
    }

    async function handlePlay(item: VoiceCacheItem) {
        voiceStore.playFile(item.file_path, item.message_id);
    }

    async function handleDelete(id: string) {
        try {
            await voiceStore.deleteCache(id);
            items = items.filter((i) => i.id !== id);
            toastStore.success('已删除');
        } catch (e) {
            toastStore.error('删除失败: ' + e);
        }
    }

    async function handleClearAll() {
        try {
            await voiceStore.clearCache();
            items = [];
            toastStore.success('语音缓存已清空');
        } catch (e) {
            toastStore.error('清空失败: ' + e);
        }
    }

    function formatSize(bytes: number): string {
        if (bytes >= 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB';
        return (bytes / 1024).toFixed(1) + ' KB';
    }

    $effect(() => {
        load();
    });
</script>

<div class="space-y-3">
    <div class="flex justify-between items-center">
        <p class="text-sm text-text-secondary">
            共 {items.length} 条，占用 {formatSize(totalSize)}
        </p>
        {#if items.length > 0}
            <button onclick={handleClearAll} class="text-sm text-red-600 hover:underline">清空全部</button>
        {/if}
    </div>
    {#if loading}
        <p class="text-sm text-text-secondary">加载中...</p>
    {:else if items.length === 0}
        <p class="text-sm text-text-secondary">暂无语音缓存</p>
    {:else}
        <ul class="space-y-1 max-h-64 overflow-y-auto">
            {#each items as item (item.id)}
                <li class="flex items-center justify-between gap-2 text-sm px-3 py-2 bg-bg rounded-lg border border-border">
                    <div class="flex-1 min-w-0">
                        <p class="truncate text-text">消息 {item.message_id.slice(0, 8)}…</p>
                        <p class="text-xs text-text-secondary">{formatSize(item.file_size)} · {formatTime(item.created_at)}</p>
                    </div>
                    <div class="flex items-center gap-1 shrink-0">
                        <button
                            onclick={() => handlePlay(item)}
                            class="p-1.5 rounded hover:bg-surface text-primary transition-colors"
                            title="播放"
                        >
                            <Play size={14} />
                        </button>
                        <button
                            onclick={() => handleDelete(item.id)}
                            class="p-1.5 rounded hover:bg-surface text-red-600 transition-colors"
                            title="删除"
                        >
                            <Trash2 size={14} />
                        </button>
                    </div>
                </li>
            {/each}
        </ul>
    {/if}
</div>
