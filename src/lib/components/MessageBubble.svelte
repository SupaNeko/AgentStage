<script lang="ts">
    import type { Message } from '$lib/types';
    import { formatTime } from '$lib/utils';
    import { User, Bot } from 'lucide-svelte';

    interface Props {
        message: Message;
        isMe: boolean;
        senderName: string;
    }

    let { message, isMe, senderName }: Props = $props();
</script>

<div class="max-w-[80%] flex {isMe ? 'flex-row-reverse' : 'flex-row'} items-end gap-2">
    <!-- Avatar -->
    {#if !isMe}
        <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
            {#if message.sender_avatar}
                <img src={message.sender_avatar} alt={senderName} class="w-full h-full object-cover" />
            {:else}
                <Bot size={16} />
            {/if}
        </div>
    {/if}

    <div class="flex flex-col {isMe ? 'items-end' : 'items-start'}">
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
        <div class="text-xs text-text-secondary mt-1">
            {formatTime(message.created_at)}
        </div>
    </div>

    <!-- User avatar on the right for user messages -->
    {#if isMe}
        <div class="w-8 h-8 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0">
            <User size={16} />
        </div>
    {/if}
</div>
