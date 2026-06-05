<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { stickerStore } from '$lib/stores/stickerStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import CreateStickerPackModal from './CreateStickerPackModal.svelte';
    import StickerPackDetail from './StickerPackDetail.svelte';
    import type { StickerPack } from '$lib/types';
    import { Plus, Upload, Share2 } from 'lucide-svelte';

    let showCreateModal = $state(false);
    let importing = $state(false);
    let selectedPack = $state<StickerPack | null>(null);

    $effect(() => {
        stickerStore.load();
    });

    async function createPack(name: string) {
        try {
            const pack = await invoke<StickerPack>('create_sticker_pack', { req: { name } });
            await stickerStore.load();
            showCreateModal = false;
            selectedPack = pack;
        } catch (e: any) {
            toastStore.error(e || '创建失败');
        }
    }

    async function exportPack(pack: StickerPack) {
        try {
            const result = await invoke<{ exportedPath: string; fileContent: string; warnings: string[] }>('export_sticker_pack', {
                req: { packId: pack.id },
            });
            const blob = new Blob([result.fileContent], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `${pack.name}.agentsticker`;
            a.click();
            URL.revokeObjectURL(url);
            const path = result.exportedPath;
            if (result.warnings.length > 0) {
                toastStore.error(`导出完成（${path}），但有 ${result.warnings.length} 个警告`);
            } else {
                toastStore.success(`已导出 ${pack.name} 到 ${path}`);
            }
        } catch (e: any) {
            toastStore.error(e || '导出失败');
        }
    }

    async function importPack(file: File) {
        try {
            const fileContent = await file.text();
            const result = await invoke<{ pack: StickerPack; renamed: boolean; warnings: string[] }>('import_sticker_pack', {
                req: { fileContent },
            });
            await stickerStore.load();
            toastStore.success(`已导入表情包 ${result.pack.name}`);
        } catch (e: any) {
            toastStore.error(e || '导入失败');
        }
    }
</script>

{#if selectedPack}
    <StickerPackDetail pack={selectedPack} onBack={() => selectedPack = null} />
{:else}
    <div class="flex flex-col h-full">
        <!-- Header -->
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">表情包管理</h3>
            <div class="flex items-center gap-2">
                <input
                    type="file"
                    accept=".agentsticker"
                    onchange={(e) => {
                        const file = e.currentTarget.files?.[0];
                        if (file) importPack(file);
                        e.currentTarget.value = '';
                    }}
                    class="hidden"
                    id="import-pack-input"
                />
                <label
                    for="import-pack-input"
                    class="flex items-center gap-1 px-3 py-2 bg-surface border border-border rounded-lg text-sm hover:bg-bg transition-colors cursor-pointer"
                >
                    <Upload size={16} />
                    导入
                </label>
                <button
                    onclick={() => showCreateModal = true}
                    class="flex items-center gap-1 px-3 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-dark transition-colors"
                >
                    <Plus size={16} />
                    新建表情包
                </button>
            </div>
        </div>

        <!-- Pack grid -->
        <div class="flex-1 overflow-y-auto p-4">
            {#if stickerStore.packs.length === 0}
                <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                    <p class="text-sm mb-4">还没有表情包</p>
                    <button
                        onclick={() => showCreateModal = true}
                        class="px-4 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-dark transition-colors"
                    >
                        创建第一个表情包
                    </button>
                </div>
            {:else}
                <div class="grid grid-cols-4 gap-4">
                    {#each stickerStore.packs as pack}
                        {@const cover = pack.stickers[0] ?? null}
                        <div class="relative group">
                            <button
                                onclick={() => selectedPack = pack}
                                class="w-full border border-border rounded-xl p-4 text-left hover:border-primary/30 hover:shadow-sm transition-all bg-surface"
                            >
                                <div class="aspect-square mb-3 rounded-lg bg-bg flex items-center justify-center overflow-hidden">
                                    {#if cover}
                                        <img
                                            src={stickerStore.imageUrl(cover.filePath)}
                                            alt={pack.name}
                                            class="w-full h-full object-contain p-2"
                                        />
                                    {:else}
                                        <span class="text-xs text-text-secondary">空表情包</span>
                                    {/if}
                                </div>
                                <div class="font-medium text-sm truncate">{pack.name}</div>
                                <div class="text-xs text-text-secondary">{pack.stickers.length} 个表情</div>
                            </button>
                            <button
                                onclick={(e) => { e.stopPropagation(); exportPack(pack); }}
                                class="absolute top-2 right-2 p-1.5 bg-surface border border-border rounded-lg opacity-0 group-hover:opacity-100 transition-opacity hover:text-primary"
                                title="导出表情包"
                            >
                                <Share2 size={21} />
                            </button>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    </div>
{/if}

<CreateStickerPackModal
    open={showCreateModal}
    onConfirm={createPack}
    onCancel={() => showCreateModal = false}
/>
