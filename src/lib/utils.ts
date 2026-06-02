import { convertFileSrc } from '@tauri-apps/api/core';

export function formatTime(ts: number): string {
    const date = new Date(ts);
    const now = new Date();
    if (date.toDateString() === now.toDateString()) {
        return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
    }
    return `${date.getMonth() + 1}/${date.getDate()}`;
}

const avatarVersion = new Map<string, number>();

export function bumpAvatarVersion(path: string | null | undefined) {
    if (!path) return;
    const normalizedPath = path.replace(/\\/g, '/');
    const current = avatarVersion.get(normalizedPath) ?? 0;
    avatarVersion.set(normalizedPath, current + 1);
}

export function resolveAvatarUrl(path: string | null | undefined): string {
    if (!path) return '';
    if (path.startsWith('http') || path.startsWith('asset:') || path.startsWith('data:')) {
        return path;
    }
    const normalizedPath = path.replace(/\\/g, '/');

    const version = avatarVersion.get(normalizedPath);
    const cacheBust = version ? `?v=${version}` : '';

    if (import.meta.env.DEV) {
        const url = `http://${window.location.host}/@fs/${normalizedPath}${cacheBust}`;
        return url;
    }

    try {
        const url = convertFileSrc(normalizedPath) + cacheBust;
        return url;
    } catch (e) {
        console.warn('[Avatar] convertFileSrc failed:', path, e);
        return path;
    }
}
