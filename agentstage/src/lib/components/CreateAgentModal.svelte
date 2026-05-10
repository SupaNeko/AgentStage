<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X } from 'lucide-svelte';
    
    let { open = $bindable(false), onSuccess }: { open: boolean; onSuccess?: () => void } = $props();
    
    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        model_provider: 'openai',
        model_name: 'gpt-4o',
        api_key: '',
    });
    let submitting = $state(false);
    let error = $state('');
    
    async function handleSubmit(e: Event) {
        e.preventDefault();
        submitting = true;
        error = '';
        
        try {
            await invoke('create_agent', { req: form });
            open = false;
            onSuccess?.();
            form = { name: '', detailed_persona: '', simplified_persona: '', model_provider: 'openai', model_name: 'gpt-4o', api_key: '' };
        } catch (err: any) {
            error = err.toString();
        } finally {
            submitting = false;
        }
    }
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={() => open = false}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">新建 Agent</h3>
            <button onclick={() => open = false} class="p-1 hover:bg-gray-100 rounded">
                <X size={20} />
            </button>
        </div>
        
        <form onsubmit={handleSubmit} class="p-4 space-y-4">
            {#if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {/if}
            
            <div>
                <label class="block text-sm font-medium mb-1">Agent 名称 <span class="text-red-500">*</span></label>
                <input type="text" bind:value={form.name} required maxlength={20}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">详细人设 <span class="text-red-500">*</span></label>
                <textarea bind:value={form.detailed_persona} required rows={4}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="你是 Alice，一位来自维多利亚时代的贵族少女..."></textarea>
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">简易人设 <span class="text-red-500">*</span></label>
                <textarea bind:value={form.simplified_persona} required rows={2}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="Alice，贵族少女，性格优雅但内心叛逆，是你的青梅竹马。"></textarea>
            </div>
            
            <div class="grid grid-cols-2 gap-4">
                <div>
                    <label class="block text-sm font-medium mb-1">模型提供商</label>
                    <select bind:value={form.model_provider}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20">
                        <option value="openai">OpenAI</option>
                        <option value="anthropic">Anthropic</option>
                        <option value="google">Google</option>
                        <option value="custom">自定义</option>
                    </select>
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1">模型名称</label>
                    <input type="text" bind:value={form.model_name}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">API Key <span class="text-red-500">*</span></label>
                <input type="password" bind:value={form.api_key} required
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            
            <div class="flex justify-end gap-3 pt-2">
                <button type="button" onclick={() => open = false}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">取消</button>
                <button type="submit" disabled={submitting}
                    class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                    {submitting ? '创建中...' : '创建'}
                </button>
            </div>
        </form>
    </div>
</div>
{/if}
