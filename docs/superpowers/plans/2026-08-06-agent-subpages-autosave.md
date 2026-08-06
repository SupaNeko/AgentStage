# 角色配置子页面 UI 统一 + 自动保存 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一角色详情页五个子 tab 的保存按钮到右下角固定 footer，并全面实现自动保存（文本框失焦保存、其余控件即时保存），自动保存成功后顶部滑动 toast 提示"已自动保存"。

**Architecture:** 纯前端改动。新增一个 `autoSaveToast` 工具函数（2 秒去重）；各面板通过 `export function saveAll()` 暴露手动保存入口，`AgentDetail.svelte` 用 `bind:this` 持有面板引用并在统一 footer 中调用。语音面板额外暴露 `deleteConfig()` 和 `$bindable` 的 `hasExisting`。自动保存全部采用显式事件处理（onchange/onblur），不用 `$effect` 监听表单，避免初始加载误触发保存。

**Tech Stack:** Svelte 5（Runes）、Vitest + @testing-library/svelte、Tauri v2 `invoke`（前端 camelCase 参数）。

**项目约定：** 按 AGENTS.md，本计划不含 git commit 步骤；所有任务完成后由用户决定是否提交。

**规格文档:** `docs/superpowers/specs/2026-08-06-agent-subpages-autosave-design.md`

---

### Task 1: 自动保存 toast 去重工具

**Files:**
- Create: `src/lib/autoSaveToast.ts`
- Test: `src/lib/autoSaveToast.test.ts`

- [ ] **Step 1: 写失败测试**

创建 `src/lib/autoSaveToast.test.ts`：

```ts
import { describe, it, expect, afterEach, vi } from 'vitest';
import { toastStore } from '$lib/stores/toastStore.svelte';

async function loadModule() {
    vi.resetModules();
    return await import('./autoSaveToast');
}

describe('toastAutoSaved', () => {
    afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
    });

    it('2秒内重复调用只提示一次', async () => {
        vi.useFakeTimers();
        const successSpy = vi.spyOn(toastStore, 'success').mockImplementation(() => {});
        const { toastAutoSaved } = await loadModule();
        toastAutoSaved();
        toastAutoSaved();
        expect(successSpy).toHaveBeenCalledTimes(1);
        expect(successSpy).toHaveBeenCalledWith('已自动保存', 1500);
    });

    it('超过2秒后再次调用会再次提示', async () => {
        vi.useFakeTimers();
        const successSpy = vi.spyOn(toastStore, 'success').mockImplementation(() => {});
        const { toastAutoSaved } = await loadModule();
        toastAutoSaved();
        vi.advanceTimersByTime(2100);
        toastAutoSaved();
        expect(successSpy).toHaveBeenCalledTimes(2);
    });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/lib/autoSaveToast.test.ts`
Expected: FAIL（模块不存在，`Cannot find module './autoSaveToast'`）

- [ ] **Step 3: 实现工具函数**

创建 `src/lib/autoSaveToast.ts`：

```ts
import { toastStore } from '$lib/stores/toastStore.svelte';

const DEDUPE_MS = 2000;
let lastToastAt = 0;

/** 自动保存成功提示：全局 2 秒去重，避免连续输入时 toast 刷屏 */
export function toastAutoSaved() {
    const now = Date.now();
    if (now - lastToastAt < DEDUPE_MS) return;
    lastToastAt = now;
    toastStore.success('已自动保存', 1500);
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/lib/autoSaveToast.test.ts`
Expected: PASS（2 passed）

---

### Task 2: 关系设定面板 — 自动保存提示 + saveAll

**Files:**
- Modify: `src/lib/components/AgentRelationshipPanel.svelte`

- [ ] **Step 1: 修改 script 部分**

在 import 区追加：

```ts
import { toastAutoSaved } from '$lib/autoSaveToast';
```

将 `saveRelationship` 改为（加 `isAuto` 参数，成功时提示）：

