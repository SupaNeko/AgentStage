<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Save, Loader2, Clock } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import type { ScheduledTask, Session } from '$lib/types';

    interface Props {
        agentId: string;
        task?: ScheduledTask | null;
        onSave: () => void;
        onClose: () => void;
    }

    let { agentId, task = null, onSave, onClose }: Props = $props();
    let showCloseConfirm = $state(false);

    let submitting = $state(false);
    let sessions = $state<Session[]>([]);

    // Form state
    let description = $state('');
    let taskType = $state<'single' | 'recurring'>('single');
    let triggerMode = $state<'after_minutes' | 'datetime'>('after_minutes');
    let afterMinutes = $state(5);
    let datetimeValue = $state('');
    let intervalMinutes = $state(60);
    let targetSessionId = $state('');

    // Edit mode: editable next_trigger_at
    let editNextTriggerAt = $state('');

    function formatDatetimeLocal(ts: number): string {
        const d = new Date(ts);
        const pad = (n: number) => n.toString().padStart(2, '0');
        return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
    }

    function parseDatetimeLocal(value: string): number {
        return new Date(value).getTime();
    }

    async function loadSessions() {
        try {
            const all = await invoke<Session[]>('list_sessions');
            sessions = all.filter(s =>
                s.participants.some(p => p.participant_type === 'agent' && p.participant_id === agentId)
            );
        } catch (err) {
            logger.error('Failed to load sessions:', err);
        }
    }

    function initForm() {
        if (task) {
            description = task.description;
            taskType = task.task_type;
            triggerMode = task.trigger_mode || 'after_minutes';
            afterMinutes = task.after_minutes || 5;
            if (task.year && task.month && task.day && task.hour !== undefined && task.minute !== undefined) {
                datetimeValue = `${task.year}-${String(task.month).padStart(2, '0')}-${String(task.day).padStart(2, '0')}T${String(task.hour).padStart(2, '0')}:${String(task.minute).padStart(2, '0')}`;
            } else if (task.next_trigger_at) {
                datetimeValue = formatDatetimeLocal(task.next_trigger_at);
            } else {
                datetimeValue = formatDatetimeLocal(Date.now() + 3600000);
            }
            intervalMinutes = task.interval_minutes || 60;
            targetSessionId = task.target_session_id || '';
            editNextTriggerAt = formatDatetimeLocal(task.next_trigger_at);
        } else {
            description = '';
            taskType = 'single';
            triggerMode = 'after_minutes';
            afterMinutes = 5;
            datetimeValue = formatDatetimeLocal(Date.now() + 3600000);
            intervalMinutes = 60;
            targetSessionId = '';
            editNextTriggerAt = '';
        }
    }

    $effect(() => {
        initForm();
        loadSessions();
    });

    async function handleSave() {
        if (!description.trim()) {
            toastStore.error('请填写描述');
            return;
        }

        submitting = true;
        try {
            if (task) {
                const nextTrigger = editNextTriggerAt ? parseDatetimeLocal(editNextTriggerAt) : undefined;
                await invoke('update_timer_command', {
                    agentId,
                    req: {
                        id: task.id,
                        description: description.trim(),
                        next_trigger_at: nextTrigger,
                        target_session_id: targetSessionId || null,
                    }
                });
                toastStore.success('定时任务已更新');
            } else {
                const req: Record<string, unknown> = {
                    description: description.trim(),
                    task_type: taskType,
                    target_session_id: targetSessionId || null,
                };

                if (taskType === 'single') {
                    req.trigger_mode = triggerMode;
                    if (triggerMode === 'after_minutes') {
                        req.after_minutes = afterMinutes;
                    } else {
                        const d = new Date(datetimeValue);
                        req.year = d.getFullYear();
                        req.month = d.getMonth() + 1;
                        req.day = d.getDate();
                        req.hour = d.getHours();
                        req.minute = d.getMinutes();
                    }
                } else {
                    req.interval_minutes = intervalMinutes;
                }

                await invoke('create_timer_command', { agentId, req });
                toastStore.success('定时任务已创建');
            }
            onSave();
            onClose();
        } catch (err: any) {
            logger.error('Failed to save timer:', err);
            toastStore.error('保存失败: ' + String(err));
        } finally {
            submitting = false;
        }
    }

    function setQuickInterval(minutes: number) {
        intervalMinutes = minutes;
    }

    function handleClose() {
        if (submitting) {
            showCloseConfirm = true;
            return;
        }
        onClose();
    }

    function doClose() {
        showCloseConfirm = false;
        onClose();
    }
</script>

