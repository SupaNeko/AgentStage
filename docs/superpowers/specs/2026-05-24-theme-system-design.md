# Theme System — Design Doc

**Date:** 2026-05-24  
**Status:** DRAFT  
**Problem:** No theme system exists — all 30+ components use hardcoded Tailwind v4 `@theme` color tokens for light mode only. Users cannot switch visual appearance. A plugin-capable theme architecture is needed.

---

## 1. Goals

1. **Multiple preset themes**: Ship with default light + a "fantasy bulletin board" theme as built-in presets.
2. **Global switch, instant effect**: Click a theme card → CSS injects immediately via `<style>` textContent replacement.
3. **Plugin-architecture ready**: Theme system is decoupled from future UI extension plugins. User-installed themes go in `data/themes/user/`.
4. **Full visual redefinition (Scope C)**: Each theme controls colors + fonts + border-radius + spacing via `@theme` tokens AND arbitrary component-level CSS overrides.

**Out of scope for this phase:**
- Plugin SDK for UI extensions (only theme plugins)
- Theme marketplace / sharing
- User-installed theme folder (reserved path only)

---

## 2. Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│  themes/                                              │
│  ├── default/        (theme.json + style.css + prev.) │
│  ├── bulletin-board/                                  │
│  └── user/           (reserved for future)            │
└──────────┬───────────────────────────────────────────┘
           │ list_themes() + read_theme_css()
           ▼
┌──────────────────────────────────────────────────────┐
│  Rust Backend (commands/theme.rs)                     │
│    • list_themes() → ThemeInfo[]                      │
│    • read_theme_css(theme_id) → String                │
└──────────┬───────────────────────────────────────────┘
           │ invoke()
           ▼