```ts
async function saveRelationship(item: RelationshipItem, isAuto = true) {
    try {
        await invoke('update_agent_relationship', {
            observerId: agentId,
            targetId: item.target_id,
            targetType: item.target_type,
            relationshipText: item.relationship_text,
        });
        logger.debug('[DEBUG AgentRelationshipPanel] saved', { agentId, targetId: item.target_id });
        if (isAuto) toastAutoSaved();
    } catch (err) {
        logger.error('Failed to save relationship:', err);
        error = '保存失败: ' + String(err);
    }
}
```

在 `handleBlur` 之后追加 `saveAll` 导出方法：

```ts
/** 手动保存：清除所有待执行防抖，立即保存全部关系条目 */
export async function saveAll() {
    for (const key of Object.keys(saveTimeouts)) {
        clearTimeout(saveTimeouts[key]);
    }
    saveTimeouts = {};
    await Promise.all(items.map((item) => saveRelationship(item, false)));
    toastStore.success('保存成功');
}
```

模板部分不改。

- [ ] **Step 2: 类型检查**

Run: `pnpm check`
Expected: 0 errors（a11y warning 可忽略）

---

### Task 3: 记忆面板 — 自动保存提示 + saveAll

**Files:**
- Modify: `src/lib/components/AgentMemoryPanel.svelte`

- [ ] **Step 1: 修改 script 部分**

在 import 区追加：

```ts
import { toastAutoSaved } from '$lib/autoSaveToast';
```

将 `saveLongTermMemory` 和 `saveMemory` 改为：

```ts
async function saveLongTermMemory(isAuto = true) {
    try {
        await invoke('update_agent', { req: { id: agentId, long_term_memory: longTermMemory } });
        if (isAuto) toastAutoSaved();
    } catch (err) {
        logger.error('Failed to save long term memory:', err);
        toastStore.error('保存长期记忆失败');
    }
}

async function saveMemory(item: RelationshipItem, isAuto = true) {
    try {
        await invoke('update_agent_memory', {
            observerId: agentId,
            targetId: item.target_id,
            targetType: item.target_type,
            memoryText: item.memory_text,
        });
        if (isAuto) toastAutoSaved();
    } catch (err) {
        logger.error('Failed to save memory:', err);
        toastStore.error('保存记忆失败');
    }
}
```

在 `handleToggleEnabled` 之后追加：

```ts
/** 手动保存：清除所有待执行防抖，立即保存长期记忆与全部他人记忆 */
export async function saveAll() {
    for (const key of Object.keys(saveTimeouts)) {
        clearTimeout(saveTimeouts[key]);
    }
    saveTimeouts = {};
    await saveLongTermMemory(false);
    await Promise.all(items.map((item) => saveMemory(item, false)));
    toastStore.success('保存成功');
}
```

模板部分不改。

- [ ] **Step 2: 类型检查**

Run: `pnpm check`
Expected: 0 errors

---

### Task 4: 表情包面板 — 自动保存 + 移除内嵌按钮 + saveAll（TDD）

**Files:**
- Modify: `src/lib/components/AgentStickerPackPanel.svelte`
- Test: `src/lib/components/AgentStickerPackPanel.test.ts`

- [ ] **Step 1: 写失败测试**

创建 `src/lib/components/AgentStickerPackPanel.test.ts`：

