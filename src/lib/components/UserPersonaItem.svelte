<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import type { UserPersona } from '$lib/stores/userPersonaStore.svelte';
    import { ChevronDown, ChevronUp, User } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    let {
        persona,
        isActive,
    }: {
        persona: UserPersona;
        isActive: boolean;
    } = $props();

    let expanded = $state(false);
    let draftName = $state(persona.name);
    let draftDesc = $state(persona.description ?? '');
    let avatarUploadOpen = $state(false);
    let saving = $state(false);

    function toggleExpand() {
        expanded = !expanded;
        if (expanded) {
            draftName = persona.name;
            draftDesc = persona.description ?? '';
        }
    }

    async function handleActivate() {
        try {
            await userPersonaStore.activatePersona(persona.id);
        } catch (e) {
            toastStore.show('启用失败: ' + String(e), 'error');
        }
    }

    async function handleSave() {
        saving = true;
        try {
            await userPersonaStore.updatePersona({
                id: persona.id,
                name: draftName.trim() || persona.name,
                description: draftDesc.trim() || undefined,
            });
            expanded = false;
        } finally {
            saving = false;
        }
    }

    function handleCancel() {
        draftName = persona.name;
        draftDesc = persona.description ?? '';
        expanded = false;
    }

    async function handleUseDefaultAvatar() {
        const defaultPath = settingsStore.settings?.default_avatar_path;
        if (defaultPath) {
            try {
                await userPersonaStore.updatePersona({ id: persona.id, avatar_path: defaultPath });
            } catch (e) {
                toastStore.show('设置头像失败: ' + String(e), 'error');
            }
        }
    }

    async function handleAvatarUploaded(path: string) {
        avatarUploadOpen = false;
        try {
            await userPersonaStore.updatePersona({ id: persona.id, avatar_path: path });
        } catch (e) {
            toastStore.show('上传头像失败: ' + String(e), 'error');
        }
    }
</script>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_persona"
        targetId={persona.id}
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}

<div class="border border-border rounded-lg bg-surface overflow-hidden">
    <!-- Header Row -->
    <div class="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-gray-50" onclick={toggleExpand}>
        <!-- Avatar (clickable to change) -->
        <button
            onclick={(e) => { e.stopPropagation(); avatarUploadOpen = true; }}
            class="w-9 h-9 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0 hover:ring-2 hover:ring-primary"
            title="点击更换头像"
        >
            {#if persona.avatar_path}
                <img src={resolveAvatarUrl(persona.avatar_path)} alt="" class="w-full h-full object-cover" />
            {:else}
                    <User size={18} class="text-gray-400" />
            {/if}
        </button>

        <!-- Name -->
        <span class="flex-1 font-medium text-sm truncate">{persona.name}</span>

        <!-- Activate Button -->
        {#if isActive}
            <button class="px-3 py-1 rounded-md bg-primary text-white text-xs font-medium shadow-inner">
                启用中
            </button>
        {:else}
            <button
                onclick={(e) => { e.stopPropagation(); handleActivate(); }}
                class="px-3 py-1 rounded-md bg-primary/10 text-primary text-xs font-medium hover:bg-primary hover:text-white transition-colors"
            >
                启用
            </button>
        {/if}

        <!-- Expand Icon -->
        {#if expanded}
            <ChevronUp size={16} class="text-text-secondary" />
        {:else}
            <ChevronDown size={16} class="text-text-secondary" />
        {/if}
    </div>

    <!-- Expanded Content -->
    {#if expanded}
        <div class="px-4 pb-4 border-t border-border bg-bg">
            <div class="pt-3 space-y-3">
                <div>
                    <label class="block text-xs font-medium text-text-secondary mb-1">角色名</label>
                    <input type="text" bind:value={draftName} class="w-full px-3 py-2 rounded-lg border border-border bg-surface text-sm focus:outline-none focus:ring-2 focus:ring-primary" />
                </div>
                <div>
                    <label class="block text-xs font-medium text-text-secondary mb-1">简易人设</label>
                    <textarea bind:value={draftDesc} rows={2} class="w-full px-3 py-2 rounded-lg border border-border bg-surface text-sm focus:outline-none focus:ring-2 focus:ring-primary resize-none"></textarea>
                </div>
                <div class="flex items-center gap-2">
                    <button onclick={handleUseDefaultAvatar} class="text-xs text-primary hover:underline">使用默认头像</button>
                </div>
                <div class="flex justify-end gap-2">
                    <button onclick={handleCancel} class="px-3 py-1.5 rounded-lg text-xs text-text-secondary hover:bg-gray-100">取消</button>
                    <button onclick={handleSave} disabled={saving} class="px-3 py-1.5 rounded-lg text-xs bg-primary text-white disabled:opacity-50">
                        {saving ? '保存中...' : '保存'}
                    </button>
                </div>
            </div>
        </div>
    {/if}
</div>
