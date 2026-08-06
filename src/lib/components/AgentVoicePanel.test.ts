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

    async function renderAndWait() {
        const utils = render(AgentVoicePanel, { props: { agent } });
        const select = await waitFor(() => {
            const el = utils.container.querySelector('#voice-model');
            if (!el) throw new Error('not rendered');
            return el;
        });
        return { ...utils, select };
    }

    it('选择模型后立即自动保存', async () => {
        const { select } = await renderAndWait();
        await fireEvent.change(select, { target: { value: 'm1' } });
        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('save_agent_voice', {
                req: expect.objectContaining({ agent_id: 'a1', model_name: 'm1' }),
            });
        });
    });

    it('情感参数文本框失焦后自动保存', async () => {
        const { container, select } = await renderAndWait();
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
        const { container } = await renderAndWait();
        const lang = container.querySelector('#voice-target-lang') as HTMLSelectElement;
        await fireEvent.change(lang, { target: { value: 'zh' } });
        await sleep(50);
        expect(mockInvoke).not.toHaveBeenCalledWith('save_agent_voice', expect.anything());
        expect(infoSpy).toHaveBeenCalled();
        infoSpy.mockRestore();
    });

    it('saveAll 未选模型时提示错误', async () => {
        const errorSpy = vi.spyOn(toastStore, 'error').mockImplementation(() => {});
        const { component } = await renderAndWait();
        await component.saveAll();
        expect(mockInvoke).not.toHaveBeenCalledWith('save_agent_voice', expect.anything());
        expect(errorSpy).toHaveBeenCalledWith('请先选择语音模型');
        errorSpy.mockRestore();
    });
});
