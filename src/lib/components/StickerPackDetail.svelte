<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { stickerStore } from '$lib/stores/stickerStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import type { StickerPack } from '$lib/types';
    import { ArrowLeft, Trash2, Edit2, ImagePlus, Upload } from 'lucide-svelte';

    interface Props {
        pack: StickerPack;
        onBack: () => void;
    }

    let { pack, onBack }: Props = $props();
    let renaming = $state(false);
    let renameValue = $state('');
    let showDeleteConfirm = $state(false);
    let importing = $state(false);
    let importProgress = $state('');
    let editingStickerId = $state<string | null>(null);
    let editingStickerName = $state('');

    let currentPack = $derived(stickerStore.packs.find((p) => p.id === pack.id) ?? pack);

    async function updatePackName() {
        if (!renameValue.trim()) return;
        try {
            await invoke('update_sticker_pack', { req: { id: currentPack.id, name: renameValue.trim() } });
            await stickerStore.load();
            renaming = false;
        } catch (e: any) {
            toastStore.error(e || '重命名失败');
        }
    }

    async function deletePack() {
        try {
            await invoke('delete_sticker_pack', { req: { id: currentPack.id } });
            await stickerStore.load();
            showDeleteConfirm = false;
            onBack();
        } catch (e: any) {
            toastStore.error(e || '删除失败');
        }
    }

    async function addStickers(files: FileList) {
        importing = true;
        let success = 0;
        let failed = 0;
        let errorDetails: string[] = [];

        for (const file of Array.from(files)) {
            const rawName = file.name.replace(/\.[^/.]+$/, '');
            // 清洗文件名：下划线替换为空格，避免后端校验失败
            const name = rawName.replace(/[_]/g, ' ').trim();
            if (!name) continue;
            try {
                const base64 = await fileToBase64(file);
                await invoke('add_sticker_to_pack', {
                    req: {
                        packId: currentPack.id,
                        name,
                        imageDataBase64: base64,
                        compressionRatio: 1.0,
                    },
                });
                success++;
            } catch (e: any) {
                failed++;
                const msg = typeof e === 'string' ? e : e?.message || String(e);
                errorDetails.push(`${file.name}: ${msg}`);
            }
            importProgress = `导入中... ${success + failed}/${files.length}`;
        }

        await stickerStore.load();
        importing = false;
        importProgress = '';

        if (failed === 0) {
            toastStore.success(`成功导入 ${success} 个表情`);
        } else if (success === 0) {
            // 全部失败，展示前3条具体错误
            const preview = errorDetails.slice(0, 3).join('\n');
            toastStore.error(`全部导入失败:\n${preview}${errorDetails.length > 3 ? '\n...' : ''}`);
        } else {
            toastStore.error(`${success} 成功，${failed} 失败`);
        }
    }

    function fileToBase64(file: File): Promise<string> {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result as string);
            reader.onerror = reject;
            reader.readAsDataURL(file);
        });
    }

    async function deleteSticker(id: string) {
        try {
            await invoke('delete_stickers', { req: { ids: [id] } });
            await stickerStore.load();
        } catch (e: any) {
            toastStore.error(e || '删除失败');
        }
    }

    function startRenameSticker(sticker: { id: string; name: string }) {
        editingStickerId = sticker.id;
        editingStickerName = sticker.name;
    }

    async function saveStickerRename(id: string) {
        const name = editingStickerName.trim();
        if (!name) return;
        try {
            await invoke('update_sticker', { req: { id, name } });
            await stickerStore.load();
            const updated = stickerStore.packs.find((p) => p.id === currentPack.id);
            if (updated) currentPack = updated;
            editingStickerId = null;
        } catch (e: any) {
            toastStore.error(e || '重命名失败');
        }
    }
</script>