```ts
import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import AgentStickerPackPanel from './AgentStickerPackPanel.svelte';
import { stickerStore } from '$lib/stores/stickerStore.svelte';
import { toastStore } from '$lib/stores/toastStore.svelte';
import type { StickerPack } from '$lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';

const packs: StickerPack[] = [
    { id: 'p1', name: 'Pack1', stickers: [], createdAt: 0, updatedAt: 0 },
    { id: 'p2', name: 'Pack2', stickers: [], createdAt: 0, updatedAt: 0 },
];

function sleep(ms: number) {
    return new Promise((r) => setTimeout(r, ms));
}

describe('AgentStickerPackPanel', () => {
    const mockInvoke = vi.mocked(invoke);

    beforeEach(() => {
        vi.clearAllMocks();
        stickerStore.packs = [];
        stickerStore.dataDir = '';
        mockInvoke.mockImplementation((cmd: string) => {
            switch (cmd) {
                case 'get_data_dir_cmd': return Promise.resolve('data');
                case 'list_sticker_packs': return Promise.resolve(packs);
                case 'list_agent_sticker_packs': return Promise.resolve([]);
                case 'set_agent_sticker_packs': return Promise.resolve(undefined);
                default: return Promise.resolve(undefined);
            }
        });
    });

    it('勾选表情包后防抖 300ms 自动保存', async () => {
        render(AgentStickerPackPanel, { props: { agentId: 'a1' } });
        await screen.findByText('Pack1');
        await fireEvent.click(screen.getByText('Pack1'));
        expect(mockInvoke).not.toHaveBeenCalledWith('set_agent_sticker_packs', expect.anything());
        await sleep(400);
        expect(mockInvoke).toHaveBeenCalledWith('set_agent_sticker_packs', {
            req: { agentId: 'a1', packIds: ['p1'] },
        });
    });

    it('快速连续勾选合并为一次保存，且自动保存提示去重', async () => {
        const successSpy = vi.spyOn(toastStore, 'success');
        render(AgentStickerPackPanel, { props: { agentId: 'a1' } });
        await screen.findByText('Pack1');
        await fireEvent.click(screen.getByText('Pack1'));
        await sleep(100);
        await fireEvent.click(screen.getByText('Pack2'));
        await sleep(400);
        const saveCalls = mockInvoke.mock.calls.filter((c) => c[0] === 'set_agent_sticker_packs');
        expect(saveCalls).toHaveLength(1);
        expect(saveCalls[0][1]).toEqual({ req: { agentId: 'a1', packIds: ['p1', 'p2'] } });
        const autoSavedToasts = successSpy.mock.calls.filter((c) => c[0] === '已自动保存');
        expect(autoSavedToasts).toHaveLength(1);
        successSpy.mockRestore();
    });

    it('saveAll 立即保存并提示保存成功', async () => {
        const successSpy = vi.spyOn(toastStore, 'success');
        const { component } = render(AgentStickerPackPanel, { props: { agentId: 'a1' } });
        await screen.findByText('Pack1');
        await fireEvent.click(screen.getByText('Pack1'));
        await component.saveAll();
        expect(mockInvoke).toHaveBeenCalledWith('set_agent_sticker_packs', {
            req: { agentId: 'a1', packIds: ['p1'] },
        });
        expect(successSpy).toHaveBeenCalledWith('保存成功');
        successSpy.mockRestore();
    });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/lib/components/AgentStickerPackPanel.test.ts`
Expected: FAIL（自动保存与 saveAll 不存在）

- [ ] **Step 3: 实现自动保存**

将 `src/lib/components/AgentStickerPackPanel.svelte` 的 `<script>` 整体替换为：

```ts
import { invoke } from '@tauri-apps/api/core';
import { stickerStore } from '$lib/stores/stickerStore.svelte';
import { toastStore } from '$lib/stores/toastStore.svelte';
import { toastAutoSaved } from '$lib/autoSaveToast';

interface Props {
    agentId: string;
}

let { agentId }: Props = $props();
let selectedPackIds = $state<Set<string>>(new Set());
let saving = $state(false);
let saveTimeout: ReturnType<typeof setTimeout> | null = null;

async function load() {
    await stickerStore.load();
    const selected = await invoke<string[]>('list_agent_sticker_packs', {
        req: { agentId },
    });
    selectedPackIds = new Set(selected);
}

function togglePack(id: string) {
    const next = new Set(selectedPackIds);
    if (next.has(id)) {
        next.delete(id);
    } else {
        next.add(id);
    }
    selectedPackIds = next;
    scheduleAutoSave();
}

function scheduleAutoSave() {
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
        saveTimeout = null;
        save(true);
    }, 300);
}

async function save(isAuto: boolean) {
    saving = true;
    try {
        await invoke('set_agent_sticker_packs', {
            req: {
                agentId,
                packIds: Array.from(selectedPackIds),
            },
        });
        if (isAuto) {
            toastAutoSaved();
        } else {
            toastStore.success('保存成功');
        }
    } catch (e: any) {
        toastStore.error(e || '保存失败');
    } finally {
        saving = false;
    }
}

/** 手动保存：取消防抖，立即保存当前勾选集合 */
export async function saveAll() {
    if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
    }
    await save(false);
}

$effect(() => {
    load();
});
```

