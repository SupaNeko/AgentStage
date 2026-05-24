<script lang="ts">
    interface Props {
        open: boolean;
        title: string;
        content: string;
        confirmText: string;
        confirmClass?: string;
        onConfirm: () => void;
        onCancel: () => void;
    }

    let { open, title, content, confirmText, confirmClass = 'bg-primary text-white', onConfirm, onCancel }: Props = $props();
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 modal-overlay" onclick={onCancel}>
        <div class="bg-surface rounded-xl p-6 w-80 max-w-full shadow-lg border border-border modal-card" onclick={(e) => e.stopPropagation()}>
            <h3 class="text-lg font-semibold mb-2">{title}</h3>
            <p class="text-sm text-text-secondary mb-6">{content}</p>
            <div class="flex justify-end gap-2">
                <button onclick={onCancel} class="px-4 py-2 text-sm rounded-lg hover:bg-bg transition-colors">取消</button>
                <button onclick={onConfirm} class="px-4 py-2 text-sm rounded-lg btn-primary {confirmClass}">{confirmText}</button>
            </div>
        </div>
    </div>
{/if}
