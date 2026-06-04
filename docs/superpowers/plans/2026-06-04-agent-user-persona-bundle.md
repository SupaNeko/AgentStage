# Agent and User Persona Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `.agentstage` bundle export/import for AI agents and user personas, including embedded avatars and package-internal relationships, memory, and friendships.

**Architecture:** Backend owns bundle validation, export filtering, avatar embedding/writing, import preview, and DB writes. Frontend provides a selection-mode export UI, import preview modal with editable names and optional model assignment, and warning/confirmation flows. Exported files are written by the backend to `<exe_dir>/exports/bundles/`.

**Tech Stack:** Rust/Tauri v2, rusqlite, serde_json, base64, Svelte 5, TypeScript, TailwindCSS v4

**Project rule:** Do not commit during implementation unless the user explicitly asks.

---

## File Map

| File | Responsibility |
|------|----------------|
| `.gitignore` | Ignore `/exports/` so dev export artifacts are not committed |
| `docs/feature_list.md` | Track `AGT-20` status |
| `src-tauri/src/db/agent_bundle.rs` | Bundle structs, export/import preview logic, DB import/export logic, tests |
| `src-tauri/src/db/mod.rs` | Register `agent_bundle` module |
| `src-tauri/src/commands/agent_bundle.rs` | Tauri commands for preview/export/import |
| `src-tauri/src/commands/mod.rs` | Register command module |
| `src-tauri/src/lib.rs` | Register Tauri command handlers |
| `src/lib/types.ts` | Bundle-related TypeScript interfaces |
| `src/lib/components/AgentList.svelte` | Export selection UI and import button |
| `src/lib/components/AgentBundleImportModal.svelte` | Import preview UI |

---

## Task 1: Backend Bundle Core

**Files:**
- Create: `src-tauri/src/db/agent_bundle.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Define bundle structs**

Create Rust structs for:
- `AgentStageBundle`
- `BundleAgent`
- `BundleUserPersona`
- `BundleRelationship`
- `BundleFriendship`
- `BundleAsset`
- `ExportBundlePreview`
- `ExportBundleResult`
- `ImportBundlePreview`
- `ImportPreviewAgent`
- `ImportPreviewUserPersona`
- `ImportAgentSelection`
- `ImportUserPersonaSelection`
- `ImportBundleRequest`
- `ImportBundleResult`

Required constants:
- `BUNDLE_FORMAT = "agentstage.bundle"`
- `BUNDLE_VERSION = 1`

- [ ] **Step 2: Implement export preview**

Implement `preview_export_bundle(conn, agent_ids, user_persona_ids)`.

Rules:
- Empty selection returns an error.
- Count selected existing non-deleted agents and user personas.
- Count omitted package-external relationship descriptions, relationship memories, and agent-agent friendships.
- Relationship memory count means `agent_relationships.memory_text`, not `agents.long_term_memory`.

- [ ] **Step 3: Implement export bundle**

Implement `export_bundle_to_file(conn, export_root, agent_ids, user_persona_ids, confirm_omissions)`.

Rules:
- If omissions exist and `confirm_omissions` is false, return preview with `requires_confirmation = true`.
- Bundle includes selected agents and user personas.
- Agent entries exclude `model_config_id` and temperature.
- Agent long-term memory is stored on agent entries.
- Relationship entries include only selected observer agent to selected target agent/user persona.
- Friendships include only selected agent-agent pairs.
- Avatar assets are embedded as base64 when source files exist; missing avatar files produce warnings and do not block export.
- Backend writes file to `<exe_dir>/exports/bundles/<timestamp>.agentstage`.

- [ ] **Step 4: Implement import preview**

Implement `preview_import_bundle(conn, json)`.

Rules:
- Parse JSON string only; do not read file paths.
- Validate `format`, `version`, duplicate bundle-local IDs, relationship references, friendship references.
- Suggested names use shared namespace across agents and user personas.
- Existing or intra-import collisions produce `Name (导入)` / `Name (导入 2)` suggestions.
- Return avatar data URLs for preview when embedded assets exist.

- [ ] **Step 5: Implement import**

Implement `import_bundle(conn, data_dir, json, agent_selections, user_persona_selections)`.

Rules:
- Use user-confirmed names.
- Agent `model_config_id` is optional.
- Agent temperature override is always `NULL`.
- Embedded avatars are written to local avatar storage with UUID file names; never overwrite existing files.
- New IDs are generated for all imported agents and user personas.
- Rebuild internal relationships/memory and friendships using new IDs.
- Imported user personas are not activated.

- [ ] **Step 6: Tests**

Add unit tests in `agent_bundle.rs`:
- export A/B from A/B/C omits C relationships and warns.
- export A/B/U includes A->U relationship memory.
- import rebuilds agent-agent and agent-user-persona relationships with new IDs.
- import rebuilds friendships.
- import auto-renames against existing agent and user persona names.
- import allows missing model selection and sets `model_config_id` to `NULL`.
- import does not activate imported user persona.
- import rejects duplicate bundle-local IDs.

- [ ] **Step 7: Register module**

Add `pub mod agent_bundle;` to `src-tauri/src/db/mod.rs`.

- [ ] **Step 8: Verify**

Run:

```bash
cd src-tauri
cargo test db::agent_bundle::tests -- --nocapture
```

Expected: all agent bundle tests pass.

---

## Task 2: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/agent_bundle.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add commands**

Create commands:
- `preview_agent_bundle_export(req)`
- `export_agent_bundle(req)`
- `preview_agent_bundle_import(req)`
- `import_agent_bundle(req)`

Command behavior:
- `preview_agent_bundle_import` and `import_agent_bundle` accept file content string, not file path.
- `export_agent_bundle` returns exported path after backend writes the file.
- Commands use camelCase-compatible request fields for frontend calls.

- [ ] **Step 2: Register commands**

Add module in `commands/mod.rs` and handlers/imports in `lib.rs`.

- [ ] **Step 3: Verify**

Run:

```bash
cd src-tauri
cargo check
```

Expected: no Rust compile errors.

---

## Task 3: Frontend Import/Export UI

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/components/AgentList.svelte`
- Create: `src/lib/components/AgentBundleImportModal.svelte`

