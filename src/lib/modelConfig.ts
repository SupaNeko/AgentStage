export interface ProviderDefaultConfig {
    baseUrl: string;
    modelName: string;
}

export const PROVIDER_DEFAULTS: Record<string, ProviderDefaultConfig> = {
    deepseek: {
        baseUrl: 'https://api.deepseek.com',
        modelName: 'deepseek-chat',
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
