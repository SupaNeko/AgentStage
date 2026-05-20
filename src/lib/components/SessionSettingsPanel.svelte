<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { slide } from 'svelte/transition';
    import { X, User, Trash2, RotateCcw, MessageSquare } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import type { SessionConfig, GroupMember } from '$lib/types';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import AddMemberModal from './AddMemberModal.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';

    interface Props {
        open: boolean;
        sessionId: string;
        sessionType: string;
        members: GroupMember[];
        groupAvatar?: string | null;
        mode?: 'chat' | 'history';
        onClose: () => void;
        onMembersChange: () => void;
    }

    let { open, sessionId, sessionType, members, groupAvatar = null, mode = 'chat', onClose, onMembersChange }: Props = $props();

    let config = $state<SessionConfig | null>(null);
    let saveTimer: ReturnType<typeof setTimeout> | null = null;
    let showResetConfirm = $state(false);
    let showDisbandConfirm = $state(false);
    let showAddMember = $state(false);
    let showAvatarModal = $state(false);

    $effect(() => {
        if (open && sessionId) {
            loadConfig();
        }
    });

    async function loadConfig() {
        try {
            const data = await invoke<SessionConfig>('get_session_config', { req: { session_id: sessionId, session_type: sessionType } });
            config = data;
        } catch (err) {
            logger.error('Failed to load session config:', err);
        }
    }

    function queueSave(updates: Partial<SessionConfig>) {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            doSave(updates);
        }, 500);
    }

    async function doSave(updates: Partial<SessionConfig>) {
        if (!config) return;
        try {
            await invoke('update_session_config', {
                req: {
                    session_id: sessionId,
                    history_limit: updates.history_limit,
                    message_limit: updates.message_limit,
                    message_limit_enabled: updates.message_limit_enabled,
                    mute_enabled: updates.mute_enabled,
                }
            });
        } catch (err) {
            logger.error('Failed to save config:', err);
        }
    }

    async function handleReset() {
        try {
            await sessionStore.resetSession(sessionId);
            showResetConfirm = false;
            onClose();
            toastStore.show('会话已重置，历史消息已归档', 'error', 10000);
        } catch (err) {
            logger.error('Reset failed:', err);
            toastStore.show('重置失败，请稍后重试', 'error', 5000);
        }
    }

    async function handleDisband() {
        try {
            await sessionStore.disbandGroup(sessionId);
            showDisbandConfirm = false;
            onClose();
        } catch (err) {
            logger.error('Disband failed:', err);
        }
    }

    async function handleRemoveMember(agentId: string) {
        if (members.filter(m => m.participant_type === 'agent').length <= 2) {
            alert('群聊至少需要保留 2 名角色成员');
            return;
        }
        try {
            await invoke('remove_group_member', { req: { session_id: sessionId, agent_id: agentId } });
            onMembersChange();
        } catch (err) {
            logger.error('Failed to remove member:', err);
        }
    }
</script>