模板部分：删除文件末尾的保存按钮块（`onclick={save}` 的那个 `<button>`），保留表情包网格。

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/lib/components/AgentStickerPackPanel.test.ts`
Expected: PASS（3 passed）

---

### Task 5: 语音面板 — 自动保存 + 按钮外移 + saveAll/deleteConfig（TDD）

**Files:**
- Modify: `src/lib/components/AgentVoicePanel.svelte`
- Test: `src/lib/components/AgentVoicePanel.test.ts`

- [ ] **Step 1: 写失败测试**

创建 `src/lib/components/AgentVoicePanel.test.ts`：

```ts
import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import AgentVoicePanel from './AgentVoicePanel.svelte';
import { voiceStore } from '$lib/stores/voiceStore.svelte';
import { toastStore } from '$lib/stores/toastStore.svelte';
import type { Agent } from '$lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';

const agent = { id: 'a1' } as Agent;

const model = { name: 'm1', path: '/models/m1', language: 'ja', speakers: [], has_config: true };

const savedVoice = {
    id: 'v1',
    agent_id: 'a1',
    model_name: 'm1',
    model_path: '/models/m1',
    speaker_id: null,
    target_language: 'ja',
    emotion_params: null,
    speed: 1.0,
    translate_enabled: true,
    translate_model_config_id: null,
    generation_mode: 'auto_silent',
    created_at: 0,
    updated_at: 0,
};

function sleep(ms: number) {
    return new Promise((r) => setTimeout(r, ms));
}

describe('AgentVoicePanel', () => {
    const mockInvoke = vi.mocked(invoke);

    beforeEach(() => {
        vi.clearAllMocks();
        voiceStore.agentVoices = new Map();
        mockInvoke.mockImplementation((cmd: string) => {
            switch (cmd) {
                case 'check_vits_runtime': return Promise.resolve(true);
                case 'scan_vits_models': return Promise.resolve([model]);
                case 'get_agent_voice': return Promise.resolve(null);
                case 'save_agent_voice': return Promise.resolve(savedVoice);
                default: return Promise.resolve(undefined);
            }
        });
    });

    it('选择模型后立即自动保存', async () => {
        const { container } = render(AgentVoicePanel, { props: { agent } });
        const select = await waitFor(() => {
            const el = container.querySelector('#voice-model');
            if (!el) throw new Error('not rendered');
            return el;
        });
        await fireEvent.change(select, { target: { value: 'm1' } });
        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('save_agent_voice', {
                req: expect.objectContaining({ agent_id: 'a1', model_name: 'm1' }),
            });
        });
    });

    it('情感参数文本框失焦后自动保存', async () => {
        const { container } = render(AgentVoicePanel, { props: { agent } });
        const select = await waitFor(() => {
            const el = container.querySelector('#voice-model');
            if (!el) throw new Error('not rendered');
            return el;
        });
        await fireEvent.change(select, { target: { value: 'm1' } });
        const emotion = container.querySelector('#voice-emotion') as HTMLInputElement;
        await fireEvent.input(emotion, { target: { value: 'happy' } });
        await fireEvent.blur(emotion);
        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('save_agent_voice', {
                req: expect.objectContaining({ emotion_params: 'happy' }),
            });
        });
    });

    it('未选择模型时自动保存跳过并提示', async () => {
        const infoSpy = vi.spyOn(toastStore, 'info').mockImplementation(() => {});
        const { container } = render(AgentVoicePanel, { props: { agent } });
        const lang = await waitFor(() => {
            const el = container.querySelector('#voice-target-lang');
            if (!el) throw new Error('not rendered');
            return el;
        });
        await fireEvent.change(lang, { target: { value: 'zh' } });
        await sleep(50);
        expect(mockInvoke).not.toHaveBeenCalledWith('save_agent_voice', expect.anything());
        expect(infoSpy).toHaveBeenCalled();
        infoSpy.mockRestore();
    });

    it('saveAll 未选模型时提示错误', async () => {
        const errorSpy = vi.spyOn(toastStore, 'error').mockImplementation(() => {});
        const { component } = render(AgentVoicePanel, { props: { agent } });
        await waitFor(() => expect(voiceStore.runtimeAvailable).toBe(true));
        await component.saveAll();
        expect(mockInvoke).not.toHaveBeenCalledWith('save_agent_voice', expect.anything());
        expect(errorSpy).toHaveBeenCalledWith('请先选择语音模型');
        errorSpy.mockRestore();
    });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/lib/components/AgentVoicePanel.test.ts`
Expected: FAIL（自动保存未接入，saveAll 不存在）

- [ ] **Step 3: 实现自动保存与导出方法**

`src/lib/components/AgentVoicePanel.svelte` 的 `<script>` 改动：

import 区追加：

```ts
import { toastAutoSaved } from '$lib/autoSaveToast';
```

props 解构改为（`hasExisting` 变为 bindable prop）：

```ts
let { agent, hasExisting = $bindable(false) }: { agent: Agent; hasExisting?: boolean } = $props();
```

删除原来的 `let hasExisting = $state(false);`。

将 `handleSave` 重构为 `persist` + `autoSave` + 导出方法：

```ts
async function persist(isAuto: boolean) {
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
        if (isAuto) {
            toastAutoSaved();
        } else {
            toastStore.success('语音配置已保存');
        }
    } catch (e) {
        toastStore.error('保存失败: ' + e);
    } finally {
        saving = false;
    }
}

