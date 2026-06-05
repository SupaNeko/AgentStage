<script lang="ts">
    import { X } from 'lucide-svelte';

    interface Props {
        open: boolean;
        onConfirm: (name: string) => void;
        onCancel: () => void;
    }

    let { open, onConfirm, onCancel }: Props = $props();
    let name = $state('');
    let error = $state('');

    function handleConfirm() {
        const trimmed = name.trim();
        if (!trimmed) {
            error = '名称不能为空';
            return;
        }
        if (trimmed.includes('_')) {
            error = '名称不能包含下划线';
            return;
        }
        onConfirm(trimmed);
        name = '';
        error = '';
    }

    function handleCancel() {
        name = '';
        error = '';
        onCancel();
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[90] flex items-center justify-center bg-black/50"
        onclick={(e) => { if (e.target === e.currentTarget) handleCancel(); }}>
        <div class="bg-surface rounded-xl p-6 w-80 max-w-full shadow-lg border border-border">
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold">新建表情包</h3>
                <button onclick={handleCancel} class="p-1 hover:bg-gray-100 rounded">
                    <X size={18} />
                </button>
            </div>
            <div class="mb-4">
                <label class="block text-sm font-medium mb-1">表情包名称</label>
                <input
                    type="text"
                    bind:value={name}
                    placeholder="给表情包起个名字"
                    class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                    onkeydown={(e) => { if (e.key === 'Enter') handleConfirm(); }}
                />
                {#if error}
                    <p class="text-xs text-red-500 mt-1">{error}</p>
                {/if}
            </div>
            <div class="flex justify-end gap-2">
                <button onclick={handleCancel} class="px-4 py-2 rounded-lg text-text-secondary hover:bg-gray-100">取消</button>
                <button onclick={handleConfirm} class="px-4 py-2 rounded-lg bg-primary text-white hover:bg-primary-dark btn-primary">创建</button>
            </div>
        </div>
    </div>
{/if}
