import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
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
    let now: number;

    beforeEach(() => {
        vi.clearAllMocks();
        // toastAutoSaved 按 Date.now 全局 2s 去重；让每次调用时间前进 3s，隔离跨测试的去重状态
        now = 1_000_000;
        vi.spyOn(Date, 'now').mockImplementation(() => (now += 3000));
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

    afterEach(() => {
        vi.restoreAllMocks();
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
