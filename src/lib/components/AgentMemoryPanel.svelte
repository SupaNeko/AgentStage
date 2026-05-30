<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, User } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { logger } from '$lib/logger';
    import type { RelationshipItem } from '$lib/types';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import ConfirmResetMemoryModal from './ConfirmResetMemoryModal.svelte';

    let { agentId, longTermMemory = $bindable(''), memoryEnabled = $bindable(true) }: {
        agentId: string;
        longTermMemory: string;
        memoryEnabled: boolean;
    } = $props();

    let items = $state<RelationshipItem[]>([]);
    let loading = $state(false);
    let error = $state('');
    let saveTimeouts = $state<Record<string, ReturnType<typeof setTimeout>>>({});
    let showResetModal = $state(false);

    async function loadRelationships() {
        loading = true;
        error = '';
        try {
            const result = await invoke<RelationshipItem[]>('list_agent_relationships', { agentId });
            items = result;
        } catch (err) {
            logger.error('Failed to load relationships for memory panel:', err);
            error = '加载记忆列表失败';
        } finally {
            loading = false;
        }
    }

    async function saveLongTermMemory() {
        try {
            await invoke('update_agent', { req: { id: agentId, long_term_memory: longTermMemory } });
        } catch (err) {
            logger.error('Failed to save long term memory:', err);
            toastStore.error('保存长期记忆失败');
        }
    }

    async function saveMemory(item: RelationshipItem) {
        try {
            await invoke('update_agent_memory', {
                observerId: agentId,
                targetId: item.target_id,
                targetType: item.target_type,
                memoryText: item.memory_text,
            });
        } catch (err) {
            logger.error('Failed to save memory:', err);
            toastStore.error('保存记忆失败');
        }
    }

    function handleLongTermInput(value: string) {
        longTermMemory = value;
        if (saveTimeouts['long_term']) {
            clearTimeout(saveTimeouts['long_term']);
        }
        saveTimeouts['long_term'] = setTimeout(() => {
            saveLongTermMemory();
        }, 1000);
    }

    function handleLongTermBlur() {
        if (saveTimeouts['long_term']) {
            clearTimeout(saveTimeouts['long_term']);
            delete saveTimeouts['long_term'];
        }
        saveLongTermMemory();
    }

    function handleMemoryInput(item: RelationshipItem, value: string) {
        item.memory_text = value;
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
        }
        saveTimeouts[key] = setTimeout(() => {
            saveMemory(item);
        }, 500);
    }

    function handleMemoryBlur(item: RelationshipItem) {
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
            delete saveTimeouts[key];
        }
        saveMemory(item);
    }

    async function handleReset() {
        try {
            await invoke('reset_agent_memory', { agentId });
            longTermMemory = '';
            items = items.map(i => ({ ...i, memory_text: '' }));
            toastStore.success('记忆已重置');
        } catch (err) {
            logger.error('Failed to reset memory:', err);
            toastStore.error('重置记忆失败');
        }
    }

    async function handleToggleEnabled() {
        memoryEnabled = !memoryEnabled;
        try {
            await invoke('update_agent', { req: { id: agentId, memory_enabled: memoryEnabled } });
        } catch (err) {
            logger.error('Failed to update memory_enabled:', err);
            toastStore.error('更新设置失败');
            memoryEnabled = !memoryEnabled;
        }
    }

    $effect(() => {
        if (agentId) {
            loadRelationships();
        }
    });
</script>

<div class="max-w-2xl space-y-6">
    {#if error}
        <div class="mb-4 p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
    {/if}

    <!-- Controls -->
    <div class="flex items-center justify-between p-3 bg-surface border border-border rounded-lg">
        <div class="flex items-center gap-3">
            <input
                id="memory-enabled"
                type="checkbox"
                checked={memoryEnabled}
                onchange={handleToggleEnabled}
                class="w-4 h-4 rounded border-border text-primary focus:ring-primary"
            />
            <label for="memory-enabled" class="text-sm font-medium">启用记忆</label>
        </div>
        <button
            onclick={() => showResetModal = true}
            class="text-sm text-red-600 hover:text-red-700 hover:bg-red-50 px-3 py-1.5 rounded-lg transition-colors"
        >
            重置记忆
        </button>
    </div>

    {#if !memoryEnabled}
        <div class="text-sm text-text-secondary bg-gray-50 p-3 rounded-lg">
            记忆功能已关闭，当前内容不会被使用
        </div>
    {/if}

    <!-- Long-term Memory -->
    <div>
        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">长期记忆</h3>
        <div class="relative">
            <textarea
                value={longTermMemory}
                oninput={(e) => handleLongTermInput((e.target as HTMLTextAreaElement).value)}
                onblur={handleLongTermBlur}
                rows={8}
                maxlength={3000}
                disabled={!memoryEnabled}
                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface disabled:opacity-50 disabled:cursor-not-allowed input-field"
                placeholder="和该角色有关的记忆"
            ></textarea>
            <div class="absolute bottom-2 right-2 text-[10px] text-text-secondary">
                {longTermMemory.length}/3000
            </div>
        </div>
    </div>

    <!-- Memory about others -->
    <div>
        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">对他人的记忆</h3>
        {#if loading}
            <div class="text-text-secondary text-sm py-4">加载中...</div>
        {:else if items.length === 0}
            <div class="text-text-secondary text-sm py-8 text-center">
                <p>该角色尚未与其他参与者建立关联</p>
                <p class="mt-1">在群聊或私聊中会自动显示</p>
            </div>
        {:else}
            <div class="space-y-3">
                {#each items as item (item.target_id + item.target_type)}
                    <div class="flex items-start gap-3 p-3 bg-surface border border-border rounded-lg">
                        <div class="w-9 h-9 rounded-full bg-primary/10 flex-shrink-0 flex items-center justify-center overflow-hidden">
                            {#if item.target_avatar}
                                <img src={resolveAvatarUrl(item.target_avatar)} alt={item.target_name} class="w-full h-full object-cover" />
                            {:else if item.target_type === 'user_persona'}
                                <User size={18} class="text-primary" />
                            {:else}
                                <Bot size={18} class="text-primary" />
                            {/if}
                        </div>
                        <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-2 mb-1.5">
                                <span class="text-sm font-medium truncate">{item.target_name}</span>
                                <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-100 text-text-secondary">
                                    {item.target_label}
                                </span>
                            </div>
                            <div class="relative">
                                <textarea
                                    value={item.memory_text}
                                    oninput={(e) => handleMemoryInput(item, (e.target as HTMLTextAreaElement).value)}
                                    onblur={() => handleMemoryBlur(item)}
                                    rows={3}
                                    maxlength={500}
                                    disabled={!memoryEnabled}
                                    class="w-full px-2.5 py-1.5 text-sm border border-border rounded-md focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-bg disabled:opacity-50 disabled:cursor-not-allowed input-field"
                                    placeholder="关于此人的重要信息，如喜好、习惯、共同经历..."
                                ></textarea>
                                <div class="absolute bottom-1 right-2 text-[10px] text-text-secondary">
                                    {item.memory_text.length}/500
                                </div>
                            </div>
                        </div>
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>

<ConfirmResetMemoryModal
    open={showResetModal}
    onClose={() => showResetModal = false}
    onConfirm={async () => {
        await handleReset();
        showResetModal = false;
    }}
/>
