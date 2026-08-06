<script lang="ts">
    import { voiceStore } from '$lib/stores/voiceStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { RefreshCw, VolumeX } from 'lucide-svelte';
    import type { Agent, VitsModelInfo } from '$lib/types';
    import VoiceCachePanel from './VoiceCachePanel.svelte';

    let { agent }: { agent: Agent } = $props();

    let form = $state({
        model_name: '',
        model_path: '',
        speaker_id: null as string | null,
        target_language: 'ja',
        emotion_params: '',
        speed: 1.0,
        translate_enabled: true,
        translate_model_config_id: null as string | null,
        generation_mode: 'auto_silent',
    });

    let saving = $state(false);
    let hasExisting = $state(false);
    let showCache = $state(false);

    const selectedModel: VitsModelInfo | null = $derived(
        voiceStore.models.find((m) => m.name === form.model_name) ?? null
    );

    $effect(() => {
        voiceStore.checkRuntime();
        voiceStore.scanModels();
        voiceStore.loadAgentVoice(agent.id).then(() => {
            const existing = voiceStore.agentVoices.get(agent.id);
            if (existing) {
                hasExisting = true;
                form = {
                    model_name: existing.model_name,
                    model_path: existing.model_path,
                    speaker_id: existing.speaker_id,
                    target_language: existing.target_language,
                    emotion_params: existing.emotion_params || '',
                    speed: existing.speed,
                    translate_enabled: existing.translate_enabled,
                    translate_model_config_id: existing.translate_model_config_id,
                    generation_mode: existing.generation_mode,
                };
            }
        });
    });

    function handleModelChange() {
        const m = voiceStore.models.find((x) => x.name === form.model_name);
        form.model_path = m?.path ?? '';
        form.speaker_id = null;
        // 模型声明了语言时，默认对齐目标语言
        if (m?.language) {
            const lang = m.language.toLowerCase();
            if (lang.includes('ja') || lang.includes('jp') || lang.includes('日')) {
                form.target_language = 'ja';
            } else if (lang.includes('zh') || lang.includes('中文') || lang.includes('中')) {
                form.target_language = 'zh';
            } else if (lang.includes('en') || lang.includes('英')) {
                form.target_language = 'en';
            }
        }
    }

    async function handleSave() {
        if (!form.model_name) {
            toastStore.error('请先选择语音模型');
            return;
        }
        saving = true;
        try {
            await voiceStore.saveAgentVoice({
                agent_id: agent.id,
                model_name: form.model_name,
                model_path: form.model_path,
                speaker_id: form.speaker_id,
                target_language: form.target_language,
                emotion_params: form.emotion_params || null,
                speed: form.speed,
                translate_enabled: form.translate_enabled,
                translate_model_config_id: form.translate_model_config_id,
                generation_mode: form.generation_mode,
            });
            hasExisting = true;
            toastStore.success('语音配置已保存');
        } catch (e) {
            toastStore.error('保存失败: ' + e);
        } finally {
            saving = false;
        }
    }

    async function handleDelete() {
        try {
            await voiceStore.deleteAgentVoice(agent.id);
            hasExisting = false;
            form = {
                model_name: '',
                model_path: '',
                speaker_id: null,
                target_language: 'ja',
                emotion_params: '',
                speed: 1.0,
                translate_enabled: true,
                translate_model_config_id: null,
                generation_mode: 'auto_silent',
            };
            toastStore.success('语音配置已删除');
        } catch (e) {
            toastStore.error('删除失败: ' + e);
        }
    }
</script>

