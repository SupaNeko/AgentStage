<script lang="ts">
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { X } from 'lucide-svelte';

    let draft = $state({ global_min_trigger_interval: 30 });
    let saving = $state(false);

    $effect(() => {
        if (settingsStore.settings) {
            draft = {
                global_min_trigger_interval: settingsStore.settings.global_min_trigger_interval,
            };
        }
    });

    async function handleSave() {
        saving = true;
        try {
            await settingsStore.update({
                global_min_trigger_interval: draft.global_min_trigger_interval,
            });
            toastStore.show('已保存', 'success', 2000);
        } catch (err) {
            toastStore.show(`保存失败：${err}`, 'error');
        } finally {
            saving = false;
        }
    }

    let { onclose }: { onclose: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[80vh] overflow-y-auto">
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">设置</h3>
            <button onclick={onclose} class="p-1 hover:bg-gray-100 rounded">
                <X size={20} />
            </button>
        </div>
        <div class="p-6 space-y-6">
            <div>
                <label class="block text-sm font-medium mb-1">角色触发消息间隔（秒）</label>
                <input
                    type="number"
                    min="0"
                    bind:value={draft.global_min_trigger_interval}
                    class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                />
                <p class="text-xs text-text-secondary mt-1">0 = 不限制，>0 = 防止角色被连续调用的最小间隔秒数</p>
            </div>
        </div>
        <div class="p-4 border-t border-border flex justify-end">
            <button
                onclick={handleSave}
                disabled={saving}
                class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50"
            >
                {saving ? '保存中...' : '保存'}
            </button>
        </div>
    </div>
</div>