- [ ] **Step 1: Add TypeScript types**

Add interfaces matching command responses and requests:
- export preview/result
- import preview
- import selections/result

- [ ] **Step 2: Export selection mode**

Update `AgentList.svelte`:
- Header buttons: `导入`, `导出`, `新建`.
- Export mode title: `选择要导出的配置`.
- Helper text: `可以同时导出多个角色和用户人设。导出包会保留包内角色/人设之间的关系和记忆；未选对象相关的关系和记忆不会导出。`
- Group selection into `AI 角色` and `用户人设`.
- Clicking rows toggles checkbox selection instead of opening detail.
- Actions: `取消`, `导出 X 项`.

- [ ] **Step 3: Export warnings**

On export:
- Call `preview_agent_bundle_export`.
- If omissions exist, show `ConfirmDialog` with title `未包含的角色关系将会被忽略`.
- If user confirms, call `export_agent_bundle` with `confirmOmissions: true`.
- On success, show path: `已导出到 <path>`.

- [ ] **Step 4: Import preview modal**

Create `AgentBundleImportModal.svelte`.

UI:
- File summary: `AI 角色：N 个`, `用户人设：M 个`.
- Agent rows: avatar preview, editable name input, model dropdown.
- Batch control: `应用模型到全部 AI 角色`.
- User persona rows: avatar preview, editable name input.
- Do not show description, detailed persona, simplified persona, memory, or relationship fields.
- Confirm is allowed even when agent model is empty.

- [ ] **Step 5: Import flow**

In `AgentList.svelte`:
- `导入` opens file input accepting `.agentstage`.
- Read file text in frontend.
- Call `preview_agent_bundle_import`.
- Open modal.
- On confirm, call `import_agent_bundle`.
- Reload agents and user personas.
- Success toast: `已导入 X 个角色、Y 个用户人设。`
- If renames occurred, toast: `部分名称已自动调整，建议导入后检查并按需修改。`

- [ ] **Step 6: Verify**

Run:

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: no Svelte/TypeScript errors.

---

## Task 4: Docs and Final Verification

**Files:**
- Modify: `.gitignore`
- Modify: `docs/feature_list.md`

- [ ] **Step 1: Ignore exports**

Add to `.gitignore`:

```gitignore
/exports/
```

Do not remove existing entries.

- [ ] **Step 2: Update status**

If implementation and verification pass, update `AGT-20` in `docs/feature_list.md` from `📝 设计中` to `✅ 已实现`.

- [ ] **Step 3: Full verification**

Run:

```bash
cd src-tauri
cargo test db::agent_bundle::tests -- --nocapture
cargo check
```

Run:

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: all pass.

---

## Self-Review

Spec coverage:
- `.agentstage` single-file bundle: Tasks 1-2.
- Embedded avatars: Task 1.
- Export directory `<exe_dir>/exports/bundles/`: Task 1.
- Git ignore `/exports/`: Task 4.
- Agent + user persona export/import: Tasks 1 and 3.
- Relationship filtering and warnings: Tasks 1 and 3.
- Import preview editable names: Task 3.
- Optional model selection plus apply-to-all: Task 3.
- No temperature export: Task 1.
- No model config/API key export: Task 1.

Placeholder scan: no TBD/TODO placeholders.

Type consistency: command names and TypeScript names match plan text; Rust request/response fields should be serde-compatible with Tauri camelCase frontend arguments.
