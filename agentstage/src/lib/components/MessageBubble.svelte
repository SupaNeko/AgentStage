<script lang="ts">
    import type { Message } from '$lib/types';
    import { formatTime } from '$lib/utils';

    interface Props {
        message: Message;
        isMe: boolean;
        senderName: string;
    }

    let { message, isMe, senderName }: Props = $props();
</script>

<div class="max-w-[80%]">
    {#if !isMe}
        <div class="text-xs text-text-secondary mb-1">{senderName}</div>
    {/if}
    <div
        class="{isMe
            ? 'bg-primary text-white rounded-2xl rounded-tr-sm'
            : 'bg-surface border border-border rounded-2xl rounded-tl-sm'} px-4 py-2"
    >
        {message.content}
    </div>
    <div class="text-xs text-text-secondary mt-1 {isMe ? 'text-right' : 'text-left'}">
        {formatTime(message.created_at)}
    </div>
</div>
