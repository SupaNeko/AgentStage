<script lang="ts">
    let { open, onClose, onConfirm }: {
        open: boolean;
        onClose: () => void;
        onConfirm: () => void;
    } = $props();
</script>

{#if open}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
        <div class="bg-surface rounded-xl shadow-lg p-6 w-full max-w-sm mx-4">
            <h3 class="text-lg font-semibold mb-2">确认重置记忆</h3>
            <p class="text-sm text-text-secondary mb-6">
                此操作将清空该角色的长期记忆和所有对他人的记忆，且无法撤销。是否继续？
            </p>
            <div class="flex justify-end gap-3">
                <button
                    onclick={onClose}
                    class="px-4 py-2 text-sm text-text-secondary hover:bg-gray-100 rounded-lg transition-colors"
                >
                    取消
                </button>
                <button
                    onclick={onConfirm}
                    class="px-4 py-2 text-sm bg-red-600 text-white hover:bg-red-700 rounded-lg transition-colors"
                >
                    确认重置
                </button>
            </div>
        </div>
    </div>
{/if}
