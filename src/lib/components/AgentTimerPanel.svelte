<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, Pencil, Trash2, Pause, Play, Clock, Loader2 } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import type { ScheduledTask } from '$lib/types';
    import TimerEditModal from './TimerEditModal.svelte';

    let { agentId }: { agentId: string } = $props();

    let tasks = $state<ScheduledTask[]>([]);
    let loading = $state(false);
    let showModal = $state(false);
    let editingTask = $state<ScheduledTask | null>(null);

    async function loadTasks() {
        loading = true;
        try {
            const result = await invoke<ScheduledTask[]>('list_agent_timers', { agentId });
            tasks = result;
        } catch (err) {
            logger.error('Failed to load timers:', err);
            toastStore.show('加载定时任务失败', 'error');
        } finally {
            loading = false;
        }
    }

    async function handleDelete(task: ScheduledTask) {
        if (!confirm(`确定要删除定时任务 "${task.description}" 吗？`)) return;
        try {
            await invoke('delete_timer_command', { agentId, taskId: task.id });
            toastStore.show('定时任务已删除', 'success');
            loadTasks();
        } catch (err) {
            logger.error('Failed to delete timer:', err);
            toastStore.show('删除失败', 'error');
        }
    }

    async function handleToggle(task: ScheduledTask) {
        const newActive = task.is_active ? 0 : 1;
        try {
            await invoke('toggle_timer', { agentId, taskId: task.id, isActive: newActive });
            toastStore.show(newActive ? '定时任务已激活' : '定时任务已暂停', 'success');
            loadTasks();
        } catch (err) {
            logger.error('Failed to toggle timer:', err);
            toastStore.show('操作失败', 'error');
        }
    }

    function formatNextTrigger(ts: number): string {
        const d = new Date(ts);
        return d.toLocaleString('zh-CN');
    }

    function getTaskTypeLabel(task: ScheduledTask): string {
        if (task.task_type === 'single') return '单次';
        if (task.task_type === 'recurring') return '循环';
        return task.task_type;
    }

    function getTriggerDetail(task: ScheduledTask): string {
        if (task.task_type === 'single') {
            if (task.trigger_mode === 'after_minutes') {
                return `${task.after_minutes}分钟后`;
            } else if (task.trigger_mode === 'datetime') {
                return '指定时间';
            }
        } else if (task.task_type === 'recurring') {
            if (task.interval_minutes) {
                if (task.interval_minutes >= 1440 && task.interval_minutes % 1440 === 0) {
                    return `每${task.interval_minutes / 1440}天`;
                } else if (task.interval_minutes >= 60 && task.interval_minutes % 60 === 0) {
                    return `每${task.interval_minutes / 60}小时`;
                } else {
                    return `每${task.interval_minutes}分钟`;
                }
            }
        }
        return '';
    }

    function openCreate() {
        editingTask = null;
        showModal = true;
    }

    function openEdit(task: ScheduledTask) {
        editingTask = task;
        showModal = true;
    }

    $effect(() => {
        if (agentId) {
            loadTasks();
        }
    });
</script>

<div class="max-w-2xl">
    <div class="flex items-center justify-between mb-4">
        <h3 class="text-sm font-medium text-text-secondary uppercase tracking-wide">定时任务</h3>
        <button
            onclick={openCreate}
            class="flex items-center gap-1.5 px-3 py-1.5 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors btn-primary"
        >
            <Plus size={16} />
            新建定时任务
        </button>
    </div>

    {#if loading}
        <div class="flex items-center gap-2 text-text-secondary text-sm py-8">
            <Loader2 size={16} class="animate-spin" />
            加载中...
        </div>
    {:else if tasks.length === 0}
        <div class="text-text-secondary text-sm py-8 text-center bg-surface border border-dashed border-border rounded-lg">
            <Clock size={24} class="mx-auto mb-2 opacity-50" />
            <p>暂无定时任务</p>
            <button
                onclick={openCreate}
                class="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 text-primary hover:bg-primary/5 rounded-lg transition-colors text-sm"
            >
                <Plus size={14} />
                创建一个
            </button>
        </div>
    {:else}
        <div class="space-y-2">
            {#each tasks as task (task.id)}
                <div class="flex items-center gap-3 p-3 bg-surface border border-border rounded-lg">
                    <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-2 mb-1">
                            <span class="text-sm font-medium truncate">{task.description}</span>
                            <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-100 text-text-secondary">
                                {getTaskTypeLabel(task)}
                            </span>
                            <span class="text-[10px] px-1.5 py-0.5 rounded-full {task.is_active ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-text-secondary'}">
                                {task.is_active ? '活跃' : '暂停'}
                            </span>
                        </div>
                        <div class="text-xs text-text-secondary flex items-center gap-2">
                            <span>{getTriggerDetail(task)}</span>
                            <span>·</span>
                            <span>下次: {formatNextTrigger(task.next_trigger_at)}</span>
                        </div>
                    </div>
                    <div class="flex items-center gap-1">
                        <button
                            onclick={() => openEdit(task)}
                            class="p-1.5 text-text-secondary hover:text-primary hover:bg-primary/5 rounded-md transition-colors"
                            title="编辑"
                        >
                            <Pencil size={14} />
                        </button>
                        <button
                            onclick={() => handleToggle(task)}
                            class="p-1.5 text-text-secondary hover:text-primary hover:bg-primary/5 rounded-md transition-colors"
                            title={task.is_active ? '暂停' : '恢复'}
                        >
                            {#if task.is_active}
                                <Pause size={14} />
                            {:else}
                                <Play size={14} />
                            {/if}
                        </button>
                        <button
                            onclick={() => handleDelete(task)}
                            class="p-1.5 text-text-secondary hover:text-red-600 hover:bg-red-50 rounded-md transition-colors"
                            title="删除"
                        >
                            <Trash2 size={14} />
                        </button>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>

{#if showModal}
    <TimerEditModal
        {agentId}
        task={editingTask}
        onSave={() => { loadTasks(); }}
        onClose={() => { showModal = false; editingTask = null; }}
    />
{/if}
