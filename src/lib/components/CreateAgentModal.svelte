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
        base_url: '',
        temperature: 0.7,
        max_tokens: 2048,
        thinking_mode: false,
    });
    let submitting = $state(false);
    let error = $state('');

    async function handleSubmit(e: Event) {
        e.preventDefault();
        submitting = true;
        error = '';

        try {
            const req = {
                name: form.name,
                detailed_persona: form.detailed_persona,
                simplified_persona: form.simplified_persona,
                model_provider: form.model_provider,
                model_name: form.model_name,
                api_key: form.api_key,
                base_url: form.base_url || null,
                temperature: form.temperature,
                max_tokens: form.max_tokens,
                thinking_mode: form.thinking_mode,
            };
            await invoke('create_agent', { req });
            open = false;
            onSuccess?.();
            form = { name: '', detailed_persona: '', simplified_persona: '', model_provider: 'openai', model_name: 'gpt-4o', api_key: '', base_url: '', temperature: 0.7, max_tokens: 2048, thinking_mode: false };
        } catch (err: any) {
            error = err.toString();
        } finally {
            submitting = false;
        }
    }
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={() => open = false} role="dialog" aria-modal="true">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">新建角色</h3>
            <button onclick={() => open = false} class="p-1 hover:bg-gray-100 rounded" aria-label="关闭">
                <X size={20} />
            </button>
        </div>

        <form onsubmit={handleSubmit} class="p-4 space-y-4">
            {#if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {/if}

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-name">角色名称 <span class="text-red-500">*</span></label>
                <input id="ca-name" type="text" bind:value={form.name} required maxlength={20}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-detailed">详细人设 <span class="text-red-500">*</span></label>
                <textarea id="ca-detailed" bind:value={form.detailed_persona} required rows={4}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="你是 Fate/stay night 中的角色卫宫士郎，性格坚韧不拔，内心温柔但执拗，拥有强烈的正义感，口头禅是'人被杀就会死'。你是冬木市穗群原学园的学生，同时也是拥有投影魔术的见习魔术师..."></textarea>
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-simplified">简易人设 <span class="text-red-500">*</span></label>
                <textarea id="ca-simplified" bind:value={form.simplified_persona} required rows={2}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="出自 Fate/stay night 的角色卫宫士郎，冬木市的见习魔术师，性格正义感强烈。"></textarea>
                <p class="text-xs text-text-secondary mt-1">这是给其它角色看的角色名片（角色简介）</p>
            </div>

            <div class="grid grid-cols-2 gap-4">
                <div>
                    <label class="block text-sm font-medium mb-1" for="ca-provider">模型提供商 <span class="text-red-500">*</span></label>
                    <select id="ca-provider" bind:value={form.model_provider}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20">
                        <option value="openai">OpenAI</option>
                        <option value="anthropic">Anthropic</option>
                        <option value="google">Google</option>
                        <option value="kimi">Kimi (Moonshot)</option>
                        <option value="minimax">MiniMax</option>
                        <option value="custom">自定义</option>
                    </select>
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1" for="ca-model">模型名称 <span class="text-red-500">*</span></label>
                    <input id="ca-model" type="text" bind:value={form.model_name} required
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                        placeholder="gpt-4o, claude-3-sonnet, kimi-k2..." />
                </div>
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-baseurl">Base URL</label>
                <input id="ca-baseurl" type="text" bind:value={form.base_url}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                    placeholder="可选，默认使用官方地址" />
            </div>

            <div>
                <label class="block text-sm font-medium mb-1" for="ca-apikey">API Key <span class="text-red-500">*</span></label>
                <input id="ca-apikey" type="password" bind:value={form.api_key} required
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>

            <div class="grid grid-cols-2 gap-4">
                <div>
                    <label class="block text-sm font-medium mb-1" for="ca-temp">Temperature</label>
                    <input id="ca-temp" type="number" bind:value={form.temperature} min={0} max={2} step={0.1}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1" for="ca-maxtok">Max Tokens</label>
                    <input id="ca-maxtok" type="number" bind:value={form.max_tokens} min={1}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
            </div>

            <div class="flex items-center gap-2">
                <input id="ca-thinking" type="checkbox" bind:checked={form.thinking_mode}
                    class="w-4 h-4 rounded border-border text-primary focus:ring-primary/20" />
                <label for="ca-thinking" class="text-sm">启用思考模式（如支持）</label>
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
