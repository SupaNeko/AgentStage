export interface ProviderDefaultConfig {
    baseUrl: string;
    modelName: string;
}

export const PROVIDER_DEFAULTS: Record<string, ProviderDefaultConfig> = {
    deepseek: {
        baseUrl: 'https://api.deepseek.com',
        modelName: 'DeepSeek V4 Flash',
    },
    minimax: {
        baseUrl: 'https://api.minimax.chat/v1',
        modelName: 'MiniMax-M2.7',
    },
    custom: {
        baseUrl: '',
        modelName: '',
    },
};

export function getProviderDefaults(provider: string): ProviderDefaultConfig {
    return PROVIDER_DEFAULTS[provider] ?? PROVIDER_DEFAULTS.custom;
}
