export interface ToastItem {
    id: number;
    message: string;
    type: 'success' | 'error' | 'info';
}

export class ToastStore {
    items = $state<ToastItem[]>([]);
    private nextId = 0;

    show(message: string, type: ToastItem['type'] = 'info', duration = 0) {
        const id = this.nextId++;
        this.items = [...this.items, { id, message, type }];
        if (duration > 0) {
            setTimeout(() => {
                this.remove(id);
            }, duration);
        }
    }

    remove(id: number) {
        this.items = this.items.filter((t) => t.id !== id);
    }
}

export const toastStore = new ToastStore();
