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

    show(message: string, type: ToastItem['type'] = 'info', autoDismissOrDuration: boolean | number = false, duration = 0) {
        let autoDismiss: boolean;
        let finalDuration: number;

        if (typeof autoDismissOrDuration === 'boolean') {
            autoDismiss = autoDismissOrDuration;
            finalDuration = duration;
        } else if (typeof autoDismissOrDuration === 'number' && autoDismissOrDuration > 0) {
            autoDismiss = true;
            finalDuration = autoDismissOrDuration;
        } else {
            autoDismiss = false;
            finalDuration = 0;
        }

        const id = this.nextId++;
        const item: ToastItem = { id, message, type, autoDismiss, duration: finalDuration, progress: 100 };
        this.items = [...this.items, item];

        if (autoDismiss && finalDuration > 0) {
            const intervalMs = 50;
            const step = 100 / (finalDuration / intervalMs);

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
            }, finalDuration);

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
