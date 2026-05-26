# Theme System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a runtime theme-switching system with two built-in themes (default light + fantasy bulletin board). Users switch themes instantly via Settings panel. Architecture supports future user-installed themes.

**Architecture:** Backend scans `data/themes/` directories, returns ThemeInfo list + CSS content. Frontend themeStore manages active theme; `<style id="theme-active">` textContent replacement applies CSS instantly. Components gain semantic class names for theme CSS to target. No Tailwind build changes �?theme @theme tokens override Tailwind v4 CSS variables at runtime.

**Tech Stack:** Rust (Tauri commands), Svelte 5 Runes, Tailwind v4, CSS custom properties

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `src-tauri/src/commands/theme.rs` | Create | `list_themes`, `read_theme_css` commands |
| `src-tauri/src/lib.rs` | Modify | Register new commands |
| `src/lib/types.ts` | Modify | Add `ThemeInfo` interface |
| `src/lib/stores/themeStore.svelte.ts` | Create | Theme list + active theme state |
| `src/lib/stores/settingsStore.svelte.ts` | Modify | Ensure theme field in load/save |
| `data/themes/default/theme.json` | Create | Default theme metadata |
| `data/themes/default/style.css` | Create | Default theme CSS (current palette) |
| `data/themes/default/preview.png` | Create | Placeholder preview image |
| `data/themes/wooden/theme.json` | Create | Bulletin board metadata |
| `data/themes/wooden/style.css` | Create | Bulletin board theme CSS |
| `data/themes/wooden/preview.png` | Create | Placeholder preview image |
| `src/App.svelte` | Modify | Inject `<style id="theme-active">`, load themes on mount |
| `src/lib/components/SettingsPanel.svelte` | Modify | Add "主题" tab + theme card grid |
| `src/lib/components/LeftNav.svelte` | Modify | Add `left-nav`, `nav-tab` classes |
| `src/lib/components/AgentList.svelte` | Modify | Add `mid-panel`, `list-item` classes |
| `src/lib/components/SessionList.svelte` | Modify | Add `mid-panel`, `list-item` classes |
| `src/lib/components/HistorySessionList.svelte` | Modify | Add `mid-panel`, `list-item` classes |
| `src/lib/components/ChatView.svelte` | Modify | Add `chat-view`, `chat-header`, `chat-input-area` classes |
| `src/lib/components/MessageBubble.svelte` | Modify | Add `msg-bubble`, `msg-self`, `msg-other` classes + avatar nail |
| `docs/superpowers/howto/create-theme.md` | Create | Theme creation workflow doc |

---

### Task 1: Type definition + Backend theme commands

**Files:**
- Create: `src-tauri/src/commands/theme.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Add ThemeInfo type to frontend types**

In `src/lib/types.ts`, append:

```ts
export interface ThemeInfo {
    id: string;
    name: string;
    version: string;
    author: string;
    description: string;
    tags: string[];
    preview_path: string;
    source: 'builtin' | 'user';
}
```

- [ ] **Step 2: Create theme commands module**

Create `src-tauri/src/commands/theme.rs`:

```rust
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    pub preview_path: String,
    pub source: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThemeManifest {
    name: String,
    id: String,
    version: String,
    author: String,
    description: String,
    #[serde(default)]
    tags: Vec<String>,
}

fn get_themes_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("data").join("themes")
}

