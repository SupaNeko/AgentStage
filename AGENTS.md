# AgentStage — Agent Quick Reference

AgentStage is a Windows desktop multi-agent roleplay chat app built with **Tauri v2** (Rust backend + WebView2 frontend). The frontend uses **Svelte 5 + Vite + TailwindCSS v4**; the backend uses **Rust + SQLite (rusqlite)**. All LLM API calls are proxied through the Rust backend—**never from the frontend**.

---

## Development Commands

```bash
# Full dev environment (starts Vite + compiles Rust + opens window)
pnpm tauri dev

# Frontend build only
pnpm build

# Rust type check only (~3s)
cd src-tauri
cargo check

# Run Rust backend only (no Vite frontend)
cd src-tauri
cargo run

# Svelte type check
npx svelte-check --tsconfig ./tsconfig.json
```

> **Important:** `pnpm dev` alone starts Vite but not the Rust backend. Always use `pnpm tauri dev` for full-stack development.

---

## Project Boundaries

| Directory | Role |
|-----------|------|
| `src/` | Frontend: Svelte 5 components, stores (`.svelte.ts`), types |
| `src-tauri/src/` | Rust backend: Tauri Commands, DB repositories, models, crypto |
| `src-tauri/src/db/` | SQLite connection, schema, migrations, handwritten repositories |
| `src-tauri/src/commands/` | Tauri IPC command handlers (exposed to frontend via `invoke`) |
| `src-tauri/src/models/` | Rust structs for DB rows and request/response DTOs |
| `docs/` | Product docs: PRD.md, feature_list.md, schema.md, tech-stack.md |

---

## Frontend Traps (Svelte 5 + Tailwind v4)

### Mount syntax
Svelte 5 uses `mount()`, not `new App()`:
```ts
import { mount } from 'svelte';
const app = mount(App, { target: document.getElementById('app')! });
```

### `tsconfig.json` — `useDefineForClassFields` must be `false`
Svelte 5 Runes (`$state`) inside classes will break at runtime if this is `true`:
```json
"useDefineForClassFields": false
```

### TailwindCSS v4 syntax
Use `@import "tailwindcss"` and `@theme` in `styles.css`. Do **not** use `@tailwind base/components/utilities` or `tailwind.config.js`.
Custom colors are defined in `@theme`:
```css
@theme {
  --color-primary: #3b82f6;
  --color-bg: #f3f4f6;
}
```

### Svelte `class:` directive does not support `/`
Class names with opacity modifiers (e.g. `bg-primary/10`) cannot be used with Svelte's `class:` directive. Use inline conditional strings instead:
```svelte
<!-- Wrong -->
<div class:bg-primary/10={active} />

<!-- Right -->
<div class={active ? 'bg-primary/10 text-primary' : ''} />
```

### State management
Use Svelte 5 Runes in `.svelte.ts` files. No Redux/Zustand needed. Example:
```ts
// src/lib/stores/appState.svelte.ts
class AppState {
    currentView = $state<'agents' | 'chat' | 'history'>('agents');
    selectedAgentId = $state<string | null>(null);
}
export const appState = new AppState();
```

---

## Backend Traps (Rust + SQLite)

### Database location
SQLite file is created at runtime in the program directory (forced portable mode):
```
<exe_dir>\data\agentstage.db
```
In dev mode, data is stored at the project root `data/`.
WAL mode is enforced (`PRAGMA journal_mode = WAL`).

### Async mutex for DB connection
The `DbState` wraps the `rusqlite::Connection` in a `tokio::sync::Mutex`. **Never** use `std::sync::Mutex` in async Tauri commands.

### No ORM — handwritten SQL
All queries are raw SQL in repository modules (`src/db/*.rs`). Schema changes require:
1. Update `src/db/schema.rs` (DDL)
2. Add a migration in `src/db/migration.rs`
3. Update the corresponding repository CRUD methods

### API Key security
- API Keys are encrypted with AES-256-GCM in Rust (`src/crypto.rs`)
- `AgentResponse` DTO **excludes** `api_key_encrypted` — it never leaves the backend
- Frontend sends the raw key only during create/update; backend encrypts before storage

### LLM calls go through Rust
Frontend **must not** call OpenAI/Claude APIs directly. All LLM interactions are Tauri Commands that the Rust backend executes. This protects API keys and prevents Prompt inspection via DevTools.

---

## Tauri IPC Design

Commands are registered in `src/lib.rs` via `tauri::generate_handler!`. Current commands (in `src/commands/agent.rs`):
- `create_agent`
- `get_agent`
- `list_agents`
- `update_agent`
- `delete_agent` (soft delete)

