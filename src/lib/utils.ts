import { convertFileSrc } from '@tauri-apps/api/core';

export function formatTime(ts: number): string {
    const date = new Date(ts);
    const now = new Date();
    if (date.toDateString() === now.toDateString()) {
        return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
    }
    return `${date.getMonth() + 1}/${date.getDate()}`;
}

export function resolveAvatarUrl(path: string | null | undefined): string {
    if (!path) return '';
    // 如果已经是 URL 格式，直接返回
    if (path.startsWith('http') || path.startsWith('asset:') || path.startsWith('data:')) {
        return path;
    }
    const normalizedPath = path.replace(/\\/g, '/');

    // Development: Vite dev server can serve local files via @fs/
    // (Tauri convertFileSrc uses asset.localhost which has no listener in dev mode)
    if (import.meta.env.DEV) {
        const url = `http://${window.location.host}/@fs/${normalizedPath}`;
        console.log('[Avatar] resolve (dev):', path, '->', url);
        return url;
    }

    // Production: Tauri asset protocol
    try {
        const url = convertFileSrc(normalizedPath);
        console.log('[Avatar] resolve (prod):', path, '->', url);
        return url;
    } catch (e) {
        console.warn('[Avatar] convertFileSrc failed:', path, e);
        return path;
    }
}