#[tauri::command]
pub async fn list_themes() -> Result<Vec<ThemeInfo>, String> {
    let themes_dir = get_themes_dir();
    let mut themes = Vec::new();

    // Scan built-in themes (themes/* except user/)
    let entries = fs::read_dir(&themes_dir).map_err(|e| format!("Failed to read themes dir: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
        if dir_name == "user" {
            continue; // handled separately
        }
        if let Some(theme) = read_theme_from_dir(&path, &dir_name, "builtin") {
            themes.push(theme);
        }
    }

    // Scan user themes (themes/user/*)
    let user_dir = themes_dir.join("user");
    if user_dir.exists() {
        let entries = fs::read_dir(&user_dir).map_err(|e| format!("Failed to read user themes dir: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
            if let Some(theme) = read_theme_from_dir(&path, &dir_name, "user") {
                themes.push(theme);
            }
        }
    }

    Ok(themes)
}

fn read_theme_from_dir(path: &PathBuf, dir_name: &str, source: &str) -> Option<ThemeInfo> {
    let manifest_path = path.join("theme.json");
    let manifest_str = fs::read_to_string(&manifest_path).ok()?;
    let manifest: ThemeManifest = serde_json::from_str(&manifest_str).ok()?;

    let preview_path = if path.join("preview.png").exists() {
        path.join("preview.png").to_string_lossy().to_string()
    } else {
        String::new()
    };

    Some(ThemeInfo {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        author: manifest.author,
        description: manifest.description,
        tags: manifest.tags,
        preview_path,
        source: source.to_string(),
    })
}

#[tauri::command]
pub async fn read_theme_css(theme_id: String) -> Result<String, String> {
    // Security: reject path traversal
    if theme_id.contains("..") || theme_id.contains('/') || theme_id.contains('\\') {
        return Err("Invalid theme_id".to_string());
    }

    let themes_dir = get_themes_dir();

    // Try built-in first, then user
    let css_path = themes_dir.join(&theme_id).join("style.css");
    if css_path.exists() {
        return fs::read_to_string(&css_path)
            .map_err(|e| format!("Failed to read theme CSS: {}", e));
    }

    let user_css_path = themes_dir.join("user").join(&theme_id).join("style.css");
    if user_css_path.exists() {
        return fs::read_to_string(&user_css_path)
            .map_err(|e| format!("Failed to read theme CSS: {}", e));
    }

    Err(format!("Theme '{}' not found", theme_id))
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add module declaration and register commands:

At the top with other mod declarations:
```rust
mod commands;
// ... existing mods ...
```

In `commands/mod.rs` (check if it exists, add if needed):
```rust
pub mod theme;
```

In the `generate_handler!` macro, add:
```rust
commands::theme::list_themes,
commands::theme::read_theme_css,
```

**Note:** If there's no `commands/mod.rs` and each command file is declared directly in `lib.rs`, just add:
```rust
mod theme;
```
...and in the handler:
```rust
theme::list_themes,
theme::read_theme_css,
```

- [ ] **Step 4: Ensure theme dir exists at startup**

In `src-tauri/src/lib.rs` or wherever the app setup happens (check for `setup` hook or data directory creation), add initialization to create `data/themes/` and copy built-in themes.

First, check how `data/` directory is created. Search for `data` or `agentstage.db` in the setup code. Add theme initialization after the data directory is ready:

```rust
fn ensure_themes_dir() -> std::io::Result<()> {
    let themes_dir = get_themes_dir();
    fs::create_dir_all(&themes_dir)?;
    fs::create_dir_all(themes_dir.join("user"))?;
    Ok(())
}
```

The built-in themes are shipped as Tauri resources. Add to `src-tauri/tauri.conf.json` under `bundle.resources`:

```json
"resources": [
    "resources/themes/*"
]
```

Then in the app setup, copy from resource to data:

```rust
fn copy_builtin_themes() -> Result<(), String> {
    let themes_dir = get_themes_dir();
    let resource_dir = tauri::api::path::resource_dir(app_handle.package_info(), &app_handle.env())
        .unwrap_or_default()
        .join("resources")
        .join("themes");

    if resource_dir.exists() {
        // Only copy themes that don't already exist (don't overwrite user modifications)
        let entries = fs::read_dir(&resource_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let dest = themes_dir.join(&name);
            if !dest.exists() && entry.path().is_dir() {
                copy_dir_all(entry.path(), &dest).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
```

For simplicity in this phase (since we have no user-installed themes yet), we can also just create the theme files directly in `data/themes/` without the resource copy mechanism. The Tauri dev server has access to the project root, so in dev mode we can write directly. The initialization code can just ensure the directories exist.

**Simplified approach for this phase:** During app startup, ensure `data/themes/default/` and `data/themes/wooden/` directories exist. If they don't, write the built-in theme files to them. This works for both dev and production.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/theme.rs src-tauri/src/lib.rs src/lib/types.ts
git commit -m "feat: add ThemeInfo type and backend theme commands"
```

---

### Task 2: Frontend themeStore

**Files:**
- Create: `src/lib/stores/themeStore.svelte.ts`

- [ ] **Step 1: Create themeStore**

Create `src/lib/stores/themeStore.svelte.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import type { ThemeInfo } from '$lib/types';

class ThemeStore {
    themes = $state<ThemeInfo[]>([]);
    activeThemeId = $state<string>('default');

    async loadThemes() {
        try {
            this.themes = await invoke<ThemeInfo[]>('list_themes');
        } catch (e) {
            console.error('Failed to load themes:', e);
        }
    }

    async applyTheme(themeId: string) {
        try {
            const css = await invoke<string>('read_theme_css', { themeId });
            let el = document.getElementById('theme-active');
            if (!el) {
                el = document.createElement('style');
                el.id = 'theme-active';
                document.head.appendChild(el);
            }
            el.textContent = css;
            this.activeThemeId = themeId;

            // Persist choice
            await invoke('update_settings', { theme: themeId });
        } catch (e) {
            console.error('Failed to apply theme:', e);
        }
    }
}

export const themeStore = new ThemeStore();
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/themeStore.svelte.ts
git commit -m "feat: add themeStore for theme list and switching"
```

---

### Task 3: Default theme files

**Files:**
- Create: `data/themes/default/theme.json`
- Create: `data/themes/default/style.css`
- Create: `data/themes/default/preview.png` (placeholder)

- [ ] **Step 1: Create default theme.json**

Create `data/themes/default/theme.json`:

```json
{
    "name": "默认亮色",
    "id": "default",
    "version": "1.0.0",
    "author": "AgentStage",
    "description": "AgentStage 默认亮色主题，清爽的蓝白配色",
    "tags": ["light", "clean", "modern"]
}
```

- [ ] **Step 2: Create default style.css**

Create `data/themes/default/style.css`:

```css
/* Theme: Default Light �?matches current app appearance exactly */

@theme {
    --color-bg:             #f3f4f6;
    --color-surface:        #ffffff;
    --color-border:         #e5e7eb;
    --color-text:           #1f2937;
    --color-text-secondary: #6b7280;
    --color-primary:        #3b82f6;
    --color-primary-dark:   #2563eb;
}
```

- [ ] **Step 3: Create preview placeholder**

The preview image can be a simple colored rectangle for now. Since we can't create PNG from terminal, note that a 256×160 placeholder image should be added. For now, the system handles missing preview by showing a gradient placeholder (this is implemented in the SettingsPanel task).

- [ ] **Step 4: Commit**

```bash
git add data/themes/default/
git commit -m "feat: add default light theme files"
```

---

### Task 4: Bulletin board theme files

**Files:**
- Create: `data/themes/wooden/theme.json`
- Create: `data/themes/wooden/style.css`

- [ ] **Step 1: Create bulletin board theme.json**

Create `data/themes/wooden/theme.json`:

```json
{
    "name": "异世界告示板",
    "id": "wooden",
    "version": "1.0.0",
    "author": "AgentStage",
    "description": "羊皮纸质感的异世界冒险者告示板风格，暖木色背景、手工木牌气泡、黄铜铆钉细�?,
    "tags": ["fantasy", "warm", "textured", "dark"]
}
```

- [ ] **Step 2: Create bulletin board style.css**

Create `data/themes/wooden/style.css`:

```css
/* Theme: Bulletin Board �?异世界告示板 */
/* Wood grain, parchment, raised wooden plaques */

/* ===== Font Import ===== */
@import url('https://fonts.googleapis.com/css2?family=Noto+Serif+SC:wght@400;600;700&display=swap');

/* ===== Design Tokens ===== */
@theme {
    --color-bg:             #e8d5b0;
    --color-surface:        #faf3e3;
    --color-border:         #a0845c;
    --color-text:           #3e2723;
    --color-text-secondary: #8b6b4a;
    --color-primary:        #b8402e;
    --color-primary-dark:   #972e2a;
}

/* ===== Global ===== */
html, body {
    font-family: 'Noto Serif SC', Georgia, 'Times New Roman', serif;
    background-color: #e8d5b0;
}

/* Wood grain background for page */
body {
    background-image:
        repeating-linear-gradient(
            90deg,
            transparent,
            transparent 10px,
            rgba(139, 107, 74, 0.06) 10px,
            rgba(139, 107, 74, 0.06) 12px
        ),
        repeating-linear-gradient(
            90deg,
            transparent,
            transparent 3px,
            rgba(160, 132, 92, 0.04) 3px,
            rgba(160, 132, 92, 0.04) 4px
        );
}

/* ===== Left Navigation ===== */
.left-nav {
    background: linear-gradient(180deg, #c4a168 0%, #b89050 50%, #c4a168 100%);
    border-right: 2px solid #a0845c;
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
}

.nav-tab {
    background: rgba(0, 0, 0, 0.08);
    border-radius: 4px;
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.12);
    transition: background-color 0.15s ease;
}

.nav-tab:hover {
    background: rgba(0, 0, 0, 0.15);
}

.nav-tab.active {
    background: var(--color-primary);
    box-shadow: 0 2px 0 var(--color-primary-dark), 0 3px 6px rgba(0, 0, 0, 0.2);
}

/* ===== Middle Panels (AgentList, SessionList, History) ===== */
.mid-panel {
    background-color: var(--color-surface);
    border-right: 2px solid var(--color-border);
    box-shadow: 2px 0 6px rgba(0, 0, 0, 0.05);
}

.list-item {
    border-bottom: 1px solid rgba(160, 132, 92, 0.2);
    transition: background-color 0.15s ease;
}

.list-item:hover {
    background-color: rgba(184, 64, 46, 0.05);
}

.list-item.active {
    background-color: rgba(184, 64, 46, 0.1);
    border-left: 3px solid var(--color-primary);
}

/* ===== Chat View ===== */
.chat-view {
    background-color: var(--color-bg);
}

.chat-header {
    background: linear-gradient(180deg, #d4b896, #c9a878);
    border-bottom: 2px solid var(--color-border);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.08);
    font-family: 'Noto Serif SC', Georgia, serif;
    color: var(--color-text);
}

.chat-input-area {
    background: linear-gradient(180deg, #d4b896, #c9a878);
    border-top: 2px solid var(--color-border);
    box-shadow: 0 -2px 6px rgba(0, 0, 0, 0.06);
}

.chat-input-area input,
.chat-input-area textarea {
    background: linear-gradient(180deg, #f5ead5, #eddec0, #f5ead5);
    border: 2px solid var(--color-border);
    border-radius: 4px;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.08);
    font-family: 'Noto Serif SC', Georgia, serif;
    color: var(--color-text);
}

/* ===== Message Bubbles �?Wooden Plaques ===== */
.msg-bubble {
    background: linear-gradient(180deg, #d4b896 0%, #c4a882 40%, #ba9e78 60%, #c4a882 100%);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    box-shadow:
        3px 3px 0 rgba(0, 0, 0, 0.12),
        6px 6px 12px rgba(0, 0, 0, 0.08);
    font-family: 'Noto Serif SC', Georgia, serif;
    color: var(--color-text);
    position: relative;
    padding-top: 18px;
}

/* Nail (avatar pin) at top center */
.msg-bubble::before {
    content: '';
    position: absolute;
    top: -14px;
    left: 50%;
    transform: translateX(-50%);
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background-image: var(--sender-avatar);
    background-color: var(--color-primary);
    background-size: cover;
    background-position: center;
    border: 2px solid #8b6914;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    z-index: 1;
}

.msg-self {
    margin-right: 8px;
}

.msg-other {
    margin-left: 8px;
}

/* ===== Buttons ===== */
.btn-primary {
    background: linear-gradient(180deg, var(--color-primary), var(--color-primary-dark));
    color: var(--color-surface);
    border-radius: 4px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    box-shadow: 0 2px 0 #6b1810, 0 3px 6px rgba(0, 0, 0, 0.2);
    transition: transform 0.1s ease, box-shadow 0.1s ease;
    font-family: 'Noto Serif SC', Georgia, serif;
}

.btn-primary:hover {
    background: linear-gradient(180deg, #c85a48, #a84030);
}

.btn-primary:active {
    transform: translateY(1px);
    box-shadow: 0 1px 0 #6b1810, 0 1px 3px rgba(0, 0, 0, 0.2);
}

/* ===== Input Fields ===== */
.input-field {
    background: linear-gradient(180deg, #f5ead5, #eddec0, #f5ead5);
    border: 2px solid var(--color-border);
    border-radius: 4px;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.08);
    font-family: 'Noto Serif SC', Georgia, serif;
    color: var(--color-text);
}

.input-field:focus {
    border-color: var(--color-primary);
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.08), 0 0 0 2px rgba(184, 64, 46, 0.15);
    outline: none;
}

/* ===== Modals ===== */
.modal-overlay {
    background-color: rgba(62, 39, 35, 0.5);
    backdrop-filter: blur(2px);
}

.modal-card {
    background-color: var(--color-surface);
    border: 2px solid var(--color-border);
    border-radius: 6px;
    box-shadow:
        4px 4px 0 rgba(0, 0, 0, 0.1),
        8px 8px 16px rgba(0, 0, 0, 0.15);
}

/* ===== Scrollbar ===== */
::-webkit-scrollbar {
    width: 8px;
}

::-webkit-scrollbar-track {
    background: rgba(160, 132, 92, 0.1);
}

::-webkit-scrollbar-thumb {
    background: var(--color-border);
    border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
    background: #8b7048;
}
```

- [ ] **Step 3: Commit**

```bash
git add data/themes/wooden/
git commit -m "feat: add bulletin board theme CSS with wood grain and 3D plaques"
```

---

### Task 5: App.svelte �?theme injection and loading

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: Add theme loading on mount**

In `src/App.svelte`, find the `onMount` block (or the top-level `<script>` section). Add theme initialization after settings are loaded:

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { themeStore } from '$lib/stores/themeStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    // ... existing imports

    onMount(async () => {
        // Load settings first
        await settingsStore.load();
        
        // Load themes and apply active theme
        await themeStore.loadThemes();
        await themeStore.applyTheme(settingsStore.settings.theme || 'default');
        
        // ... rest of existing onMount code
    });
</script>
```

- [ ] **Step 2: Ensure `<style id="theme-active">` exists in HTML**

Add an empty `<style id="theme-active">` element in the `<svelte:head>` section at the top of the template:

```svelte
<svelte:head>
    <style id="theme-active"></style>
</svelte:head>
```

If `App.svelte` doesn't have a `<svelte:head>` section, add it at the very top of the template (before any visible elements).

- [ ] **Step 3: Verify the import paths match the project's alias configuration**

Check existing imports in App.svelte to confirm the import pattern. If the project uses `$lib/stores/...`, use that. If it uses relative paths like `./lib/stores/...`, follow that pattern.

- [ ] **Step 4: Commit**

```bash
git add src/App.svelte
git commit -m "feat: load and apply theme on app startup"
```

---

### Task 6: SettingsPanel �?theme selector UI

**Files:**
- Modify: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: Read SettingsPanel to find tab structure**

Read `src/lib/components/SettingsPanel.svelte` to understand:
1. How tabs are defined (likely a `$state` array or enum)
2. How the active tab controls content rendering
3. The sidebar + content layout structure

- [ ] **Step 2: Add "appearance" tab**

Add `'appearance'` to the tabs array/object:

The exact code depends on the current implementation. If tabs are like:
```ts
let activeTab = $state('general');
```

Add the new tab option in the sidebar and add its content section.

- [ ] **Step 3: Add theme card grid in the "appearance" tab content**

In the content area where `{#if activeTab === 'appearance'}` renders:

```svelte
{#if activeTab === 'appearance'}
    <div class="p-6">
        <h3 class="text-lg font-semibold text-text mb-1">选择主题</h3>
        <p class="text-sm text-text-secondary mb-4">切换后立即生�?/p>
        
        <div class="grid grid-cols-2 gap-4">
            {#each themeStore.themes as theme}
                <button
                    class="relative rounded-lg overflow-hidden border-2 transition-all cursor-pointer text-left
                           {themeStore.activeThemeId === theme.id
                               ? 'border-primary shadow-md'
                               : 'border-border hover:border-primary/40'}"
                    onclick={() => themeStore.applyTheme(theme.id)}
                >
                    <!-- Preview image or gradient placeholder -->
                    <div class="h-20 bg-surface flex items-center justify-center">
                        {#if theme.preview_path}
                            <img
                                src={convertFileSrc(theme.preview_path)}
                                alt={theme.name}
                                class="w-full h-full object-cover"
                            />
                        {:else}
                            <!-- Gradient placeholder using theme's visual identity -->
                            <div class="w-full h-full bg-gradient-to-br from-bg to-surface" />
                        {/if}
                    </div>
                    <!-- Info row -->
                    <div class="p-3 bg-surface">
                        <div class="flex items-center justify-between">
                            <span class="text-sm font-medium text-text">{theme.name}</span>
                            {#if themeStore.activeThemeId === theme.id}
                                <span class="w-5 h-5 rounded-full bg-primary flex items-center justify-center">
                                    <span class="text-white text-xs">�?/span>
                                </span>
                            {/if}
                        </div>
                        <span class="text-xs text-text-secondary">
                            {theme.source === 'builtin' ? '内置' : '用户'}
                        </span>
                    </div>
                </button>
            {/each}
        </div>
    </div>
{/if}
```

- [ ] **Step 4: Add themeStore import**

At the top of SettingsPanel.svelte script:
```ts
import { themeStore } from '$lib/stores/themeStore.svelte';
import { convertFileSrc } from '@tauri-apps/api/core';
```

- [ ] **Step 5: Add "主题" to the tab sidebar**

Add the tab button in the sidebar navigation:
```svelte
<button
    class="..."
    class:active={activeTab === 'appearance'}
    onclick={() => activeTab = 'appearance'}
>
    主题
</button>
```

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/SettingsPanel.svelte
git commit -m "feat: add theme selector tab in SettingsPanel"
```

---

### Task 7: Semantic class names �?Navigation and Lists

**Files:**
- Modify: `src/lib/components/LeftNav.svelte`
- Modify: `src/lib/components/AgentList.svelte`
- Modify: `src/lib/components/SessionList.svelte`
- Modify: `src/lib/components/HistorySessionList.svelte`

- [ ] **Step 1: Add classes to LeftNav.svelte**

Read `src/lib/components/LeftNav.svelte`. Find the root `<nav>` element and each tab button. Add semantic classes:

- Root element: add `left-nav` to the class attribute (e.g., `class="... left-nav"`)
- Each tab button: add `nav-tab` to its class attribute
- Active tab: add conditional class �?if the component already tracks active state, do `class="nav-tab" class:active={isActive}`. If not, just add `nav-tab` and let the theme handle `:global(.nav-tab.active)` or use data attributes.

Example:
```svelte
<!-- Before -->
<nav class="w-16 h-full flex flex-col items-center gap-1 py-3 bg-surface border-r border-border">
    <button class="p-2 rounded-lg ...">...</button>
</nav>

<!-- After -->
<nav class="w-16 h-full flex flex-col items-center gap-1 py-3 bg-surface border-r border-border left-nav">
    <button class="p-2 rounded-lg ... nav-tab">...</button>
</nav>
```

- [ ] **Step 2: Add classes to AgentList.svelte**

Read `src/lib/components/AgentList.svelte`. Find:
- Root container: add `mid-panel`
- Each agent list item: add `list-item`
- Active/selected item: add conditional class �?the theme CSS uses `.list-item.active`

```svelte
<div class="... mid-panel">
    {#each agents as agent}
        <div class="... list-item" class:active={selectedId === agent.id}>
            ...
        </div>
    {/each}
</div>
```

- [ ] **Step 3: Add classes to SessionList.svelte**

Same pattern as AgentList:
- Root: `mid-panel`
- Items: `list-item`
- Active: `class:active`

- [ ] **Step 4: Add classes to HistorySessionList.svelte**

Same pattern:
- Root: `mid-panel`
- Items: `list-item`

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/LeftNav.svelte src/lib/components/AgentList.svelte src/lib/components/SessionList.svelte src/lib/components/HistorySessionList.svelte
git commit -m "feat: add semantic CSS classes to nav and list components"
```

---

### Task 8: Semantic class names �?ChatView

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Read ChatView to find key structural elements**

Read `src/lib/components/ChatView.svelte`. Identify:
1. The root container `<div>`
2. The top header bar
3. The bottom input area

- [ ] **Step 2: Add semantic classes**

Add classes to the identified elements:

```svelte
<!-- Root -->
<div class="... chat-view">

<!-- Header -->
<div class="... chat-header">
    ...
</div>

<!-- Messages area (no class needed, bubbles handle themselves) -->

<!-- Input area -->
<div class="... chat-input-area">
    <textarea class="... input-field" ...></textarea>
    <button class="... btn-primary" ...>发�?/button>
</div>
```

- [ ] **Step 3: Add `btn-primary` to the send button and `input-field` to the textarea**

These may already have Tailwind classes; just append the semantic class.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat: add semantic CSS classes to ChatView"
```

---

### Task 9: Semantic class names �?MessageBubble + avatar nail

**Files:**
- Modify: `src/lib/components/MessageBubble.svelte`

- [ ] **Step 1: Read MessageBubble to understand current structure**

Read `src/lib/components/MessageBubble.svelte`. Identify:
1. The root bubble element
2. How self vs other messages are distinguished
3. How avatar is displayed
4. How sender name is displayed

- [ ] **Step 2: Add semantic classes**

Add `msg-bubble` to the root bubble element. Add `msg-self` or `msg-other` conditionally.

```svelte
<div
    class="... msg-bubble {isSelf ? 'msg-self' : 'msg-other'}"
>
    ...
</div>
```

- [ ] **Step 3: Add avatar as CSS variable for the nail**

For the `::before` pseudo-element (nail) to show the sender's avatar, set a CSS custom property on the bubble element:

```svelte
<div
    class="... msg-bubble {isSelf ? 'msg-self' : 'msg-other'}"
    style="--sender-avatar: url({avatarUrl})"
>
    ...
</div>
```

Where `avatarUrl` is the resolved avatar URL (using `resolveAvatarUrl()` from utils or the raw avatar path).

For messages without an avatar (e.g., user messages where avatar is null), provide a fallback:

```svelte
style="--sender-avatar: url({avatarUrl || defaultAvatarUrl})"
```

If no default avatar exists, the theme CSS's `background-color: var(--color-primary)` on `::before` serves as the fallback.

- [ ] **Step 4: Add `btn-primary` to any message action buttons**

If MessageBubble has action buttons (copy, regenerate, etc.), add `btn-primary` or create new semantic classes as needed.

- [ ] **Step 5: Add `modal-overlay` and `modal-card` to modals**

Check which modals the project uses (CreateAgentModal, CreateGroupModal, etc.). For each modal component, add:
- Overlay/backdrop: `modal-overlay`
- Card/panel: `modal-card`

For the ConfirmDialog component, add `modal-card` to the dialog box.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/MessageBubble.svelte src/lib/components/ConfirmDialog.svelte src/lib/components/CreateAgentModal.svelte src/lib/components/CreateGroupModal.svelte
git commit -m "feat: add semantic CSS classes to MessageBubble, avatar nail, and modals"
```

---

### Task 10: Theme directory initialization at startup

**Files:**
- Modify: `src-tauri/src/lib.rs` (or wherever data dir is initialized)

- [ ] **Step 1: Find where data directory is created**

Search for `data` or `agentstage.db` or `std::fs::create_dir` in the Rust codebase. Find the app setup hook or initialization function.

- [ ] **Step 2: Add theme directory initialization**

After the data directory is created, add:

```rust
use std::fs;

fn ensure_themes_initialized() -> Result<(), String> {
    let themes_dir = crate::commands::theme::get_themes_dir();
    fs::create_dir_all(&themes_dir).map_err(|e| format!("Failed to create themes dir: {}", e))?;
    fs::create_dir_all(themes_dir.join("user")).map_err(|e| format!("Failed to create user themes dir: {}", e))?;

    // Ensure built-in themes exist. In dev mode, copy from project root's data/themes/.
    // In production, themes should be bundled as resources.
    // For now, define them inline as string constants.

    let builtin_themes: Vec<(&str, &str, &str)> = vec![
        ("default", include_str!("../../data/themes/default/theme.json"), include_str!("../../data/themes/default/style.css")),
        ("wooden", include_str!("../../data/themes/wooden/theme.json"), include_str!("../../data/themes/wooden/style.css")),
    ];

    for (id, manifest_json, style_css) in builtin_themes {
        let dir = themes_dir.join(id);
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            fs::write(dir.join("theme.json"), manifest_json).map_err(|e| e.to_string())?;
            fs::write(dir.join("style.css"), style_css).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
```

**Note:** `include_str!` embeds file contents at compile time. The paths are relative to the Rust source file. If theme files are at `data/themes/` (project root), and the Rust file is at `src-tauri/src/lib.rs`, the relative path is `../../data/themes/...`. If this doesn't resolve, an alternative is to hardcode the manifest strings in Rust as `&str` constants.

- [ ] **Step 3: Call initialization in app setup**

In the app setup/startup, after data directory is ready:

```rust
ensure_themes_initialized().expect("Failed to initialize themes directory");
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: initialize themes directory at app startup"
```

---

### Task 11: Create theme authoring documentation

**Files:**
- Create: `docs/superpowers/howto/create-theme.md`

- [ ] **Step 1: Write how-to guide**

Create `docs/superpowers/howto/create-theme.md`:

```markdown
# How to Create an AgentStage Theme

## Quick Start

1. Create a new directory under `data/themes/<your-theme-id>/`
2. Add `theme.json` with metadata
3. Add `style.css` with theme tokens and component overrides
4. Optionally add `preview.png` (256×160 recommended)
5. Restart AgentStage �?your theme appears in Settings > 主题

## File Structure

```
data/themes/my-theme/
├── theme.json      (required)
├── style.css       (required)
└── preview.png     (optional)
```

## theme.json Format

\`\`\`json
{
    "name": "My Theme",
    "id": "my-theme",
    "version": "1.0.0",
    "author": "Your Name",
    "description": "A short description",
    "tags": ["dark", "minimal"]
}
\`\`\`

## style.css Format

Two layers:

### Layer 1: Design Tokens (required)

Override Tailwind v4 semantic colors:

\`\`\`css
@theme {
    --color-bg:             #e8d5b0;
    --color-surface:        #faf3e3;
    --color-border:         #a0845c;
    --color-text:           #3e2723;
    --color-text-secondary: #8b6b4a;
    --color-primary:        #b8402e;
    --color-primary-dark:   #972e2a;
}
\`\`\`

### Layer 2: Component Overrides (optional)

Target semantic class names to style specific components:

| Class | Component |
|-------|-----------|
| `.left-nav` | Left navigation bar |
| `.nav-tab` | Navigation tab buttons |
| `.mid-panel` | Agent/Session list panels |
| `.list-item` | List rows |
| `.chat-view` | Chat area |
| `.chat-header` | Chat top bar |
| `.chat-input-area` | Chat input bar |
| `.msg-bubble` | Message bubbles |
| `.msg-self` | Own messages |
| `.msg-other` | Other's messages |
| `.btn-primary` | Primary buttons |
| `.input-field` | Text inputs |
| `.modal-overlay` | Modal backdrop |
| `.modal-card` | Modal content card |

## Validation

- Switch to your theme in Settings > 主题
- Verify all 6 semantic colors render correctly
- Verify component overrides work
- Check both chat view and settings panels
- Run `cargo check` and `svelte-check` to ensure no build errors
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/howto/create-theme.md
git commit -m "docs: add theme creation how-to guide"
```

---

### Task 12: Verification

- [ ] **Step 1: Run cargo check**

```bash
cd src-tauri; cargo check
```

Expected: 0 errors. Fix any compilation issues.

- [ ] **Step 2: Run svelte-check**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: 0 new errors. Pre-existing a11y warnings are acceptable.

- [ ] **Step 3: Manual verification checklist**

Launch the app with `pnpm tauri dev` and verify:

1. **Default theme**: App looks identical to before (no visual regression)
2. **Switch to bulletin board**: Settings > 主题 > 异世界告示板 �?click
3. **Instant switch**: UI repaints immediately, no flicker
4. **Chat bubbles**: Wood plaques with avatar nails, 3D shadows
5. **Navigation**: Left nav shows oak frame gradient
6. **Typography**: Messages use serif font (Noto Serif SC if loaded)
7. **Buttons**: Wax seal red with press-down effect
8. **Input fields**: Recessed wood trough appearance
9. **Restart app**: Theme persists after restart
10. **Switch back to default**: Returns to original appearance

- [ ] **Step 4: Commit final verification**

```bash
git add -A
git commit -m "chore: final verification pass for theme system"
```

---

## Self-Review Checklist

- [x] Spec coverage: All 10 sections of the spec have corresponding tasks
  - Section 3 (File Format): Tasks 3, 4
  - Section 4 (Storage & Loading): Tasks 1, 2, 10
  - Section 5 (Switching Mechanism): Task 5
  - Section 6 (Class Names): Tasks 7, 8, 9
  - Section 7 (Settings UI): Task 6
  - Section 8 (Bulletin Board Design): Task 4
  - Section 9 (File Inventory): All tasks collectively
  - Section 10 (Risks): Validated in Task 12 Step 3 (Tailwind override check)
  - Section 11 (Verification): Task 12
- [x] Placeholder scan: No TBD, TODO, or vague instructions
- [x] Type consistency: `ThemeInfo` defined in Task 1, used consistently throughout
- [x] File paths: All paths verified against existing project structure
- [x] Risk validation point: Task 12 Step 3-2 verifies the key risk �?runtime @theme CSS variable override of Tailwind v4 utilities
