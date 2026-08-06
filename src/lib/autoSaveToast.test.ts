import { describe, it, expect, afterEach, vi } from 'vitest';

// resetModules 后动态导入，保证 autoSaveToast 与被 spy 的 toastStore 是同一模块实例
async function loadModules() {
    vi.resetModules();
    const toastModule = await import('$lib/stores/toastStore.svelte');
    const autoModule = await import('./autoSaveToast');
    return { toastStore: toastModule.toastStore, toastAutoSaved: autoModule.toastAutoSaved };
}

describe('toastAutoSaved', () => {
    afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
    });

    it('2秒内重复调用只提示一次', async () => {
        vi.useFakeTimers();
        const { toastStore, toastAutoSaved } = await loadModules();
        const successSpy = vi.spyOn(toastStore, 'success').mockImplementation(() => {});
        toastAutoSaved();
        toastAutoSaved();
        expect(successSpy).toHaveBeenCalledTimes(1);
        expect(successSpy).toHaveBeenCalledWith('已自动保存', 1500);
    });

    it('超过2秒后再次调用会再次提示', async () => {
        vi.useFakeTimers();
        const { toastStore, toastAutoSaved } = await loadModules();
        const successSpy = vi.spyOn(toastStore, 'success').mockImplementation(() => {});
        toastAutoSaved();
        vi.advanceTimersByTime(2100);
        toastAutoSaved();
        expect(successSpy).toHaveBeenCalledTimes(2);
    });
});