<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center modal-overlay" onclick={handleClose} role="dialog" aria-modal="true">
    <div class="bg-surface rounded-xl p-6 w-[28rem] max-h-[90vh] overflow-y-auto shadow-xl modal-card" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-2">
                <Clock size={18} class="text-primary" />
                <h3 class="font-semibold">{task ? '编辑定时任务' : '新建定时任务'}</h3>
            </div>
            <button onclick={handleClose} class="p-1 hover:bg-bg rounded" aria-label="关闭">
                <X size={18} />
            </button>
        </div>

        <div class="space-y-4">
            <!-- Description -->
            <div>
                <label class="block text-sm font-medium mb-1">描述 <span class="text-red-500">*</span></label>
                <input
                    type="text"
                    bind:value={description}
                    disabled={submitting}
                    placeholder="任务描述"
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                />
            </div>

            <!-- Task Type -->
            <div>
                <label class="block text-sm font-medium mb-2">类型</label>
                <div class="flex gap-4">
                    <label class="flex items-center gap-2 text-sm">
                        <input
                            type="radio"
                            name="taskType"
                            value="single"
                            bind:group={taskType}
                            disabled={!!task || submitting}
                            class="text-primary focus:ring-primary"
                        />
                        <span class={task ? 'text-text-secondary' : ''}>单次</span>
                    </label>
                    <label class="flex items-center gap-2 text-sm">
                        <input
                            type="radio"
                            name="taskType"
                            value="recurring"
                            bind:group={taskType}
                            disabled={!!task || submitting}
                            class="text-primary focus:ring-primary"
                        />
                        <span class={task ? 'text-text-secondary' : ''}>循环</span>
                    </label>
                </div>
            </div>

            <!-- Single Mode (create only) -->
            {#if taskType === 'single' && !task}
                <div class="space-y-3">
                    <label class="block text-sm font-medium">触发方式</label>
                    <div class="flex gap-4">
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="radio"
                                name="triggerMode"
                                value="after_minutes"
                                bind:group={triggerMode}
                                disabled={submitting}
                                class="text-primary focus:ring-primary"
                            />
                            多少分钟后
                        </label>
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="radio"
                                name="triggerMode"
                                value="datetime"
                                bind:group={triggerMode}
                                disabled={submitting}
                                class="text-primary focus:ring-primary"
                            />
                            指定时间
                        </label>
                    </div>

                    {#if triggerMode === 'after_minutes'}
                        <div>
                            <label class="block text-sm font-medium mb-1">分钟后触发</label>
                            <input
                                type="number"
                                bind:value={afterMinutes}
                                min={1}
                                disabled={submitting}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                            />
                        </div>
                    {:else}
                        <div>
                            <label class="block text-sm font-medium mb-1">指定时间</label>
                            <input
                                type="datetime-local"
                                bind:value={datetimeValue}
                                disabled={submitting}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                            />
                        </div>
                    {/if}
                </div>
            {/if}

            <!-- Recurring Mode (create only) -->
            {#if taskType === 'recurring' && !task}
                <div class="space-y-3">
                    <label class="block text-sm font-medium mb-1">快捷选择</label>
                    <div class="flex gap-2">
                        <button
                            type="button"
                            onclick={() => setQuickInterval(1440)}
                            disabled={submitting}
                            class="px-3 py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors disabled:opacity-50"
                        >
                            每天
                        </button>
                        <button
                            type="button"
                            onclick={() => setQuickInterval(60)}
                            disabled={submitting}
                            class="px-3 py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors disabled:opacity-50"
                        >
                            每小时
                        </button>
                        <button
                            type="button"
                            onclick={() => setQuickInterval(30)}
                            disabled={submitting}
                            class="px-3 py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors disabled:opacity-50"
                        >
                            每30分钟
                        </button>
                    </div>
                    <div>
                        <label class="block text-sm font-medium mb-1">自定义间隔（分钟）</label>
                        <input
                            type="number"
                            bind:value={intervalMinutes}
                            min={1}
                            disabled={submitting}
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                        />
                    </div>
                </div>
            {/if}

            <!-- Edit mode: show current trigger info read-only + editable next trigger -->
            {#if task}
                <div class="p-3 bg-bg rounded-lg space-y-2">
                    <div class="text-sm text-text-secondary">
                        <span class="font-medium">当前类型:</span>
                        {task.task_type === 'single' ? '单次' : '循环'}
                    </div>
                    {#if task.task_type === 'single' && task.trigger_mode}
                        <div class="text-sm text-text-secondary">
                            <span class="font-medium">触发方式:</span>
                            {task.trigger_mode === 'after_minutes' ? '多少分钟后' : '指定时间'}
                        </div>
                    {/if}
                    {#if task.interval_minutes}
                        <div class="text-sm text-text-secondary">
                            <span class="font-medium">间隔:</span>
                            {task.interval_minutes}分钟
                        </div>
                    {/if}
                    <div>
                        <label class="block text-sm font-medium mb-1">下次触发时间</label>
                        <input
                            type="datetime-local"
                            bind:value={editNextTriggerAt}
                            disabled={submitting}
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                        />
                    </div>
                </div>
            {/if}

            <!-- Target Session -->
            <div>
                <label class="block text-sm font-medium mb-1">期望会话 <span class="text-text-secondary">（可选）</span></label>
                <select
                    bind:value={targetSessionId}
                    disabled={submitting}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50 input-field"
                >
                    <option value="">自动创建新会话</option>
                    {#each sessions as session}
                        <option value={session.id}>{session.participants.map(p => p.name).join(', ')}</option>
                    {/each}
                </select>
            </div>
        </div>

        <div class="flex justify-end gap-3 mt-6">
            <button
                onclick={handleClose}
                disabled={submitting}
                class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors disabled:opacity-50"
            >
                取消
            </button>
            <button
                onclick={handleSave}
                disabled={submitting || !description.trim()}
                class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary"
            >
                {#if submitting}
                    <Loader2 size={16} class="animate-spin" />
                    <span>保存中...</span>
                {:else}
                    <Save size={16} />
                    <span>保存</span>
                {/if}
            </button>
        </div>
    </div>
</div>

<ConfirmDialog
    open={showCloseConfirm}
    title="关闭确认"
    content="操作正在进行中，确定要关闭吗？"
    confirmText="确认关闭"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={doClose}
    onCancel={() => showCloseConfirm = false}
/>
