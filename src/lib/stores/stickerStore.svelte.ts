import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { ResolvedSticker, StickerPack } from '$lib/types';

class StickerStore {
    packs = $state<StickerPack[]>([]);
    loading = $state(false);
    dataDir = $state<string>('');
    private resolved = $state<Map<string, ResolvedSticker>>(new Map());

    async load() {
        this.loading = true;
        try {
            if (!this.dataDir) {
                this.dataDir = await invoke<string>('get_data_dir_cmd');
            }
            this.packs = await invoke<StickerPack[]>('list_sticker_packs');
            const next = new Map<string, ResolvedSticker>();
            for (const pack of this.packs) {
                for (const sticker of pack.stickers) {
                    const reference = `${pack.name}_${sticker.name}`;
                    next.set(reference, {
                        reference,
                        status: 'valid',
                        packId: pack.id,
                        stickerId: sticker.id,
                        filePath: sticker.filePath,
                        mimeType: sticker.mimeType,
                        width: sticker.width,
                        height: sticker.height,
                    });
                }
            }
            this.resolved = next;
        } finally {
            this.loading = false;
        }
    }

    resolve(reference: string): ResolvedSticker | null {
        return this.resolved.get(reference) ?? null;
    }

    async resolveMissing(references: string[]) {
        const missing = references.filter((ref) => !this.resolved.has(ref));
        if (missing.length === 0) return;
        const results = await invoke<ResolvedSticker[]>('resolve_sticker_refs', {
            req: { refs: missing },
        });
        const next = new Map(this.resolved);
        for (const result of results) {
            next.set(result.reference, result);
        }
        this.resolved = next;
    }

    imageUrl(filePath: string): string {
        if (!filePath) return '';
        const normalizedPath = filePath.replace(/\\/g, '/');
        if (normalizedPath.startsWith('http') || normalizedPath.startsWith('asset:') || normalizedPath.startsWith('data:')) {
            return normalizedPath;
        }

        const absolutePath = normalizedPath.startsWith('/')
            ? normalizedPath
            : `${this.dataDir.replace(/\\/g, '/')}/${normalizedPath}`;

        if (import.meta.env.DEV) {
            return `http://${window.location.host}/@fs/${absolutePath}`;
        }
        try {
            return convertFileSrc(absolutePath);
        } catch (e) {
            console.warn('[Sticker] convertFileSrc failed:', filePath, e);
            return '';
        }
    }
}

export const stickerStore = new StickerStore();