┌──────────────────────────────────────────────────────┐
│  Frontend                                             │
│  ┌──────────────────┐  ┌───────────────────────────┐ │
│  │ themeStore        │  │ App.svelte                │ │
│  │  • themes[]       │  │  <style id="theme-active"> │ │
│  │  • activeThemeId  │  │  onMount: applyTheme()    │ │
│  │  • loadThemes()   │  │                           │ │
│  │  • applyTheme(id) │  │                           │ │
│  └──────────────────┘  └───────────────────────────┘ │
│  ┌──────────────────────────────────────────────────┐│
│  │ SettingsPanel → "主题" tab → ThemeCards          ││
│  │  (grid of preview + name, click to switch)       ││
│  └──────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘
```

**CSS injection order** in `<head>`:
```
1. Tailwind v4 compiled CSS (Vite build, <style> or <link>)
2. <style id="theme-active"></style>   ← runtime injection point
```

Theme CSS at position 2 naturally overrides Tailwind utilities (later in cascade, same specificity).

---

## 3. Theme File Format

### 3.1 Directory Structure

```
data/themes/
├── default/
│   ├── theme.json
│   ├── style.css
│   └── preview.png       (256×160 recommended)
├── bulletin-board/
│   ├── theme.json
│   ├── style.css
│   └── preview.png
└── user/                 (reserved)
```

### 3.2 `theme.json` — Metadata Manifest

```json
{
  "name": "异世界告示板",
  "id": "bulletin-board",
  "version": "1.0.0",
  "author": "AgentStage",
  "description": "羊皮纸质感的异世界冒险者告示板风格",
  "tags": ["fantasy", "warm", "textured"]
}
```

All fields required except `tags`. `id` must match the directory name and is used as the settings `theme` value.

### 3.3 `style.css` — Two-Layer Theme Definition

**Layer 1: Design Tokens** (`@theme` block — overrides Tailwind v4 semantic colors)

```css
@theme {
  --color-bg:             #f5e6c8;
  --color-surface:        #faf0dc;
  --color-border:         #8b7355;
  --color-text:           #3e2723;
  --color-text-secondary: #6d4c41;
  --color-primary:        #c0392b;
  --color-primary-dark:   #a93226;
}
```

**Layer 2: Component Overrides** (plain CSS selectors targeting semantic class names)

```css
.left-nav {
  background: linear-gradient(180deg, #3e2723, #5d4037);
  border-right: 2px solid #8b7355;
}
.msg-bubble {
  border-radius: 4px;
  border: 1px solid #8b7355;
  font-family: 'Noto Serif SC', serif;
}
```

**Font loading:** Themes may use `@import url('https://fonts.googleapis.com/...')` at the top of style.css. For offline support, `.woff2` files can be placed in the theme directory and referenced with a relative `@font-face`.

**No `!important` needed** — injected `<style>` comes after Tailwind in the cascade with equal specificity, so it wins naturally.

---

## 4. Theme Storage & Discovery

### 4.1 Storage Location

`data/themes/` — same portable directory as the SQLite database (`data/agentstage.db`). This ensures themes move with the app directory.

The `data/themes/` directory is created at app startup if missing. Built-in themes are copied from `src-tauri/resources/themes/` (shipped with the binary in prod, accessible in dev). The `default` theme is always present; other built-in themes are copied on first launch.

### 4.2 Backend Commands

**`list_themes()`** → `ThemeInfo[]`
- Scans `data/themes/` for subdirectories
- Reads each subdirectory's `theme.json`
- Returns array of `{ id, name, version, author, description, tags, preview_path, source: "builtin"|"user" }`
- Source is determined by path: `data/themes/user/` → "user", otherwise "builtin"
- Skips directories without valid `theme.json`

**`read_theme_css(theme_id: String)`** → `String`
- Reads `data/themes/{theme_id}/style.css`
- Returns raw CSS content as string
- Returns error if file not found or `theme_id` contains path traversal (`..`, `/`, `\`)

### 4.3 Frontend `themeStore`

```ts
// src/lib/stores/themeStore.svelte.ts
class ThemeStore {
    themes = $state<ThemeInfo[]>([]);
    activeThemeId = $state<string>('default');

    async loadThemes() {
        this.themes = await invoke<ThemeInfo[]>('list_themes');
    }

    async applyTheme(themeId: string) {
        const css = await invoke<string>('read_theme_css', { themeId });
        let el = document.getElementById('theme-active');
        if (!el) {
            el = document.createElement('style');
            el.id = 'theme-active';
            document.head.appendChild(el);
        }
        el.textContent = css;
        this.activeThemeId = themeId;
        await invoke('update_settings', { theme: themeId });
    }
}
export const themeStore = new ThemeStore();
```

---

## 5. Theme Switching Flow

```
User clicks theme card in SettingsPanel
  │
  ▼
themeStore.applyTheme('bulletin-board')
  │
  ├─► invoke('read_theme_css', { themeId: 'bulletin-board' })
  │     └─► Backend reads data/themes/bulletin-board/style.css
  │
  ├─► document.getElementById('theme-active').textContent = css
  │     └─► Browser immediately repaints with new tokens & overrides
  │
  └─► invoke('update_settings', { theme: 'bulletin-board' })
        └─► Persists choice to SQLite settings table
```

**Startup flow:**
```
App.svelte onMount
  │
  ├─► settingsStore.load()          // reads settings.theme from DB
  ├─► themeStore.loadThemes()        // discovers available themes
  └─► themeStore.applyTheme(settings.theme)  // restores active theme
```

---

## 6. Component Semantic Class Names

To enable theme CSS to target specific UI regions, 14 semantic class names are added to 7 existing components. These are **additive only** — no existing code is changed.

| Component | New Class | Element | Theme Can Control |
|-----------|-----------|---------|-------------------|
| `LeftNav.svelte` | `left-nav` | Root `<nav>` | Background, border, width |
| `LeftNav.svelte` | `nav-tab` | Each tab button | Icon color, active state |
| `AgentList` / `SessionList` / `HistorySessionList` | `mid-panel` | Root `<div>` | Background, border |
| (same) | `list-item` | Each list row | Hover color, selected state, padding |
| `ChatView.svelte` | `chat-view` | Root `<div>` | Layout, background |
| `ChatView.svelte` | `chat-header` | Top title bar | Background, border, shadow |
| `ChatView.svelte` | `chat-input-area` | Bottom input bar | Input bg, border style |
| `MessageBubble.svelte` | `msg-bubble` | Bubble root | Shape (radius/corner), shadow, max-width |
| `MessageBubble.svelte` | `msg-self` | Own messages | Color, alignment |
| `MessageBubble.svelte` | `msg-other` | Other's messages | Color, alignment |
| Modals | `modal-overlay` | Backdrop | Overlay color/blur |
| Modals | `modal-card` | Card body | Border, shadow, radius |
| Buttons & Inputs | `btn-primary` | Primary button | Shape, border, shadow |
| Buttons & Inputs | `input-field` | Text input | Border, radius, focus ring |

**Naming rules:**
- All lowercase + hyphens, no camelCase
- Avoid collision with Tailwind utility classes (no `flex`, `text`, `block` etc.)
- Appended after existing Tailwind classes: `class="flex items-center gap-2 left-nav"`
- Not used with Svelte `class:` directive (reserved for conditional styles)
- Do NOT add `!important` in theme CSS — cascade order handles precedence

---

## 7. Settings Panel — Theme Selector UI

### 7.1 Layout

SettingsPanel gains a new "主题" tab in its existing left sidebar navigation. The tab content is a scrollable grid of theme cards.

```
┌──────────────────────────────────────────────┐
│  Settings                    [✕]             │
├────────┬─────────────────────────────────────┤
│        │  选择主题                           │
│ 通用   │  切换后立即生效                      │
│ ▸主题  │                                     │
│ 触发   │  ┌──────────┐  ┌──────────────┐   │
│ 安静   │  │  [preview] │  │  [preview]   │   │
│ 头像   │  │  默认亮色  │  │  异世界告示板 │   │
│        │  │  内置  ✓  │  │  内置        │   │
│        │  └──────────┘  └──────────────┘   │
│        │                                     │
└────────┴─────────────────────────────────────┘
```

### 7.2 Behavior

| Action | Behavior |
|--------|----------|
| Click card | Immediately applies theme (no confirmation dialog). Card border turns blue + checkmark appears. |
| Preview image | `convertFileSrc(preview_path)` resolves to asset URL. Missing preview → gradient placeholder in theme colors. |
| Built-in vs User | "内置" or "用户" label below name. |
| Close panel | Theme stays applied (already persisted via `update_settings`). |
| Switching | `textContent` replacement is synchronous — no flicker between themes. |

### 7.3 Implementation in SettingsPanel

Existing SettingsPanel has tabs: `general`, `trigger`, `quiet_hours`, `avatar`. Add `appearance` tab. The content area renders a card grid (inline in SettingsPanel, or extracted as `ThemeSelector.svelte` if the file grows too large).

```svelte
{#if activeTab === 'appearance'}
  <div class="grid grid-cols-2 gap-3 p-4">
    {#each themeStore.themes as theme}
      <button
        class="theme-card {themeStore.activeThemeId === theme.id ? 'ring-2 ring-primary' : ''}"
        onclick={() => themeStore.applyTheme(theme.id)}
      >
        <ThemePreview src={theme.preview_path} />
        <span>{theme.name}</span>
        <span class="text-text-secondary text-xs">{theme.source === 'builtin' ? '内置' : '用户'}</span>
      </button>
    {/each}
  </div>
{/if}
```

---

## 8. Bulletin Board Theme — Visual Design

### 8.1 Physical Scene & Mood

> An adventurer stands before a worn wooden bulletin board in a guild hall. Flickering candlelight warms the oak frame. Parchment notices overlap on the board, each pinned with a small portrait badge of its author. The air smells of old paper and polished wood.

This drives a **warm, light-medium wood tone** palette with 3D depth everywhere — nothing is flat. Shadows create the sense of physical layers: the wall behind, the board surface, and the pinned plaques on top.

### 8.2 Color Palette

| Token | Hex | Role | Physical Metaphor |
|-------|-----|------|-------------------|
| `--color-bg` | `#e8d5b0` | Page background | Bulletin board surface (light oak) |
| `--color-surface` | `#faf3e3` | Cards / panels | Parchment sheets |
| `--color-border` | `#a0845c` | Borders / dividers | Wood frame edges |
| `--color-text` | `#3e2723` | Primary text | Dark brown ink |
| `--color-text-secondary` | `#8b6b4a` | Secondary text | Faded ink |
| `--color-primary` | `#b8402e` | Accent / active | Wax seal red |
| `--color-primary-dark` | `#972e2a` | Hover / pressed | Darker wax seal |

**Strategy: Committed** — warm wood tones carry 40-50% of the visual surface. Wax seal red is ≤8%, used for active states, buttons, and links only.

### 8.3 Wood Grain Texture (CSS)

Pure CSS, no image assets. Applied to key backgrounds via a reusable class or injected CSS:

```css
/* Base wood background */
body { background-color: #e8d5b0; }

/* Wood grain pattern — subtle horizontal lines */
.wood-grain {
  background-image:
    repeating-linear-gradient(
      90deg,
      transparent, transparent 10px,
      rgba(139, 107, 74, 0.06) 10px,
      rgba(139, 107, 74, 0.06) 12px
    ),
    repeating-linear-gradient(
      90deg,
      transparent, transparent 3px,
      rgba(160, 132, 92, 0.04) 3px,
      rgba(160, 132, 92, 0.04) 4px
    );
}
```

### 8.4 Message Bubbles — Wooden Plaques

The signature element of this theme. Each message is a raised wooden plaque pinned to the board.

**Structure:**
```
┌─────────────────────────────┐
│         ●  (nail = avatar)  │  ← ::before pseudo-element
│ ┌─────────────────────────┐ │
│ │ Alice                  │ │  ← sender name
│ │                        │ │
│ │ Message text in serif  │ │  ← content
│ │                        │ │
│ │                 14:33  │ │  ← timestamp
│ └─────────────────────────┘ │
└─────────────────────────────┘
```

**CSS recipe for `.msg-bubble`:**
```css
.msg-bubble {
  /* Wood plank gradient */
  background: linear-gradient(180deg, #d4b896 0%, #c4a882 40%, #ba9e78 60%, #c4a882 100%);
  /* Sharp corners like cut wood, not modern round */
  border-radius: 4px;
  /* Wood frame border */
  border: 1px solid #a0845c;
  /* 3D raised effect: near hard shadow + far soft shadow */
  box-shadow:
    3px 3px 0 rgba(0, 0, 0, 0.12),
    6px 6px 12px rgba(0, 0, 0, 0.08);
  /* Font */
  font-family: 'Noto Serif SC', Georgia, serif;
  color: #3e2723;
}
```

**Nail (avatar pin) via `::before`:**
```css
.msg-bubble::before {
  content: '';
  position: absolute;
  top: -14px;
  left: 50%;
  transform: translateX(-50%);
  width: 28px;
  height: 28px;
  border-radius: 50%;
  /* Avatar image as background (set per-sender by component) */
  background-image: var(--sender-avatar);
  background-size: cover;
  border: 2px solid #8b6914;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}
```

**Self vs other messages:**
- `.msg-self`: Right-aligned, slightly different wood shade (lighter), nail on right side
- `.msg-other`: Left-aligned, standard wood shade, nail on left side, with avatar next to bubble

### 8.5 Other Component Treatments

| Component | Class | Visual Treatment |
|-----------|-------|-----------------|
| LeftNav | `.left-nav` | Dark oak frame: `linear-gradient(180deg, #c4a168, #b89050, #c4a168)`, right border as raised wooden edge |
| Nav tabs | `.nav-tab` | Carved-wood icon slots: recessed dark squares with `box-shadow: inset` |
| Mid panels | `.mid-panel` | Parchment sheet: `bg-surface` with subtle deckle-edge via `border-image` or irregular border-color |
| Chat header | `.chat-header` | Wooden header bar with bottom shadow like a shelf |
| Input area | `.chat-input-area` | Recessed wood trough: `box-shadow: inset 0 2px 4px rgba(0,0,0,0.1)`, wooden border |
| Buttons | `.btn-primary` | Raised copper rivet: gradient body + bottom shadow simulating physical depth |
| | `.btn-primary:hover` | Presses down: reduce shadow, translate-y 1px |
| Modals | `.modal-overlay` | Darkened with warm tint: `rgba(62, 39, 35, 0.5)` |
| | `.modal-card` | Parchment sheet with torn-edge feel, heavy shadow |

**Typography:**
- Body (chat messages, panel text): `'Noto Serif SC', 'Source Han Serif SC', Georgia, serif`
- UI (buttons, labels, nav): System serif fallback
- Optional enhancement: `ZCOOL XiaoWei` for title/header text

### 8.6 Design Principles for This Theme

1. **Everything has depth**: No flat color blocks. Every surface has shadow, gradient, or texture.
2. **Warm, not dark**: bg is `#e8d5b0` (light oak), not a dark tavern. Readable in normal ambient light.
3. **Physical metaphor**: Components behave like real objects — buttons press down, panels cast shadows, plaques are pinned.
4. **Messy-adjacent, not messy**: Slight irregularities (wood grain, parchment tone variation) but clean execution. No artificial noise or grunge.
5. **Avatar is the pin**: The sender's identity is literally what holds their message to the board.

---

## 9. File Inventory

### New Files
| File | Purpose |
|------|---------|
| `src-tauri/src/commands/theme.rs` | `list_themes` + `read_theme_css` Tauri Commands |
| `src/lib/stores/themeStore.svelte.ts` | Theme state management (Svelte 5 Runes) |
| `data/themes/default/theme.json` | Default light theme manifest |
| `data/themes/default/style.css` | Default light theme CSS (current @theme values) |
| `data/themes/default/preview.png` | Default theme preview image |
| `data/themes/bulletin-board/theme.json` | Fantasy bulletin board manifest |
| `data/themes/bulletin-board/style.css` | Fantasy bulletin board CSS |
| `data/themes/bulletin-board/preview.png` | Bulletin board preview image |

### Modified Files
| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Register `list_themes`, `read_theme_css` commands |
| `src/App.svelte` | `onMount`: load settings → load themes → apply active theme; also apply when settings change |
| `src/lib/components/SettingsPanel.svelte` | Add "主题" tab + theme card grid |
| `src/lib/types.ts` | Add `ThemeInfo` interface |
| `src/lib/components/LeftNav.svelte` | Add `left-nav`, `nav-tab` classes |
| `src/lib/components/AgentList.svelte` | Add `mid-panel`, `list-item` classes |
| `src/lib/components/SessionList.svelte` | Add `mid-panel`, `list-item` classes |
| `src/lib/components/HistorySessionList.svelte` | Add `mid-panel`, `list-item` classes |
| `src/lib/components/ChatView.svelte` | Add `chat-view`, `chat-header`, `chat-input-area` classes |
| `src/lib/components/MessageBubble.svelte` | Add `msg-bubble`, `msg-self`/`msg-other` classes |

### Unchanged (no modifications needed)
All other components, stores, backend modules, CSS files — no changes. Components continue using Tailwind utility classes as before; they just gain a semantic class name next to them.

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Runtime `@theme` CSS variables might not override Tailwind v4 generated utilities | Validate before writing code: inject a test `<style>` with `@theme{--color-bg: red}` and verify `bg-bg` changes. If Tailwind v4's JIT compilation bakes values into utilities, fall back to approach A (CSS custom properties on `:root`). |
| Theme CSS specificity conflicts with Svelte scoped styles | Theme CSS is injected globally via `<style>` element, matching the specificity of Svelte's scoped styles (single class selector). Cascade order (later wins) gives theme preference. |
| `preview.png` file path resolution in dev vs prod | Use Tauri `convertFileSrc` which handles both `asset://` protocol (prod) and `http://asset.localhost` (dev). |
| `read_theme_css` path traversal attack | Reject `theme_id` containing `..`, `/`, `\`. Also prepend `data/themes/` to the resolved path and verify it starts with the expected prefix. |

---

## 11. Verification Checklist

- [ ] Switch theme in SettingsPanel → UI repaints immediately (no flash, no reload)
- [ ] Restart app → active theme persists
- [ ] All 6 semantic tokens change correctly (bg, surface, border, text, text-secondary, primary)
- [ ] Component overrides work (at minimum: left-nav background, msg-bubble border-radius)
- [ ] Preview thumbnails display correctly
- [ ] Built-in label shows "内置"
- [ ] `cargo check` 0 errors
- [ ] `svelte-check` 0 errors
- [ ] Default theme CSS produces identical appearance to current app

---

*Design approved by user on 2026-05-24. Awaiting spec review before implementation.*
