<script lang="ts">
    import { stickerStore } from '$lib/stores/stickerStore.svelte';

    interface Props {
        value?: string;
        onChange?: (value: string) => void;
        onKeyDown?: (e: KeyboardEvent) => void;
        onFocus?: () => void;
        placeholder?: string;
    }

    let {
        value = $bindable(''),
        onChange,
        onKeyDown,
        onFocus,
        placeholder = '',
    }: Props = $props();

    let editorEl: HTMLDivElement;
    let isUpdating = false;

    function escapeHtml(text: string): string {
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }

    function renderToDom(text: string) {
        if (!editorEl) return;
        if (!text) {
            editorEl.innerHTML = '';
            return;
        }

        const regex = /<sticker>([^<]+)<\/sticker>/g;
        let lastIndex = 0;
        const parts: string[] = [];

        let match;
        while ((match = regex.exec(text)) !== null) {
            const before = text.slice(lastIndex, match.index);
            if (before) {
                parts.push(escapeHtml(before).replace(/\n/g, '<br>'));
            }

            const ref = match[1];
            const resolved = stickerStore.resolve(ref);
            if (resolved && resolved.status === 'valid' && resolved.filePath) {
                parts.push(
                    `<img src="${stickerStore.imageUrl(resolved.filePath)}" alt="${ref}" contenteditable="false" data-sticker-ref="${ref}" class="inline-block h-5 align-middle mx-0.5 select-none" draggable="false" />`
                );
            } else {
                parts.push(
                    `<span class="text-text-secondary text-xs bg-bg px-1 rounded border border-border select-none">[失效表情]</span>`
                );
            }

            lastIndex = regex.lastIndex;
        }

        const after = text.slice(lastIndex);
        if (after) {
            parts.push(escapeHtml(after).replace(/\n/g, '<br>'));
        }

        editorEl.innerHTML = parts.join('');
    }

    function extractFromDom(): string {
        if (!editorEl) return '';
        const parts: string[] = [];

        function walk(node: Node) {
            if (node.nodeType === Node.TEXT_NODE) {
                parts.push(node.textContent || '');
            } else if (node.nodeType === Node.ELEMENT_NODE) {
                const el = node as HTMLElement;
                if (el.tagName === 'IMG' && el.dataset.stickerRef) {
                    parts.push(`<sticker>${el.dataset.stickerRef}</sticker>`);
                } else if (el.tagName === 'BR') {
                    parts.push('\n');
                } else if (el.tagName === 'DIV') {
                    // contenteditable 中 Enter 可能产生 <div>
                    if (parts.length > 0 && !parts[parts.length - 1].endsWith('\n')) {
                        parts.push('\n');
                    }
                    el.childNodes.forEach(walk);
                } else {
                    el.childNodes.forEach(walk);
                }
            }
        }

        editorEl.childNodes.forEach(walk);
        return parts.join('');
    }

    // 当外部 value 变化时同步到 DOM
    $effect(() => {
        if (!editorEl || isUpdating) return;
        const current = extractFromDom();
        if (current !== value) {
            isUpdating = true;
            renderToDom(value);
            isUpdating = false;
        }
    });

    function handleInput() {
        if (!editorEl || isUpdating) return;
        const text = extractFromDom();
        if (text !== value) {
            onChange?.(text);
        }
    }

    function handlePaste(e: ClipboardEvent) {
        e.preventDefault();
        const text = e.clipboardData?.getData('text/plain') || '';
        document.execCommand('insertText', false, text);
    }

    export function insertSticker(ref: string) {
        if (!editorEl) return;
        // 支持传入完整的 <sticker>...</sticker> 标签或纯 ref
        const match = ref.match(/^<sticker>(.+)<\/sticker>$/);
        const stickerRef = match ? match[1] : ref;

        editorEl.focus();

        const selection = window.getSelection();
        if (!selection || selection.rangeCount === 0) {
            // 没有选区，追加到末尾
            const newValue = value + `<sticker>${stickerRef}</sticker>`;
            isUpdating = true;
            renderToDom(newValue);
            isUpdating = false;
            onChange?.(newValue);
            return;
        }

        const range = selection.getRangeAt(0);
        const resolved = stickerStore.resolve(stickerRef);

        const img = document.createElement('img');
        if (resolved && resolved.status === 'valid' && resolved.filePath) {
            img.src = stickerStore.imageUrl(resolved.filePath);
        }
        img.alt = stickerRef;
        img.dataset.stickerRef = stickerRef;
        img.contentEditable = 'false';
        img.className = 'inline-block h-5 align-middle mx-0.5 select-none';
        img.draggable = false;

        range.deleteContents();
        range.insertNode(img);

        // 移动光标到 img 后面
        const newRange = document.createRange();
        newRange.setStartAfter(img);
        newRange.setEndAfter(img);
        selection.removeAllRanges();
        selection.addRange(newRange);

        handleInput();
    }

    const isEmpty = $derived(!value);
</script>

<div class="flex-1 relative min-h-0">
    {#if isEmpty}
        <span class="absolute left-4 top-2.5 text-sm text-text-secondary pointer-events-none select-none">
            {placeholder}
        </span>
    {/if}
    <div
        bind:this={editorEl}
        contenteditable="true"
        oninput={handleInput}
        onkeydown={onKeyDown}
        onfocus={onFocus}
        onpaste={handlePaste}
        class="w-full min-h-[4.5rem] max-h-32 overflow-y-auto px-4 py-2.5 bg-bg border border-border rounded-xl focus:outline-none focus:ring-2 focus:ring-primary/20 text-sm leading-normal"
    ></div>
</div>