<div class="flex flex-col h-full">
    <!-- Header -->
    <div class="flex items-center gap-3 p-4 border-b border-border">
        <button onclick={onBack} class="p-2 hover:bg-bg rounded-lg transition-colors">
            <ArrowLeft size={18} />
        </button>
        {#if renaming}
            <div class="flex items-center gap-2 flex-1">
                <input
                    type="text"
                    bind:value={renameValue}
                    class="px-2 py-1 bg-bg border border-border rounded text-sm"
                    onkeydown={(e) => { if (e.key === 'Enter') updatePackName(); }}
                />
                <button onclick={updatePackName} class="text-xs text-primary">保存</button>
                <button onclick={() => { renaming = false; renameValue = ''; }} class="text-xs text-text-secondary">取消</button>
            </div>
        {:else}
            <h3 class="text-lg font-semibold flex-1">{currentPack.name}</h3>
        {/if}
        <div class="flex items-center gap-1">
            <button onclick={() => { renaming = true; renameValue = currentPack.name; }} class="p-2 hover:bg-bg rounded-lg transition-colors" title="重命名">
                <Edit2 size={16} />
            </button>
            <button onclick={() => showDeleteConfirm = true} class="p-2 hover:bg-bg rounded-lg transition-colors text-red-500" title="删除表情包">
                <Trash2 size={16} />
            </button>
        </div>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-4">
        <!-- Add stickers area -->
        <div class="mb-6">
            <input
                type="file"
                accept="image/png,image/jpeg,image/gif"
                multiple
                onchange={(e) => {
                    const files = e.currentTarget.files;
                    if (files && files.length > 0) addStickers(files);
                    e.currentTarget.value = '';
                }}
                class="hidden"
                id="batch-sticker-input"
            />
            <label
                for="batch-sticker-input"
                class="flex items-center justify-center gap-2 w-full py-8 border-2 border-dashed border-border rounded-xl hover:border-primary/40 hover:bg-primary/5 transition-colors cursor-pointer"
            >
                {#if importing}
                    <span class="text-sm text-text-secondary">{importProgress}</span>
                {:else}
                    <ImagePlus size={20} class="text-text-secondary" />
                    <span class="text-sm text-text-secondary">点击选择图片批量导入表情</span>
                {/if}
            </label>
        </div>

        <!-- Sticker grid -->
        {#if currentPack.stickers.length === 0}
            <div class="text-center py-12 text-text-secondary text-sm">
                还没有表情，点击上方区域批量导入
            </div>
        {:else}
            <div class="grid grid-cols-6 gap-3">
                {#each currentPack.stickers as sticker}
                    <div class="relative group border border-border rounded-lg p-3 hover:border-primary/30 transition-colors">
                        <img
                            src={stickerStore.imageUrl(sticker.filePath)}
                            alt={sticker.name}
                            class="w-full h-20 object-contain mb-2"
                        />
                        {#if editingStickerId === sticker.id}
                            <div class="flex items-center gap-1">
                                <input
                                    type="text"
                                    bind:value={editingStickerName}
                                    class="w-full px-1 py-0.5 bg-bg border border-border rounded text-xs"
                                    onkeydown={(e) => { if (e.key === 'Enter') saveStickerRename(sticker.id); if (e.key === 'Escape') editingStickerId = null; }}
                                />
                            </div>
                        {:else}
                            <div class="text-xs text-center truncate">{sticker.name}</div>
                        {/if}
                        <div class="absolute top-1 right-1 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                            <button
                                onclick={() => startRenameSticker(sticker)}
                                class="p-1 bg-surface text-text-secondary rounded hover:text-primary"
                                title="重命名"
                            >
                                <Edit2 size={12} />
                            </button>
                            <button
                                onclick={() => deleteSticker(sticker.id)}
                                class="p-1 bg-red-500 text-white rounded"
                                title="删除"
                            >
                                <Trash2 size={12} />
                            </button>
                        </div>
                    </div>
                {/each}
            </div>
        {/if}
        <div class="text-center py-4 text-xs text-text-secondary">
            务必给表情正确的名称，AI会根据名称进行理解
        </div>
    </div>
</div>

<ConfirmDialog
    open={showDeleteConfirm}
    title="删除表情包"
    content={`确定要删除表情包 "${currentPack.name}" 吗？历史消息中的表情可能会失效。`}
    confirmText="删除"
    onConfirm={deletePack}
    onCancel={() => showDeleteConfirm = false}
/>
