class AppState {
    sidebarOpen = $state(true);
    currentView = $state<'agents' | 'chat' | 'settings'>('agents');
    
    toggleSidebar() {
        this.sidebarOpen = !this.sidebarOpen;
    }
}

export const appState = new AppState();