{#if open}
    <div class="absolute inset-y-0 right-0 w-72 bg-surface border-l border-border z-50 flex flex-col shadow-xl session-settings-panel"
         transition:slide={{ duration: 200, axis: 'x' }}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">会话配置</h3>
            <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
        </div>

        <div class="flex-1 overflow-y-auto p-4 space-y-6">
            {#if config}
                <div>
                    <label class="block text-sm font-medium mb-1">历史提示条数</label>
                    <p class="text-xs text-text-secondary mb-2">角色在 Prompt 中能看到该会话的最近 N 条消息</p>
                    <input
                        type="number"
                        min={1}
                        max={200}
                        value={config.history_limit}
                        onchange={(e) => {
                            const v = parseInt(e.currentTarget.value);
                            config = { ...config!, history_limit: v };
                            queueSave({ history_limit: v });
                        }}
                        class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20"
                    />
                </div>

                <div>
                    <label class="block text-sm font-medium mb-1">溢出总结阈值</label>
                    <p class="text-xs text-text-secondary mb-2">当超出历史消息限制的消息累计达到此数量时，自动触发 AI 总结。设为 0 关闭该功能。</p>
                    <input
                        type="number"
                        min={0}
                        max={500}
                        value={config.overflow_summary_threshold ?? 50}
                        onchange={(e) => {
                            const v = parseInt(e.currentTarget.value);
                            config = { ...config!, overflow_summary_threshold: v };
                            queueSave({ overflow_summary_threshold: v });
                        }}
                        class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20"
                    />
                </div>

                <div>
                    <div class="flex items-center justify-between mb-1">
                        <label class="text-sm font-medium">自动消息限制</label>
                        <button
                            onclick={() => {
                                const v = !config!.message_limit_enabled;
                                config = { ...config!, message_limit_enabled: v };
                                queueSave({ message_limit_enabled: v });
                            }}
                            class="relative w-10 h-5 rounded-full transition-colors {config.message_limit_enabled ? 'bg-primary' : 'bg-gray-300'}"
                        >
                            <span class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {config.message_limit_enabled ? 'translate-x-5' : ''}" />
                        </button>
                    </div>
                    <p class="text-xs text-text-secondary mb-2">角色在此会话中最多发送 N 条消息后自动停止</p>
                    <input
                        type="number"
                        min={1}
                        max={999}
                        disabled={!config.message_limit_enabled}
                        value={config.message_limit}
                        onchange={(e) => {
                            const v = parseInt(e.currentTarget.value);
                            config = { ...config!, message_limit: v };
                            queueSave({ message_limit: v });
                        }}
                        class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 disabled:opacity-50"
                    />
                </div>

                {#if mode !== 'history'}
                <div>
                    <div class="flex items-center justify-between mb-1">
                        <label class="text-sm font-medium">禁言</label>
                        <button
                            onclick={() => {
                                const v = !config!.mute_enabled;
                                config = { ...config!, mute_enabled: v };
                                queueSave({ mute_enabled: v });
                            }}
                            class="relative w-10 h-5 rounded-full transition-colors {config.mute_enabled ? 'bg-primary' : 'bg-gray-300'}"
                        >
                            <span class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {config.mute_enabled ? 'translate-x-5' : ''}" />
                        </button>
                    </div>
                    <p class="text-xs text-text-secondary">开启后角色不会自动回复，但你仍可发送消息</p>
                </div>
                {/if}

                {#if sessionType === 'group'}
                    <div>
                        {#if mode !== 'history'}
                        <div class="flex flex-col items-center gap-2 mb-4">
                            <button
                                onclick={() => showAvatarModal = true}
                                class="w-14 h-14 rounded-full bg-primary/10 flex items-center justify-center text-primary hover:ring-2 hover:ring-primary/30 transition-all"
                            >
                                {#if groupAvatar}
                                    <img src={resolveAvatarUrl(groupAvatar)} alt="群聊头像" class="w-full h-full rounded-full object-cover" />
                                {:else}
                                    <MessageSquare size={24} />
                                {/if}
                            </button>
                            <span class="text-xs text-text-secondary">点击更换群聊头像</span>
                        </div>
                        {/if}
                        <label class="block text-sm font-medium mb-2">成员</label>
                        <div class="space-y-1 mb-2">
                            {#each members as member}
                                <div class="flex items-center justify-between p-2 rounded-lg bg-bg">
                                    <div class="flex items-center gap-2">
                                        <div class="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                            {#if member.avatar_path}
                                                <img src={resolveAvatarUrl(member.avatar_path)} alt={member.name} class="w-full h-full object-cover" />
                                            {:else}
                                                <User size={14} />
                                            {/if}
                                        </div>
                                        <span class="text-sm">{member.name}</span>
                                    </div>
                                    {#if mode !== 'history' && member.participant_type === 'agent'}
                                        <button
                                            onclick={() => handleRemoveMember(member.participant_id)}
                                            class="p-1 text-text-secondary hover:text-red-500 rounded"
                                            title="移除成员"
                                        >
                                            <X size={14} />
                                        </button>
                                    {/if}
                                </div>
                            {/each}
                        </div>
                        {#if mode !== 'history'}
                        <button
                            onclick={() => showAddMember = true}
                            class="w-full py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors"
                        >
                            + 添加成员
                        </button>
                        {/if}
                    </div>
                {/if}

                {#if mode !== 'history'}
                <div class="pt-4 border-t border-border">
                    <button
                        onclick={() => showResetConfirm = true}
                        class="flex items-center gap-2 text-sm text-red-500 hover:text-red-600"
                    >
                        <RotateCcw size={16} />
                        重置{sessionType === 'group' ? '群聊' : '会话'}
                    </button>
                </div>

                {#if sessionType === 'group'}
                    <div class="pt-2">
                        <button
                            onclick={() => showDisbandConfirm = true}
                            class="w-full py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 text-sm flex items-center justify-center gap-2"
                        >
                            <Trash2 size={16} />
                            解散群聊
                        </button>
                    </div>
                {/if}
                {/if}
            {:else}
                <div class="text-sm text-text-secondary">加载中...</div>
            {/if}
        </div>
    </div>
{/if}

<ConfirmDialog
    open={showResetConfirm}
    title="重置{sessionType === 'group' ? '群聊' : '会话'}"
    content="重置后当前聊天记录将被归档，相同成员开启新会话。此操作不可撤销。"
    confirmText="确认重置"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={handleReset}
    onCancel={() => showResetConfirm = false}
/>

<ConfirmDialog
    open={showDisbandConfirm}
    title="解散群聊"
    content="解散后群聊将从列表中移除，聊天记录保留在历史记录中。"
    confirmText="确认解散"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={handleDisband}
    onCancel={() => showDisbandConfirm = false}
/>

<AddMemberModal
    open={showAddMember}
    {sessionId}
    existingMemberIds={members.map(m => m.participant_id)}
    onClose={() => showAddMember = false}
    onAdded={onMembersChange}
/>

<AvatarUploadModal
    open={showAvatarModal}
    targetType="group"
    targetId={sessionId}
    currentAvatar={groupAvatar}
    onClose={() => showAvatarModal = false}
    onUploaded={(path) => {
        groupAvatar = path;
        showAvatarModal = false;
    }}
/>