/** 控件变更触发的自动保存；未选模型时跳过 */
function autoSave() {
    if (!form.model_name) {
        toastStore.info('请先选择语音模型，当前修改未保存', 3000);
        return;
    }
    persist(true);
}

/** 手动保存（footer 保存按钮） */
export async function saveAll() {
    if (!form.model_name) {
        toastStore.error('请先选择语音模型');
        return;
    }
    await persist(false);
}

/** 删除配置（footer 删除按钮） */
export async function deleteConfig() {
    await handleDelete();
}
```

模板改动（全部在 `{#if !voiceStore.runtimeAvailable}` 的 `{:else}` 分支内）：

1. 模型 select：`onchange={handleModelChange}` 改为 `onchange={() => { handleModelChange(); autoSave(); }}`
2. 说话人 select：追加 `onchange={autoSave}`
3. 输出语言 select：追加 `onchange={autoSave}`
4. 语速 range input：追加 `onchange={autoSave}`（range 的 change 在松手时触发）
5. 情感参数 input：追加 `onblur={autoSave}`
6. 自动翻译 checkbox：追加 `onchange={autoSave}`
7. 翻译模型 select：追加 `onchange={autoSave}`
8. 三个生成时机 radio：各追加 `onchange={autoSave}`
9. "操作"行：删除保存按钮与删除配置按钮，整行改为只保留缓存开关：

```svelte
<!-- 操作 -->
<div class="flex items-center gap-3 pt-2 border-t border-border">
    <button onclick={() => (showCache = !showCache)} class="px-4 py-1.5 text-sm text-text-secondary hover:bg-bg rounded-lg transition-colors">
        {showCache ? '隐藏语音缓存' : '查看语音缓存'}
    </button>
</div>
```

`saving` 声明保留（`persist` 使用）；`handleDelete` 保留原名（`deleteConfig` 包装它）。

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/lib/components/AgentVoicePanel.test.ts`
Expected: PASS（4 passed）

---

### Task 6: AgentDetail 统一 footer

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`

- [ ] **Step 1: 添加面板引用与状态**

script 中 `let activeTab = ...` 之后追加：

```ts
// 子面板引用（bind:this），footer 保存按钮通过它们触发手动保存
type SaveablePanel = { saveAll: () => Promise<void> };
type VoicePanelRef = SaveablePanel & { deleteConfig: () => Promise<void> };
let relationshipPanel: SaveablePanel | undefined;
let memoryPanel: SaveablePanel | undefined;
let stickerPanel: SaveablePanel | undefined;
let voicePanel: VoicePanelRef | undefined;
let voiceHasExisting = $state(false);

async function handleFooterSave() {
    if (activeTab === 'config') {
        await handleSave();
    } else if (activeTab === 'relationships') {
        await relationshipPanel?.saveAll();
    } else if (activeTab === 'memory') {
        await memoryPanel?.saveAll();
    } else if (activeTab === 'stickers') {
        await stickerPanel?.saveAll();
    } else if (activeTab === 'voice') {
        await voicePanel?.saveAll();
    }
}
```

- [ ] **Step 2: 子面板挂接 bind:this 与 bind:hasExisting**

内容区四个面板改为：

```svelte
{:else if activeTab === 'relationships'}
    <AgentRelationshipPanel agentId={agent.id} bind:this={relationshipPanel} />
{:else if activeTab === 'memory'}
    <AgentMemoryPanel
        agentId={agent.id}
        bind:longTermMemory={form.long_term_memory}
        bind:memoryEnabled={form.memory_enabled}
        bind:this={memoryPanel}
    />
{:else if activeTab === 'timer'}
    <AgentTimerPanel agentId={agent.id} />
{:else if activeTab === 'stickers' && agent}
    <AgentStickerPackPanel agentId={agent.id} bind:this={stickerPanel} />
{:else if activeTab === 'voice' && agent}
    <AgentVoicePanel {agent} bind:this={voicePanel} bind:hasExisting={voiceHasExisting} />
{/if}
```

- [ ] **Step 3: 重写 footer**

将 `<!-- Footer actions -->` 整个 div 替换为：

```svelte
<!-- Footer actions（固定不滚动） -->
<div class="px-6 py-4 border-t border-border bg-surface flex justify-between items-center">
    {#if activeTab === 'config'}
        <button
            onclick={() => showGenerateModal = true}
            class="flex items-center gap-2 px-4 py-2 text-primary hover:bg-primary/5 rounded-lg transition-colors"
        >
            <Sparkles size={16} />
            <span>人设自生成</span>
        </button>
    {:else if activeTab === 'voice' && voiceHasExisting}
        <button
            onclick={() => voicePanel?.deleteConfig()}
            class="flex items-center gap-2 px-4 py-2 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
        >
            <Trash2 size={16} />
            <span>删除配置</span>
        </button>
    {:else}
        <div></div>
    {/if}
    <div class="flex gap-3">
        <button onclick={() => appState.selectAgent(null)} class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">
            取消
        </button>
        {#if activeTab !== 'timer'}
            <button onclick={handleFooterSave} disabled={saving}
                class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary">
                {#if saving}
                    <Loader2 size={16} class="animate-spin" />
                    <span>保存中...</span>
                {:else}
                    <Save size={16} />
                    <span>保存</span>
                {/if}
            </button>
        {/if}
    </div>
</div>
```

（`Trash2`、`Sparkles`、`Save`、`Loader2` 均已在现有 import 中。）

- [ ] **Step 4: 类型检查**

Run: `pnpm check`
Expected: 0 errors

---

### Task 7: 全量验证

**Files:** 无改动

- [ ] **Step 1: 全部前端测试**

Run: `pnpm test`
Expected: 全部通过（含既有测试）

- [ ] **Step 2: 类型检查**

Run: `pnpm check`
Expected: 0 errors

- [ ] **Step 3: 构建**

Run: `pnpm build`
Expected: 构建成功

- [ ] **Step 4: 手动冒烟（pnpm tauri dev）**

逐项验证：关系设定/记忆输入后约 0.5s 顶部弹出"已自动保存"；表情包勾选 300ms 后保存；语音各控件变更即保存、情感参数失焦保存、未选模型提示；四个 tab 右下角保存按钮可用；语音已配置时 footer 左侧出现"删除配置"；footer 固定不随内容滚动。

---

## Self-Review 记录

- Spec 覆盖：footer 统一（Task 6）、关系（Task 2）、记忆（Task 3）、表情包（Task 4）、语音（Task 5）、toast 去重（Task 1）、定时任务不改（符合 spec）、测试（Task 1/4/5 + Task 7）。
- 无占位符；`saveAll`/`deleteConfig`/`toastAutoSaved`/`persist`/`autoSave` 命名在任务间一致。
- 已知风险：Task 5 测试对 `@testing-library/svelte` v5 的 `render` 返回 `component` 有依赖，若版本行为不符，执行时按实际报错调整（备用：通过 DOM + invoke 断言替代）。
