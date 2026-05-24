<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Upload, User } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { resolveAvatarUrl } from '$lib/utils';

    interface Props {
        open?: boolean;
        targetType: 'user_default' | 'user_persona' | 'agent' | 'group';
        targetId: string;
        currentAvatar?: string | null;
        onClose: () => void;
        onUploaded: (path: string) => void;
    }

    let { open = true, targetType, targetId, currentAvatar = null, onClose, onUploaded }: Props = $props();

    let uploading = $state(false);
    let fileInput: HTMLInputElement | undefined = $state(undefined);

    // Cropper state
    let cropMode = $state(false);
    let canvasEl: HTMLCanvasElement | undefined = $state(undefined);
    let imgObj: HTMLImageElement | null = $state(null);
    let scale = $state(1);
    let offsetX = $state(0);
    let offsetY = $state(0);
    let isDragging = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let dragStartOffsetX = $state(0);
    let dragStartOffsetY = $state(0);

    const CANVAS_SIZE = 300;
    const CROP_RADIUS = 100;
    const CROP_SIZE = CROP_RADIUS * 2;

    function handleFileSelect(e: Event) {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (ev) => {
            const dataUrl = ev.target?.result as string;
            if (!dataUrl) return;
            imgObj = new Image();
            imgObj.onload = () => {
                // Initialize scale so image fills crop circle
                const minScale = Math.max(CROP_SIZE / imgObj!.width, CROP_SIZE / imgObj!.height);
                scale = minScale * 1.2; // slight zoom to allow adjustment
                offsetX = 0;
                offsetY = 0;
                cropMode = true;
                requestAnimationFrame(drawCanvas);
            };
            imgObj.src = dataUrl;
        };
        reader.readAsDataURL(file);
        if (fileInput) fileInput.value = '';
    }

    function drawCanvas() {
        if (!canvasEl || !imgObj) return;
        const ctx = canvasEl.getContext('2d');
        if (!ctx) return;

        ctx.clearRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);

        // Draw image
        const imgW = imgObj.width * scale;
        const imgH = imgObj.height * scale;
        const x = CANVAS_SIZE / 2 - imgW / 2 + offsetX;
        const y = CANVAS_SIZE / 2 - imgH / 2 + offsetY;
        ctx.drawImage(imgObj, x, y, imgW, imgH);

        // Draw mask outside the crop circle (rect minus circle, preserving image inside)
        ctx.save();
        ctx.beginPath();
        ctx.rect(0, 0, CANVAS_SIZE, CANVAS_SIZE);
        ctx.moveTo(CANVAS_SIZE / 2 + CROP_RADIUS, CANVAS_SIZE / 2);
        ctx.arc(CANVAS_SIZE / 2, CANVAS_SIZE / 2, CROP_RADIUS, 0, Math.PI * 2, true);
        ctx.closePath();
        ctx.fillStyle = 'rgba(0, 0, 0, 0.75)';
        ctx.fill();
        ctx.restore();

        // Draw circle border
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(CANVAS_SIZE / 2, CANVAS_SIZE / 2, CROP_RADIUS, 0, Math.PI * 2);
        ctx.stroke();
    }

    function handleMouseDown(e: MouseEvent) {
        if (!cropMode || !canvasEl) return;
        isDragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        dragStartOffsetX = offsetX;
        dragStartOffsetY = offsetY;
    }

    function handleMouseMove(e: MouseEvent) {
        if (!isDragging) return;
        const dx = e.clientX - dragStartX;
        const dy = e.clientY - dragStartY;
        offsetX = dragStartOffsetX + dx;
        offsetY = dragStartOffsetY + dy;
        drawCanvas();
    }

    function handleMouseUp() {
        isDragging = false;
    }

    function handleWheel(e: WheelEvent) {
        if (!cropMode) return;
        e.preventDefault();
        const delta = e.deltaY > 0 ? 0.9 : 1.1;
        scale *= delta;
        // Clamp scale
        if (scale < 0.1) scale = 0.1;
        if (scale > 5) scale = 5;
        drawCanvas();
    }

    function getCroppedImage(): string {
        if (!imgObj) return '';
        const cropCanvas = document.createElement('canvas');
        cropCanvas.width = CROP_SIZE;
        cropCanvas.height = CROP_SIZE;
        const ctx = cropCanvas.getContext('2d');
        if (!ctx) return '';

        const imgW = imgObj.width * scale;
        const imgH = imgObj.height * scale;
        const imgX = CANVAS_SIZE / 2 - imgW / 2 + offsetX;
        const imgY = CANVAS_SIZE / 2 - imgH / 2 + offsetY;

        // Source coordinates in original image
        const sourceX = (CANVAS_SIZE / 2 - CROP_RADIUS - imgX) / scale;
        const sourceY = (CANVAS_SIZE / 2 - CROP_RADIUS - imgY) / scale;
        const sourceW = CROP_SIZE / scale;
        const sourceH = CROP_SIZE / scale;

        ctx.drawImage(imgObj, sourceX, sourceY, sourceW, sourceH, 0, 0, CROP_SIZE, CROP_SIZE);
        return cropCanvas.toDataURL('image/png');
    }

    async function handleConfirmCrop() {
        if (!imgObj) return;
        uploading = true;
        try {
            const base64 = getCroppedImage();
            const path = await invoke<string>('upload_avatar', {
                req: { target_type: targetType, target_id: targetId, image_data_base64: base64 }
            });
            toastStore.show('头像上传成功', 'success', 2000);
            onUploaded(path);
            cropMode = false;
            imgObj = null;
        } catch (err) {
            toastStore.show('上传失败: ' + String(err), 'error', 5000);
        } finally {
            uploading = false;
        }
    }

    function handleCancelCrop() {
        cropMode = false;
        imgObj = null;
        scale = 1;
        offsetX = 0;
        offsetY = 0;
    }
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center modal-overlay" onclick={onClose} role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-80 shadow-xl modal-card" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">头像管理</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded" aria-label="关闭">
                    <X size={18} />
                </button>
            </div>

            {#if cropMode}
                <div class="flex flex-col items-center gap-4">
                    <canvas
                        bind:this={canvasEl}
                        width={CANVAS_SIZE}
                        height={CANVAS_SIZE}
                        class="rounded-lg cursor-move"
                        onmousedown={handleMouseDown}
                        onmousemove={handleMouseMove}
                        onmouseup={handleMouseUp}
                        onmouseleave={handleMouseUp}
                        onwheel={handleWheel}
                    />
                    <p class="text-xs text-text-secondary">拖拽移动，滚轮缩放</p>
                    <div class="flex gap-2">
                        <button
                            onclick={handleCancelCrop}
                            class="px-4 py-2 text-sm border border-border rounded-lg hover:bg-bg transition-colors"
                        >
                            取消
                        </button>
                        <button
                            onclick={handleConfirmCrop}
                            disabled={uploading}
                            class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50 btn-primary"
                        >
                            <Upload size={16} />
                            {uploading ? '上传中...' : '确认'}
                        </button>
                    </div>
                </div>
            {:else}
                <div class="flex flex-col items-center gap-4">
                    {#if currentAvatar}
                        <img src={resolveAvatarUrl(currentAvatar)} alt="当前头像" class="w-24 h-24 rounded-full object-cover" />
                    {:else}
                        <div class="w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                            <User size={40} />
                        </div>
                    {/if}
                    <input type="file" accept="image/*" bind:this={fileInput} onchange={handleFileSelect} class="hidden" />
                    <button
                        onclick={() => fileInput?.click()}
                        class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors btn-primary"
                    >
                        <Upload size={16} />
                        上传新头像
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}
