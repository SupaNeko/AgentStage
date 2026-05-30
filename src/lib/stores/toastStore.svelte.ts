export interface ToastItem {
    id: number;
    message: string;
    type: 'success' | 'error' | 'info';
    autoDismiss: boolean;
    duration: number;
    progress: number;
}

export class ToastStore {
    items = $state<ToastItem[]>([]);
    private nextId = 0;
    private timers = new Map<number, { interval: ReturnType<typeof setInterval>; timeout: ReturnType<typeof setTimeout> }>();

    /** 绿色成功提示，默认 5s 后自动消失，可覆盖 duration（单位 ms） */
    success(message: string, duration = 5000) {
        this.add({ message, type: 'success', autoDismiss: true, duration });
    }

    /** 蓝色/默认信息提示，默认 5s 后自动消失，可覆盖 duration（单位 ms） */
    info(message: string, duration = 5000) {
        this.add({ message, type: 'info', autoDismiss: true, duration });
    }

    /** 红色错误提示，永久保留，必须手动关闭。不接受 duration 参数。 */
    error(message: string) {
        this.add({ message, type: 'error', autoDismiss: false, duration: 0 });
    }

    private add(item: Omit<ToastItem, 'id' | 'progress'>) {
        const id = this.nextId++;
        const fullItem: ToastItem = { ...item, id, progress: 100 };
        this.items = [...this.items, fullItem];

        if (item.autoDismiss && item.duration > 0) {
            const intervalMs = 50;
            const step = 100 / (item.duration / intervalMs);

            const interval = setInterval(() => {
                const idx = this.items.findIndex(t => t.id === id);
                if (idx !== -1) {
                    const nextProgress = Math.max(0, this.items[idx].progress - step);
                    this.items[idx].progress = nextProgress;
                    if (nextProgress <= 0) {
                        this.remove(id);
                    }
                }
            }, intervalMs);

            const timeout = setTimeout(() => {
                this.remove(id);
            }, item.duration);

            this.timers.set(id, { interval, timeout });
        }
    }

    remove(id: number) {
        const timer = this.timers.get(id);
        if (timer) {
            clearInterval(timer.interval);
            clearTimeout(timer.timeout);
            this.timers.delete(id);
        }
        this.items = this.items.filter((t) => t.id !== id);
    }
}

export const toastStore = new ToastStore();
