<script lang="ts">
    import { AlertTriangle } from 'lucide-svelte';

    interface Props {
        open: boolean;
        targetName: string;
        onClose: () => void;
        onConfirm: () => void;
    }

    let { open, targetName, onClose, onConfirm }: Props = $props();
    let loading = $state(false);
    let mouseDownOnOverlay = $state(false);

    async function handleConfirm() {
        loading = true;
        try {
            await onConfirm();
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 modal-overlay"
        onmousedown={(e) => { mouseDownOnOverlay = e.target === e.currentTarget; }}
        onclick={(e) => { if (mouseDownOnOverlay && e.target === e.currentTarget) onClose(); mouseDownOnOverlay = false; }}
        role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-96 max-w-full shadow-lg border border-border modal-card" onmousedown={() => mouseDownOnOverlay = false} onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center gap-2 mb-3">
                <AlertTriangle size={20} class="text-red-500" />
                <h3 class="text-lg font-semibold">删除关系</h3>
            </div>
            <p class="text-sm text-text-primary mb-1">确定要删除与 <strong>{targetName}</strong> 的好友关系吗？</p>
            <p class="text-xs text-text-secondary mb-5">
                删除关系是双向的，双方的关系列表中都会移除对方。如果两个角色仍在同一个群中，关系将降级为群友。
            </p>
            <div class="flex gap-2">
                <button onclick={onClose} class="flex-1 py-2 bg-bg text-text-primary rounded-lg hover:bg-surface border border-border">
                    取消
                </button>
                <button
                    onclick={handleConfirm}
                    disabled={loading}
                    class="flex-1 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:opacity-50"
                >
                    {loading ? '删除中...' : '确认删除'}
                </button>
            </div>
        </div>
    </div>
{/if}