Frontend calls them with:
```ts
import { invoke } from '@tauri-apps/api/core';
const agents = await invoke<Agent[]>('list_agents');
```

---

## Code Style & Conventions

- **Naming:** Product calls them "角色" (character/role). Code identifiers remain `agent`/`Agent` for consistency with the repo naming.
- **Path alias:** `$lib` maps to `src/lib` (configured in `vite.config.js` and `tsconfig.json`)
- **Imports:** Use `@tauri-apps/api/core` for `invoke` (Tauri v2), not `@tauri-apps/api/tauri`
- **CSS:** Tailwind v4 utility classes only. Custom colors are the `@theme` tokens (`bg-bg`, `bg-surface`, `text-primary`, etc.)
- **Git:** Do not run `git commit` unless the user explicitly asks. Do not push to remote without confirmation.

---

## Reference Projects (cloned locally)

| Path | Purpose |
|------|---------|
| `reference/SillyTavern/` | Prompt assembly, Tool/Function Calling logic |
| `reference/RisuAI/` | Tauri v2 + Svelte 5 architecture patterns |
| `reference/cc-switch/` | LLM provider configuration patterns (OpenAI, Claude, Kimi, MiniMax) |
| `reference/text-generation-webui/` | API key encryption patterns |

---

## E2E Testing (Playwright)

AgentStage uses **Playwright** for end-to-end frontend testing. Since AgentStage is a Tauri desktop app, E2E tests run against the Vite dev server (`http://127.0.0.1:1420`) with mocked Tauri IPC — this tests frontend UI rendering and interaction without requiring the Rust backend.

### Setup

```bash
cd agentstage
pnpm add -D @playwright/test
# Use system Chrome/Edge instead of downloading Chromium:
pnpm exec playwright install-deps chromium
```

### Configuration (`playwright.config.ts`)

```typescript
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  reporter: 'html',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    channel: 'chrome', // Use system Chrome
    headless: true,
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    command: 'pnpm dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
  },
});
```

### Mocking Tauri IPC

**Critical:** Tauri v2's `invoke` uses `window.__TAURI_INTERNALS__.invoke`, not `window.__TAURI__.core.invoke`. You must mock `__TAURI_INTERNALS__` and also implement `transformCallback` / `unregisterCallback` for event support.

In `test.beforeEach`, inject via `page.addInitScript(...)`:

```typescript
await page.addInitScript(() => {
  const callbacks = new Map();
  const eventListeners = new Map();

  function registerCallback(callback, once = false) {
    const id = window.crypto.getRandomValues(new Uint32Array(1))[0];
    callbacks.set(id, (data) => { if (once) callbacks.delete(id); return callback?.(data); });
    return id;
  }

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      // Handle event plugin
      if (cmd === 'plugin:event|listen') { /* ... */ }
      if (cmd === 'plugin:event|emit') { /* ... */ }
      // Handle app commands
      switch (cmd) {
        case 'list_agents': return [...];
        case 'create_private_session': return {...};
        // ... etc
      }
    },
    transformCallback: registerCallback,
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (id, data) => callbacks.get(id)?.(data),
    callbacks,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } }
  };
});
```

See `e2e/smoke.spec.ts` for a complete working example.

### Running Tests

```bash
# Run all E2E tests
pnpm exec playwright test e2e/

# Run with headed browser (for debugging)
pnpm exec playwright test e2e/ --headed

# View HTML report
pnpm exec playwright show-report
```

### Limitations

- **Mocked backend:** E2E tests mock all Tauri Commands. They test frontend UI/UX, not real Rust backend logic.
- **No WebView2 testing:** Tests run in Chrome, not WebView2. WebView2-specific behaviors are not covered.
- **Real backend tests:** Use `cargo test` for Rust unit/integration tests; use `pnpm test` (Vitest) for frontend component tests.

---

## Common Issues

| Symptom | Fix |
|---------|-----|
| Blank white window | Ensure `main.ts` uses `mount()`, `tsconfig.json` has `useDefineForClassFields: false`, and `tauri.conf.json` `devUrl` matches Vite bind address (`127.0.0.1:1420`) |
| `cargo check` passes but `pnpm tauri dev` fails | Check Vite console for Svelte compile errors; a11y warnings are non-fatal |
| Database locked / busy | Only one `Connection` exists (managed in `DbState` Mutex). Check for unreleased locks in repository code |
| Playwright `invoke` mock not working | Check that you mock `window.__TAURI_INTERNALS__.invoke`, not `window.__TAURI__.core.invoke` |

---

*Last updated: 2026-05-10*
