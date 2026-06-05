<script lang="ts">
    import type { Message } from '$lib/types';
    import { formatTime, resolveAvatarUrl } from '$lib/utils';
    import { User, Bot } from 'lucide-svelte';
    import { parseStickerContent } from '$lib/stickerParser';
    import { stickerStore } from '$lib/stores/stickerStore.svelte';

    // 自动加载 sticker 数据
    $effect(() => {
        if (stickerStore.packs.length === 0 && !stickerStore.loading) {
            stickerStore.load();
        }
    });

    interface Props {
        message: Message;
        isMe: boolean;
        senderName: string;
        snapshotName?: string;
        snapshotAvatar?: string | null;
    }

    let { message, isMe, senderName, snapshotName, snapshotAvatar }: Props = $props();

    const displayName = snapshotName ?? senderName ?? '未知角色';
    const displayAvatar = snapshotAvatar ?? message.sender_avatar;
    const contentParts = $derived(parseStickerContent(message.content));
</script>

<div class="flex flex-col max-w-[80%] {isMe ? 'items-end' : 'items-start'}">
    <!-- 头像 + 名称/时间 -->
    <div class="flex items-center gap-2 mb-1 {isMe ? 'flex-row-reverse' : ''}">
        <div class="w-8 h-8 rounded-full flex items-center justify-center shrink-0 overflow-hidden {message.sender_type === 'user' ? 'bg-gray-300 text-white' : 'bg-primary/10 text-primary'}">
            {#if displayAvatar}
                <img src={resolveAvatarUrl(displayAvatar)} alt={displayName} class="w-full h-full object-cover" />
            {:else if message.sender_type === 'user'}
                <User size={16} />
            {:else}
                <Bot size={16} />
            {/if}
        </div>
        <div class="flex flex-col justify-center h-8 {isMe ? 'items-end' : 'items-start'}">
            <span class="text-xs text-text-secondary leading-none">{displayName}</span>
            <span class="text-[10px] text-text-secondary opacity-70 leading-none mt-0.5">{formatTime(message.created_at)}</span>
        </div>
    </div>

    <!-- 聊天气泡 -->
    <div
        class="msg-bubble {isMe
            ? 'msg-self bg-primary text-white rounded-2xl rounded-tr-sm'
            : 'msg-other bg-surface border border-border rounded-2xl rounded-tl-sm'} px-4 py-2 min-w-[80px]"
        style="--sender-avatar: {displayAvatar ? `url(${resolveAvatarUrl(displayAvatar)})` : 'none'}"
    >
        {#each contentParts as part}
            {#if part.type === 'text'}
                <span>{part.text}</span>
            {:else}
                {@const resolved = stickerStore.resolve(part.reference)}
                {#if resolved && resolved.status === 'valid' && resolved.filePath}
                    <img
                        src={stickerStore.imageUrl(resolved.filePath)}
                        alt={part.reference}
                        class="block max-w-32 max-h-32 my-1"
                    />
                {:else if stickerStore.packs.length === 0}
                    <span class="inline-flex items-center px-2 py-1 text-xs rounded bg-bg text-text-secondary border border-border">
                        [表情]
                    </span>
                {:else}
                    <span class="inline-flex items-center px-2 py-1 text-xs rounded bg-bg text-text-secondary border border-border">
                        失效表情
                    </span>
                {/if}
            {/if}
        {/each}
    </div>
</div>
