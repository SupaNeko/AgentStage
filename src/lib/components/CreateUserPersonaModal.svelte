<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { X, User } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    let { onClose, oncreated }: { onClose: () => void; oncreated?: () => void } = $props();

    let name = $state('');
    let description = $state('');
    let avatarPath = $state<string | undefined>(undefined);
    let avatarUploadOpen = $state(false);
    let saving = $state(false);
    let tempId = $state(crypto.randomUUID());

    function handleUseDefaultAvatar() {
        avatarPath = settingsStore.settings?.default_avatar_path ?? undefined;
    }

    function handleAvatarUploaded(path: string) {
        avatarPath = path;
        avatarUploadOpen = false;
    }

    async function handleCreate() {
        if (!name.trim()) {
            toastStore.show('角色名不能为空', 'error');
            return;
        }
        saving = true;
        try {
            await userPersonaStore.createPersona({
                name: name.trim(),
                description: description.trim() || undefined,
                avatar_path: avatarPath,
            });
            oncreated?.();
            onClose();
        } catch (e) {
            toastStore.show('创建失败: ' + String(e), 'error');
        } finally {
            saving = false;
        }
    }
</script>

    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center modal-overlay" onclick={(e) => e.target === e.currentTarget && onClose()}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md p-6 modal-card">
        <div class="flex items-center justify-between mb-4">
            <h2 class="text-lg font-semibold">创建新人设</h2>
            <button onclick={onClose} class="text-text-secondary hover:text-text"><X size={20} /></button>
        </div>

        <!-- Avatar -->
        <div class="flex items-center gap-4 mb-4">
            <button onclick={() => avatarUploadOpen = true} class="w-16 h-16 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0">
                {#if avatarPath}
                    <img src={resolveAvatarUrl(avatarPath)} alt="avatar" class="w-full h-full object-cover" />
                {:else}
                    <User size={28} class="text-gray-400" />
                {/if}
            </button>
            <div class="flex flex-col gap-2">
                <button onclick={handleUseDefaultAvatar} class="text-sm text-primary hover:underline">使用默认头像</button>
            </div>
        </div>

        <!-- Name -->
        <div class="mb-4">
            <label class="block text-sm font-medium mb-1">角色名 <span class="text-red-500">*</span></label>
            <input type="text" bind:value={name} class="w-full px-3 py-2 rounded-lg border border-border bg-bg focus:outline-none focus:ring-2 focus:ring-primary input-field" placeholder="给你的角色起个名字" />
        </div>

        <!-- Description -->
        <div class="mb-6">
            <label class="block text-sm font-medium mb-1">简易人设</label>
            <textarea bind:value={description} rows={3} class="w-full px-3 py-2 rounded-lg border border-border bg-bg focus:outline-none focus:ring-2 focus:ring-primary resize-none input-field" placeholder="其他角色会看到的你的人设描述"></textarea>
        </div>

        <!-- Actions -->
        <div class="flex justify-end gap-2">
            <button onclick={onClose} class="px-4 py-2 rounded-lg text-text-secondary hover:bg-gray-100">取消</button>
            <button
                onclick={handleCreate}
                disabled={!name.trim() || saving}
                class="px-4 py-2 rounded-lg bg-primary text-white disabled:opacity-50 disabled:cursor-not-allowed btn-primary"
            >
                {saving ? '创建中...' : '创建'}
            </button>
        </div>
    </div>
</div>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_persona"
        targetId={tempId}
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}
