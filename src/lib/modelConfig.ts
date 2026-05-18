export interface ProviderDefaultConfig {
    baseUrl: string;
    modelName: string;
}

export const PROVIDER_DEFAULTS: Record<string, ProviderDefaultConfig> = {
    openai: {
        baseUrl: 'https://api.openai.com/v1',
        modelName: 'gpt-4o',
    },
    anthropic: {
        baseUrl: 'https://api.anthropic.com/v1',
        modelName: 'claude-3-5-sonnet-20241022',
    },
    google: {
        baseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai',
        modelName: 'gemini-2.0-flash',
    },
    kimi: {
        baseUrl: 'https://api.moonshot.cn/v1',
        modelName: 'kimi-k2',
    },
    minimax: {
        baseUrl: 'https://api.minimax.chat/v1',
        modelName: 'abab6.5-chat',
    },
    custom: {
        baseUrl: '',
        modelName: '',
    },
};

export function getProviderDefaults(provider: string): ProviderDefaultConfig {
    return PROVIDER_DEFAULTS[provider] ?? PROVIDER_DEFAULTS.custom;
}
