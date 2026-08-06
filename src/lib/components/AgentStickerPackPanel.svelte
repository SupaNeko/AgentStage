<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { stickerStore } from '$lib/stores/stickerStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { toastAutoSaved } from '$lib/autoSaveToast';

    interface Props {
        agentId: string;
    }

    let { agentId }: Props = $props();
    let selectedPackIds = $state<Set<string>>(new Set());
    let saving = $state(false);
    let saveTimeout: ReturnType<typeof setTimeout> | null = null;

    async function load() {
        await stickerStore.load();
        const selected = await invoke<string[]>('list_agent_sticker_packs', {
            req: { agentId },
        });
        selectedPackIds = new Set(selected);
    }

    function togglePack(id: string) {
        const next = new Set(selectedPackIds);
        if (next.has(id)) {
            next.delete(id);
        } else {
            next.add(id);
        }
        selectedPackIds = next;
        scheduleAutoSave();
    }

    function scheduleAutoSave() {
        if (saveTimeout) clearTimeout(saveTimeout);
        saveTimeout = setTimeout(() => {
            saveTimeout = null;
            save(true);
        }, 300);
    }

    async function save(isAuto: boolean) {
        saving = true;
        try {
            await invoke('set_agent_sticker_packs', {
                req: {
                    agentId,
                    packIds: Array.from(selectedPackIds),
                },
            });
            if (isAuto) {
                toastAutoSaved();
            } else {
                toastStore.success('保存成功');
            }
        } catch (e: any) {
            toastStore.error(e || '保存失败');
        } finally {
            saving = false;
        }
    }

    /** 手动保存：取消防抖，立即保存当前勾选集合 */
    export async function saveAll() {
        if (saveTimeout) {
            clearTimeout(saveTimeout);
            saveTimeout = null;
        }
        await save(false);
    }

    $effect(() => {
        load();
    });
</script>

<div class="space-y-4">
    <div class="grid grid-cols-4 gap-3">
        {#each stickerStore.packs as pack}
            {@const cover = pack.stickers[0] ?? null}
            <button
                onclick={() => togglePack(pack.id)}
                class="relative border rounded-lg p-3 text-center transition-colors {selectedPackIds.has(pack.id) ? 'border-primary bg-primary/5' : 'border-border hover:bg-bg'}"
            >
                {#if cover}
                    <img
                        src={stickerStore.imageUrl(cover.filePath)}
                        alt={pack.name}
                        class="w-full h-20 object-contain mb-2"
                    />
                {:else}
                    <div class="w-full h-20 flex items-center justify-center text-text-secondary text-sm mb-2">
                        空表情包
                    </div>
                {/if}
                <div class="text-sm font-medium truncate">{pack.name}</div>
                {#if selectedPackIds.has(pack.id)}
                    <div class="absolute top-1 right-1 w-4 h-4 bg-primary text-white rounded-full flex items-center justify-center text-xs">
                        ✓
                    </div>
                {/if}
            </button>
        {/each}
    </div>
</div>
