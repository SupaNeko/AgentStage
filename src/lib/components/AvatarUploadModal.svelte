<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Upload } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    interface Props {
        open: boolean;
        targetType: 'user' | 'agent' | 'group';
        targetId: string;
        currentAvatar: string | null;
        onClose: () => void;
        onUploaded: (path: string) => void;
    }

    let { open, targetType, targetId, currentAvatar, onClose, onUploaded }: Props = $props();
    let uploading = $state(false);
    let fileInput: HTMLInputElement | undefined = $state(undefined);

    function handleFileSelect(e: Event) {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = async (ev) => {
            const base64 = ev.target?.result as string;
            if (!base64) return;
            uploading = true;
            try {
                const path = await invoke<string>('upload_avatar', {
                    req: { target_type: targetType, target_id: targetId, image_data_base64: base64 }
                });
                toastStore.show('头像上传成功', 'success', 2000);
                onUploaded(path);
            } catch (err) {
                toastStore.show('上传失败: ' + String(err), 'error', 5000);
            } finally {
                uploading = false;
            }
        };
        reader.readAsDataURL(file);
    }
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={onClose}>
        <div class="bg-surface rounded-xl p-6 w-80 shadow-xl" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">头像管理</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded"><X size={18} /></button>
            </div>
            <div class="flex flex-col items-center gap-4">
                {#if currentAvatar}
                    <img src={currentAvatar} alt="当前头像" class="w-24 h-24 rounded-full object-cover" />
                {:else}
                    <div class="w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                        <span class="text-2xl">?</span>
                    </div>
                {/if}
                <input type="file" accept="image/*" bind:this={fileInput} onchange={handleFileSelect} class="hidden" />
                <button
                    onclick={() => fileInput?.click()}
                    disabled={uploading}
                    class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50"
                >
                    <Upload size={16} />
                    {uploading ? '上传中...' : '上传新头像'}
                </button>
            </div>
        </div>
    </div>
{/if}
