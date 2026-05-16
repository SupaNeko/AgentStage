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

<div class="flex flex-col max-w-[80%] {isMe ? 'items-end' : 'items-start'}">
    <!-- 头像 + 名称/时间 -->
    <div class="flex items-center gap-2 mb-1 {isMe ? 'flex-row-reverse' : ''}">
        <div class="w-8 h-8 rounded-full flex items-center justify-center shrink-0 overflow-hidden {message.sender_type === 'user' ? 'bg-gray-300 text-white' : 'bg-primary/10 text-primary'}">
            {#if message.sender_avatar}
                <img src={message.sender_avatar} alt={senderName} class="w-full h-full object-cover" />
            {:else if message.sender_type === 'user'}
                <User size={16} />
            {:else}
                <Bot size={16} />
            {/if}
        </div>
        <div class="flex flex-col justify-center h-8 {isMe ? 'items-end' : 'items-start'}">
            <span class="text-xs text-text-secondary leading-none">{senderName}</span>
            <span class="text-[10px] text-text-secondary opacity-70 leading-none mt-0.5">{formatTime(message.created_at)}</span>
        </div>
    </div>

    <!-- 聊天气泡 -->
    <div
        class="{isMe
            ? 'bg-primary text-white rounded-2xl rounded-tr-sm'
            : 'bg-surface border border-border rounded-2xl rounded-tl-sm'} px-4 py-2 min-w-[80px]"
    >
        {message.content}
    </div>
</div>
