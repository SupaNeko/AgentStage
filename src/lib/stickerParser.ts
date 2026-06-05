export type StickerContentPart =
    | { type: 'text'; text: string }
    | { type: 'sticker'; reference: string };

const STICKER_RE = /<sticker>([^<]+)<\/sticker>/g;

export function parseStickerContent(content: string): StickerContentPart[] {
    const parts: StickerContentPart[] = [];
    let lastIndex = 0;
    for (const match of content.matchAll(STICKER_RE)) {
        const start = match.index ?? 0;
        if (start > lastIndex) {
            parts.push({ type: 'text', text: content.slice(lastIndex, start) });
        }
        const ref = match[1].trim();
        const pieces = ref.split('_');
        if (pieces.length === 2 && pieces[0] && pieces[1]) {
            parts.push({ type: 'sticker', reference: ref });
        } else {
            parts.push({ type: 'text', text: match[0] });
        }
        lastIndex = start + match[0].length;
    }
    if (lastIndex < content.length) {
        parts.push({ type: 'text', text: content.slice(lastIndex) });
    }
    return parts.length > 0 ? parts : [{ type: 'text', text: content }];
}
