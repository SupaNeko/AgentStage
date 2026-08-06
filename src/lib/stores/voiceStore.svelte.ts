import { invoke } from '@tauri-apps/api/core';
import type {
    VitsModelInfo,
    AgentVoice,
    VoiceCacheItem,
    SaveAgentVoiceRequest,
    GenerateVoiceRequest,
    Message,
} from '$lib/types';
import { resolveLocalFileUrl } from '$lib/utils';

class VoiceStore {
    runtimeAvailable = $state<boolean>(false);
    models = $state<VitsModelInfo[]>([]);
    agentVoices = $state<Map<string, AgentVoice>>(new Map());
    generating = $state<Set<string>>(new Set());
    playingMessageId = $state<string | null>(null);

    private audio: HTMLAudioElement | null = null;

    async checkRuntime() {
        this.runtimeAvailable = await invoke<boolean>('check_vits_runtime');
    }

    async scanModels() {
        this.models = await invoke<VitsModelInfo[]>('scan_vits_models');
    }

    async loadAgentVoice(agentId: string) {
        const voice = await invoke<AgentVoice | null>('get_agent_voice', { agentId });
        const next = new Map(this.agentVoices);
        if (voice) {
            next.set(agentId, voice);
        } else {
            next.delete(agentId);
        }
        this.agentVoices = next;
    }

    async saveAgentVoice(req: SaveAgentVoiceRequest) {
        const voice = await invoke<AgentVoice>('save_agent_voice', { req });
        const next = new Map(this.agentVoices);
        next.set(req.agent_id, voice);
        this.agentVoices = next;
    }

    async deleteAgentVoice(agentId: string) {
        await invoke('delete_agent_voice', { agentId });
        const next = new Map(this.agentVoices);
        next.delete(agentId);
        this.agentVoices = next;
    }

    async generateVoice(req: GenerateVoiceRequest): Promise<string> {
        const next = new Set(this.generating);
        next.add(req.message_id);
        this.generating = next;
        try {
            return await invoke<string>('generate_voice', { req });
        } finally {
            const done = new Set(this.generating);
            done.delete(req.message_id);
            this.generating = done;
        }
    }

    /** 播放指定路径的语音文件 */
    playFile(path: string, messageId: string | null = null) {
        this.stopPlayback();
        const audio = new Audio(resolveLocalFileUrl(path));
        this.audio = audio;
        this.playingMessageId = messageId;
        audio.onended = () => {
            if (this.audio === audio) {
                this.audio = null;
                this.playingMessageId = null;
            }
        };
        audio.onerror = () => {
            if (this.audio === audio) {
                this.audio = null;
                this.playingMessageId = null;
            }
        };
        void audio.play().catch(() => {
            if (this.audio === audio) {
                this.audio = null;
                this.playingMessageId = null;
            }
        });
    }

    stopPlayback() {
        if (this.audio) {
            this.audio.pause();
            this.audio = null;
        }
        this.playingMessageId = null;
    }

    /** 点击喇叭：生成（或命中缓存）并播放 */
    async speakMessage(msg: Message): Promise<void> {
        if (this.generating.has(msg.id)) return;
        const path = await this.generateVoice({
            message_id: msg.id,
            session_id: msg.session_id,
            agent_id: msg.sender_id,
            text: msg.content,
        });
        this.playFile(path, msg.id);
    }

    /**
     * 新 agent 消息到达时的自动生成入口。
     * auto_play: 后台生成完成后自动播放；auto_silent: 仅后台预生成；manual: 不处理。
     */
    async handleAgentMessage(msg: Message): Promise<void> {
        let voice = this.agentVoices.get(msg.sender_id);
        if (voice === undefined) {
            try {
                await this.loadAgentVoice(msg.sender_id);
            } catch {
                return;
            }
            voice = this.agentVoices.get(msg.sender_id);
        }
        if (!voice || voice.generation_mode === 'manual') return;
        try {
            const path = await this.generateVoice({
                message_id: msg.id,
                session_id: msg.session_id,
                agent_id: msg.sender_id,
                text: msg.content,
            });
            if (voice.generation_mode === 'auto_play') {
                this.playFile(path, msg.id);
            }
        } catch (e) {
            // 后台生成失败不打断聊天，仅记录日志
            console.warn('[Voice] auto generation failed:', e);
        }
    }

    async listCache(agentId?: string): Promise<VoiceCacheItem[]> {
        return await invoke<VoiceCacheItem[]>('list_voice_cache', { agentId: agentId ?? null });
    }

    async deleteCache(id: string) {
        await invoke('delete_voice_cache', { id });
    }

    async clearCache(sessionId?: string) {
        await invoke('clear_voice_cache', { sessionId: sessionId ?? null });
    }
}

export const voiceStore = new VoiceStore();
