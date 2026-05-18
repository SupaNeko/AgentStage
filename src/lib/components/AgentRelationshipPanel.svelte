<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, User } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { logger } from '$lib/logger';
    import type { RelationshipItem } from '$lib/types';
    import AddRelationshipModal from './AddRelationshipModal.svelte';
    import ConfirmDeleteRelationshipModal from './ConfirmDeleteRelationshipModal.svelte';
    import { Plus, X } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    let { agentId }: { agentId: string } = $props();

    let items = $state<RelationshipItem[]>([]);
    let loading = $state(false);
    let error = $state('');
    let saveTimeouts = $state<Record<string, ReturnType<typeof setTimeout>>>({});
    let showAddModal = $state(false);
    let showDeleteModal = $state(false);
    let deleteTarget = $state<RelationshipItem | null>(null);

    async function loadRelationships() {
        loading = true;
        error = '';
        try {
            const result = await invoke<RelationshipItem[]>('list_agent_relationships', { agentId });
            items = result;
            logger.debug('[DEBUG AgentRelationshipPanel] loaded', { agentId, count: result.length });
        } catch (err) {
            logger.error('Failed to load relationships:', err);
            error = '加载关系列表失败';
        } finally {
            loading = false;
        }
    }

    async function saveRelationship(item: RelationshipItem) {
        try {
            await invoke('update_agent_relationship', {
                observerId: agentId,
                targetId: item.target_id,
                targetType: item.target_type,
                relationshipText: item.relationship_text,
            });
            logger.debug('[DEBUG AgentRelationshipPanel] saved', { agentId, targetId: item.target_id });
        } catch (err) {
            logger.error('Failed to save relationship:', err);
            error = '保存失败: ' + String(err);
        }
    }

    function handleInput(item: RelationshipItem, value: string) {
        item.relationship_text = value;
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
        }
        saveTimeouts[key] = setTimeout(() => {
            saveRelationship(item);
        }, 500);
    }

    function handleBlur(item: RelationshipItem) {
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
            delete saveTimeouts[key];
        }
        saveRelationship(item);
    }

    async function handleRemove(item: RelationshipItem) {
        try {
            await invoke('remove_friendship', {
                observerId: agentId,
                targetId: item.target_id,
            });
            logger.debug('[DEBUG AgentRelationshipPanel] removed', { agentId, targetId: item.target_id });
            loadRelationships();
        } catch (err) {
            logger.error('Failed to remove friendship:', err);
            toastStore.show('删除关系失败: ' + String(err), 'error');
        }
    }

    function openDeleteModal(item: RelationshipItem) {
        deleteTarget = item;
        showDeleteModal = true;
    }

    $effect(() => {
        if (agentId) {
            loadRelationships();
        }
    });
</script>

<div class="max-w-2xl">
    {#if loading}
        <div class="text-text-secondary text-sm py-4">加载中...</div>
    {:else if error}
        <div class="mb-4 p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
    {:else if items.length === 0}
        <div class="text-text-secondary text-sm py-8 text-center">
            <p>该角色尚未与其他参与者建立关联</p>
            <p class="mt-1">在群聊或私聊中会自动显示关联对象</p>
            <button
                onclick={() => showAddModal = true}
                class="mt-4 inline-flex items-center gap-2 px-4 py-2 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors"
            >
                <Plus size={16} />
                添加关系
            </button>
        </div>
    {:else}
        <p class="text-xs text-text-secondary mb-4">
            以下关系描述会注入到该角色的 Prompt 中，影响其对话态度。
        </p>
        <div class="space-y-3">
            {#each items as item (item.target_id + item.target_type)}
                <div class="flex items-start gap-3 p-3 bg-surface border border-border rounded-lg">
                    <!-- Avatar -->
                    <div class="w-9 h-9 rounded-full bg-primary/10 flex-shrink-0 flex items-center justify-center overflow-hidden">
                        {#if item.target_avatar}
                            <img src={resolveAvatarUrl(item.target_avatar)} alt={item.target_name} class="w-full h-full object-cover" />
                        {:else if item.target_type === 'user_persona'}
                            <User size={18} class="text-primary" />
                        {:else}
                            <Bot size={18} class="text-primary" />
                        {/if}
                    </div>
                    <!-- Info -->
                    <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-2 mb-1.5">
                            <span class="text-sm font-medium truncate">{item.target_name}</span>
                            <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-100 text-text-secondary">
                                {item.target_label}
                            </span>
                            {#if item.target_label === '好友'}
                                <button
                                    onclick={() => openDeleteModal(item)}
                                    class="ml-auto p-1 text-red-400 hover:text-red-600 hover:bg-red-50 rounded-md transition-colors"
                                    title="删除关系"
                                >
                                    <X size={14} />
                                </button>
                            {/if}
                        </div>
                        <div class="relative">
                            <textarea
                                bind:value={item.relationship_text}
                                oninput={(e) => handleInput(item, (e.target as HTMLTextAreaElement).value)}
                                onblur={() => handleBlur(item)}
                                rows={2}
                                maxlength={200}
                                class="w-full px-2.5 py-1.5 text-sm border border-border rounded-md focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-bg"
                                placeholder="该角色对此人的主观看法，如：他是我的好朋友"
                            ></textarea>
                            <div class="absolute bottom-1 right-2 text-[10px] text-text-secondary">
                                {item.relationship_text.length}/200
                            </div>
                        </div>
                    </div>
                </div>
            {/each}
            <button
                onclick={() => showAddModal = true}
                class="w-full flex items-center justify-center gap-2 p-3 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary hover:bg-primary/5 transition-colors"
            >
                <Plus size={16} />
                <span class="text-sm">添加关系</span>
            </button>
        </div>
    {/if}
</div>

<AddRelationshipModal
    open={showAddModal}
    observerId={agentId}
    existingFriendIds={items.filter(i => i.target_label === '好友' && i.target_type === 'agent').map(i => i.target_id)}
    onClose={() => showAddModal = false}
    onAdded={() => { showAddModal = false; loadRelationships(); }}
/>

{#if deleteTarget}
    <ConfirmDeleteRelationshipModal
        open={showDeleteModal}
        targetName={deleteTarget.target_name}
        onClose={() => { showDeleteModal = false; deleteTarget = null; }}
        onConfirm={async () => {
            await handleRemove(deleteTarget!);
            showDeleteModal = false;
            deleteTarget = null;
        }}
    />
{/if}
