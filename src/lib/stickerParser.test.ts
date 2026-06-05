import { describe, expect, it } from 'vitest';
import { parseStickerContent } from './stickerParser';

describe('parseStickerContent', () => {
    it('parses text and sticker tags', () => {
        expect(parseStickerContent('早上好<sticker>猫_可爱</sticker>')).toEqual([
            { type: 'text', text: '早上好' },
            { type: 'sticker', reference: '猫_可爱' },
        ]);
    });

    it('leaves malformed tags as text', () => {
        expect(parseStickerContent('<sticker>猫</sticker>')).toEqual([
            { type: 'text', text: '<sticker>猫</sticker>' },
        ]);
    });

    it('returns plain text when no stickers', () => {
        expect(parseStickerContent('Hello world')).toEqual([
            { type: 'text', text: 'Hello world' },
        ]);
    });

    it('parses multiple stickers', () => {
        expect(parseStickerContent('a<sticker>猫_可爱</sticker>b<sticker>狗_大笑</sticker>c')).toEqual([
            { type: 'text', text: 'a' },
            { type: 'sticker', reference: '猫_可爱' },
            { type: 'text', text: 'b' },
            { type: 'sticker', reference: '狗_大笑' },
            { type: 'text', text: 'c' },
        ]);
    });
});
