<script lang="ts">
    import { X } from 'lucide-svelte';

    interface Props {
        open: boolean;
        onClose: () => void;
    }

    let { open, onClose }: Props = $props();
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={onClose} role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-96 shadow-xl" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">人设自生成</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded" aria-label="关闭">
                    <X size={18} />
                </button>
            </div>
            <p class="text-sm text-text-secondary">功能开发中，敬请期待...</p>
        </div>
    </div>
{/if}
