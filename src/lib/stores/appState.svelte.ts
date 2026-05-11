class AppState {
    currentView = $state<'agents' | 'chat' | 'history'>('agents');
    selectedAgentId = $state<string | null>(null);
    selectedSessionId = $state<string | null>(null);
    settingsOpen = $state(false);

    switchView(view: 'agents' | 'chat' | 'history') {
        this.currentView = view;
        // Reset selections when switching views
        if (view === 'agents') {
            this.selectedSessionId = null;
        } else {
            this.selectedAgentId = null;
        }
    }

    selectAgent(id: string | null) {
        this.selectedAgentId = id;
    }

    selectSession(id: string | null) {
        this.selectedSessionId = id;
    }

    openSettings() {
        this.settingsOpen = true;
    }

    closeSettings() {
        this.settingsOpen = false;
    }
}

export const appState = new AppState();
