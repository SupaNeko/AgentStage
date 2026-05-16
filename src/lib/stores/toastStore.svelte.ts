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

    show(message: string, type: ToastItem['type'] = 'info', autoDismiss = false, duration = 0) {
        const id = this.nextId++;
        const item: ToastItem = { id, message, type, autoDismiss, duration, progress: 100 };
        this.items = [...this.items, item];

        if (autoDismiss && duration > 0) {
            const intervalMs = 50;
            const step = 100 / (duration / intervalMs);

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
            }, duration);

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