<div class="max-w-2xl space-y-5">
    {#if !voiceStore.runtimeAvailable}
        <div class="p-4 bg-red-50 border border-red-200 rounded-lg text-red-700 space-y-2">
            <p class="font-medium flex items-center gap-2"><VolumeX size={16} /> 语音功能不可用</p>
            <p class="text-sm">未检测到 VITS 运行时。请将独立的 Python 推理包放置到以下目录：</p>
            <code class="block text-xs bg-red-100 rounded px-2 py-1">data\vits_runtime\vits_runtime.exe</code>
            <p class="text-sm">语音模型（含 config.json 与 .pth 权重）请放置到：</p>
            <code class="block text-xs bg-red-100 rounded px-2 py-1">data\vits_models\&lt;模型名&gt;\</code>
            <p class="text-xs text-red-500">放置完成后切换标签页或重新打开本页即可重新检测。</p>
        </div>
    {:else}
        <!-- 模型选择 -->
        <div>
            <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">语音模型</h3>
            <div class="space-y-3">
                <div>
                    <div class="flex items-center gap-2 mb-1">
                        <label for="voice-model" class="block text-sm font-medium">模型</label>
                        <button
                            onclick={() => voiceStore.scanModels()}
                            class="text-text-secondary hover:text-primary transition-colors"
                            title="重新扫描模型目录"
                        >
                            <RefreshCw size={14} />
                        </button>
                    </div>
                    <select
                        id="voice-model"
                        bind:value={form.model_name}
                        onchange={handleModelChange}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                    >
                        <option value="">选择模型（data\vits_models\ 下的目录）</option>
                        {#each voiceStore.models as model}
                            <option value={model.name} disabled={!model.has_config}>
                                {model.name}{model.language ? `（${model.language}）` : ''}{model.has_config ? '' : ' [缺少 config.json]'}
                            </option>
                        {/each}
                    </select>
                    {#if voiceStore.models.length === 0}
                        <p class="text-xs text-text-secondary mt-1">
                            未发现可用模型。请将包含 config.json 和 .pth 权重的模型目录放入 data\vits_models\。
                        </p>
                    {/if}
                </div>

                {#if selectedModel && selectedModel.speakers.length > 0}
                    <div>
                        <label for="voice-speaker" class="block text-sm font-medium mb-1">说话人</label>
                        <select
                            id="voice-speaker"
                            bind:value={form.speaker_id}
                            class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                        >
                            <option value={null}>默认</option>
                            {#each selectedModel.speakers as spk}
                                <option value={spk}>{spk}</option>
                            {/each}
                        </select>
                    </div>
                {/if}

                <div>
                    <label for="voice-target-lang" class="block text-sm font-medium mb-1">语音输出语言</label>
                    <select
                        id="voice-target-lang"
                        bind:value={form.target_language}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                    >
                        <option value="zh">中文</option>
                        <option value="ja">日语</option>
                        <option value="en">英语</option>
                    </select>
                    {#if selectedModel?.language}
                        <p class="text-xs text-text-secondary mt-1">模型声明语言：{selectedModel.language}。目标语言与模型语言不一致时合成效果可能变差。</p>
                    {/if}
                </div>
            </div>
        </div>

        <!-- 合成参数 -->
        <div>
            <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">合成参数</h3>
            <div class="space-y-3">
                <div>
                    <label for="voice-speed" class="block text-sm font-medium mb-1">语速：{form.speed.toFixed(1)}x</label>
                    <input id="voice-speed" type="range" min="0.5" max="2.0" step="0.1" bind:value={form.speed} class="w-64" />
                </div>
                <div>
                    <label for="voice-emotion" class="block text-sm font-medium mb-1">情感参数</label>
                    <input
                        id="voice-emotion"
                        type="text"
                        bind:value={form.emotion_params}
                        placeholder="留空使用模型默认，例如 happy、sad"
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                    />
                </div>
            </div>
        </div>

        <!-- 自动翻译 -->
        <div>
            <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">自动翻译</h3>
            <div class="space-y-3">
                <label class="flex items-center gap-2 text-sm">
                    <input type="checkbox" bind:checked={form.translate_enabled} class="rounded" />
                    消息语言与输出语言不一致时自动翻译
                </label>
                {#if form.translate_enabled}
                    <div class="pl-6 space-y-2">
                        <div>
                            <label for="voice-translate-model" class="block text-sm font-medium mb-1">翻译模型</label>
                            <select
                                id="voice-translate-model"
                                bind:value={form.translate_model_config_id}
                                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface"
                            >
                                <option value={null}>使用角色配置的模型</option>
                                {#each modelConfigStore.configs as cfg}
                                    <option value={cfg.id}>{cfg.name}（{cfg.model_name}）</option>
                                {/each}
                            </select>
                        </div>
                        <p class="text-xs text-amber-600">
                            提示：自动翻译会在生成语音前额外调用一次 LLM（检测语言并翻译），会产生额外的 API 开销并延长语音生成时间。翻译调用会计入开销统计的「TTS 翻译」分类。
                        </p>
                    </div>
                {/if}
            </div>
        </div>

        <!-- 生成时机 -->
        <div>
            <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">生成时机</h3>
            <div class="space-y-2">
                <label class="flex items-center gap-2 text-sm">
                    <input type="radio" bind:group={form.generation_mode} value="auto_play" />
                    自动生成并播放（角色输出消息后立即生成并播放）
                </label>
                <label class="flex items-center gap-2 text-sm">
                    <input type="radio" bind:group={form.generation_mode} value="auto_silent" />
                    自动生成不播放（后台预生成，点击喇叭立即播放）
                </label>
                <label class="flex items-center gap-2 text-sm">
                    <input type="radio" bind:group={form.generation_mode} value="manual" />
                    手动生成（点击喇叭后生成并播放）
                </label>
            </div>
        </div>

        <!-- 操作 -->
        <div class="flex items-center gap-3 pt-2 border-t border-border">
            <button
                onclick={handleSave}
                disabled={saving}
                class="px-4 py-1.5 bg-primary text-white rounded-lg text-sm hover:opacity-90 disabled:opacity-50 transition-opacity"
            >
                {saving ? '保存中...' : '保存配置'}
            </button>
            {#if hasExisting}
                <button onclick={handleDelete} class="px-4 py-1.5 text-sm text-red-600 hover:bg-red-50 rounded-lg transition-colors">
                    删除配置
                </button>
            {/if}
            <button onclick={() => (showCache = !showCache)} class="px-4 py-1.5 text-sm text-text-secondary hover:bg-bg rounded-lg transition-colors">
                {showCache ? '隐藏语音缓存' : '查看语音缓存'}
            </button>
        </div>

        {#if showCache}
            <VoiceCachePanel agentId={agent.id} />
        {/if}
    {/if}
</div>
