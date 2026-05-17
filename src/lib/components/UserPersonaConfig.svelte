<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import UserPersonaItem from './UserPersonaItem.svelte';
    import CreateUserPersonaModal from './CreateUserPersonaModal.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { Plus } from 'lucide-svelte';

    let avatarUploadOpen = $state(false);
    let createModalOpen = $state(false);

    let activePersonaId = $derived(settingsStore.settings?.active_persona_id ?? null);

    function handleAvatarUploaded(path: string) {
        avatarUploadOpen = false;
        settingsStore.update({ default_avatar_path: path });
    }

    async function handleDeactivate() {
        await userPersonaStore.activatePersona(null);
    }

    // Load on mount
    $effect(() => {
        userPersonaStore.loadPersonas();
        userPersonaStore.loadCurrentPersona();
    });
</script>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_default"
        targetId="default"
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}

{#if createModalOpen}
    <CreateUserPersonaModal
        onclose={() => createModalOpen = false}
        oncreated={() => userPersonaStore.loadPersonas()}
    />
{/if}

<div class="h-full flex flex-col">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-border">
        <h1 class="text-lg font-semibold">用户角色配置</h1>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        <!-- Default Avatar Row -->
        <div class="flex items-center gap-3 py-2">
            <button
                onclick={() => avatarUploadOpen = true}
                class="w-10 h-10 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0 hover:ring-2 hover:ring-primary"
                title="点击更换默认头像"
            >
                {#if settingsStore.settings?.default_avatar_path}
                    <img src={resolveAvatarUrl(settingsStore.settings.default_avatar_path)} alt="default" class="w-full h-full object-cover" />
                {:else}
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-gray-400"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
                {/if}
            </button>
            <span class="text-sm font-medium text-text-secondary">默认头像</span>
            <div class="flex-1"></div>
            {#if activePersonaId !== null}
                <button onclick={handleDeactivate} class="text-sm text-text-secondary hover:text-primary transition-colors">
                    关闭使用人设
                </button>
            {/if}
        </div>

        <!-- Persona List -->
        <div class="space-y-2">
            {#each userPersonaStore.personas as persona (persona.id)}
                <UserPersonaItem
                    persona={persona}
                    isActive={activePersonaId === persona.id}
                />
            {/each}
        </div>

        <!-- Create Button -->
        <button
            onclick={() => createModalOpen = true}
            class="w-full py-3 rounded-lg border border-dashed border-border flex items-center justify-center gap-2 text-text-secondary hover:text-primary hover:border-primary transition-colors"
        >
            <Plus size={18} />
            <span class="text-sm">创建新人设</span>
        </button>
    </div>
</div>
