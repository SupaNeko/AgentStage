# Sticker Pack Import/Export and Chat Usage Design

## Goal

Add sticker pack management to AgentStage so users can create reusable sticker packs, import/export packs as self-contained files, assign available packs to agents, and let both users and agents send stickers in chat through `<sticker>packName_stickerName</sticker>` tags.

## Scope

This feature covers:

- Creating, renaming, deleting, importing, and exporting sticker packs.
- Adding static images and GIF stickers to a pack.
- Requiring every sticker pack and every sticker image to have a name.
- Rejecting `_` in user-created pack names and sticker names.
- Resolving imported name conflicts with `Name1`, `Name2`, `Name3` style suffixes, without underscores.
- Compressing newly added static sticker images by a user-selected ratio and storing only the final processed file.
- Supporting GIF add/import/export/rendering; GIF compression may fall back to preserving the original GIF file.
- Assigning multiple sticker packs to each agent.
- Injecting assigned stickers into the agent prompt.
- Rendering `<sticker>...</sticker>` tags inside existing message bubbles.
- Letting users insert sticker tags from the chat input area.
- Multi-select deleting stickers inside a pack.

This feature does not cover:

- Preserving original uncompressed sticker files.
- Guaranteeing historical sticker rendering after a sticker or pack is deleted or renamed.
- Changing the existing `<br/>` message bubble split behavior.
- Adding a separate rich-message schema for stickers.
- Enforcing a hard runtime limit on how many stickers an agent can use per reply.
- Exporting sticker packs together with agent bundles in the first version.

## Confirmed Product Decisions

- Exported sticker packs must be self-contained. A single exported file must include all sticker image bytes needed for import on another machine.
- Only the final compressed image is stored. Original files are discarded after processing.
- Pack names and sticker names must not contain `_`, because the sticker reference uses `_` as the separator between pack name and sticker name.
- Imported duplicate names are renamed with numeric suffixes without underscores, such as `猫1` and `猫2`.
- Messages store raw `<sticker>packName_stickerName</sticker>` text. The frontend resolves and renders the tag as an image.
- Deleting or renaming a sticker or sticker pack may make historical messages lose the corresponding sticker image. The frontend must render these as an invalid sticker label.

## Data Model

Add three tables through a new migration and include them in `BASE_SCHEMA`.

### `sticker_packs`

Stores sticker pack metadata.

Columns:

- `id TEXT PRIMARY KEY`
- `name TEXT NOT NULL`
- `created_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`
- `is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1))`
- `deleted_at INTEGER`

Indexes:

- Unique active pack name: enforce no duplicate active pack names. SQLite can use a partial unique index on `name WHERE is_deleted = 0`.
- Pack list index on `is_deleted, updated_at`.

Validation:

- `name.trim()` must not be empty.
- `name` must not contain `_`.
- User-created and user-renamed names must pass validation.
- Import conflict resolution must produce names that also pass validation.

### `stickers`

Stores sticker image metadata. The image bytes are stored on disk.

Columns:

- `id TEXT PRIMARY KEY`
- `pack_id TEXT NOT NULL REFERENCES sticker_packs(id) ON DELETE CASCADE`
- `name TEXT NOT NULL`
- `file_path TEXT NOT NULL`
- `mime_type TEXT NOT NULL`
- `width INTEGER NOT NULL`
- `height INTEGER NOT NULL`
- `file_size INTEGER NOT NULL`
- `created_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`
- `is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1))`
- `deleted_at INTEGER`

Indexes:

- Unique active sticker name per pack: partial unique index on `(pack_id, name) WHERE is_deleted = 0`.
- Sticker list index on `pack_id, is_deleted, created_at`.

Validation:

- `name.trim()` must not be empty.
- `name` must not contain `_`.
- A sticker belongs to exactly one pack.
- A sticker cannot be added to a deleted pack.

### `agent_sticker_packs`

Stores which packs an agent may use.

Columns:

- `agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE`
- `pack_id TEXT NOT NULL REFERENCES sticker_packs(id) ON DELETE CASCADE`
- `created_at INTEGER NOT NULL`
- `PRIMARY KEY (agent_id, pack_id)`

Behavior:

- `set_agent_sticker_packs` replaces the full set for one agent in a transaction.
- Deleted packs are ignored in prompt injection and frontend selection state.
- Saving the same set repeatedly is idempotent.

## File Storage

Sticker files are stored under the portable data directory:

```text
data/stickers/<pack_id>/<sticker_id>.<ext>
```

The database stores relative paths such as:

```text
stickers/<pack_id>/<sticker_id>.png
```

Supported formats:

- PNG
- JPEG
- GIF

Static image compression:

- The frontend may inspect the selected image and show the expected compressed dimensions for the selected compression ratio.
- The backend must decode and process the image again, because frontend dimensions are only advisory.
- The backend stores only the processed output file.
- The stored `width`, `height`, and `file_size` must reflect the final written file.
- `compressionRatio` must be a finite number where `0 < compressionRatio <= 1.0`.
- `compressionRatio = 1.0` keeps the original static-image dimensions.
- Values below `1.0` reduce static-image dimensions.
- Values `<= 0`, `> 1.0`, `NaN`, or infinite values are rejected with a clear error.

GIF handling:

- GIF files are accepted, stored, exported, imported, and rendered.
- First-version compression preserves GIF files unchanged.
- GIF uploads still must pass the same `compressionRatio` validation, but the accepted ratio is ignored for the stored GIF bytes.
- GIF width and height are read from the GIF logical screen descriptor bytes in the file header. Do not add the `image` crate `gif` feature for the first version.
- If implementation later supports GIF frame resizing, it must still store only the final processed GIF.

Deletion:

- Pack and sticker deletion are soft deletes in SQLite.
- First version may keep image files on disk after soft delete to avoid file cleanup races.
- Deleted stickers and packs must not appear in normal lists, prompt injection, user sticker picker, or valid sticker resolution.

## Backend Modules and Commands

Add:

- `src-tauri/src/models/sticker.rs`
- `src-tauri/src/db/sticker.rs`
- `src-tauri/src/commands/sticker.rs`

Register commands in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs`.

### Pack Management

`list_sticker_packs`

- Returns all undeleted packs and undeleted stickers.
- Used by the settings sticker manager, agent sticker-pack tab, chat sticker picker, and frontend sticker cache.
- Packs with zero undeleted stickers are still returned for management and agent assignment UI, because they are valid configurable packs.

`create_sticker_pack`

- Request: `{ name }`
- Creates an undeleted pack.
- Rejects empty names, names containing `_`, and duplicate active names.

`update_sticker_pack`

- Request: `{ id, name }`
- Renames a pack.
- Rejects invalid or duplicate active names.
- The frontend must warn that renaming can invalidate historical sticker tags.

`delete_sticker_pack`

- Request: `{ id }`
- Soft deletes the pack and its sticker rows in one transaction.
- Removes agent-pack assignments for that pack or leaves them ignored by joins; removing them is preferred.
- The frontend must warn that deletion can invalidate historical sticker tags.

### Sticker Management

`add_sticker_to_pack`

- Request: `{ packId, name, imageDataBase64, compressionRatio }`
- Validates pack exists and is undeleted.
- Validates sticker name.
- Accepts `compressionRatio` only when it is finite and `0 < compressionRatio <= 1.0`.
- Applies `compressionRatio` to PNG/JPEG images.
- Ignores `compressionRatio` for GIF storage after validation and stores the GIF bytes unchanged.
- Rejects unsupported image types.
- Writes the final image file to `data/stickers/<pack_id>/`.
- Inserts metadata into `stickers`.

`update_sticker`

- Request: `{ id, name }`
- Renames one sticker.
- Rejects invalid or duplicate names inside the same active pack.
- The frontend must warn that renaming can invalidate historical sticker tags.

`delete_stickers`

- Request: `{ ids }`
- Supports multi-select deletion.
- Soft deletes every undeleted matching sticker in one transaction.
- The frontend must warn that deletion can invalidate historical sticker tags.

### Agent Assignment

`list_agent_sticker_packs`

- Request: `{ agentId }`
- Returns assigned undeleted pack IDs for the agent.

`set_agent_sticker_packs`

- Request: `{ agentId, packIds }`
- Replaces the full assignment set in one transaction.
- Ignores or rejects deleted/nonexistent packs; rejecting with a clear error is preferred for predictable UI.

### Sticker Resolution

`resolve_sticker_refs`

- Request: `{ refs }`, where refs are strings like `猫_可爱`.
- Response returns one item per requested ref:
  - `ref`
  - `status: "valid" | "invalid"`
  - for valid refs: `packId`, `stickerId`, `filePath`, `mimeType`, `width`, `height`
- Resolution only succeeds when both pack and sticker are undeleted and the ref matches `pack.name + "_" + sticker.name`.
- The frontend should normally resolve refs from the local sticker index loaded by `list_sticker_packs`.
- The frontend may batch unresolved or cache-miss refs through `resolve_sticker_refs`, especially after stale data, import/export refresh boundaries, or future partial-loading optimizations.

## Sticker Pack Bundle

Sticker packs are exported one pack at a time.

Recommended file extension:

```text
.agentsticker
```

The file is JSON and self-contained.

Top-level structure:

```json
{
  "format": "agentstage.sticker_pack",
  "version": 1,
  "exported_at": 1780000000000,
  "pack": {
    "name": "猫",
    "stickers": [
      {
        "name": "可爱",
        "mime_type": "image/png",
        "width": 256,
        "height": 256,
        "file_size": 12345,
        "base64_content": "..."
      }
    ]
  }
}
```

`export_sticker_pack`

- Request: `{ packId }`
- Reads the undeleted pack and undeleted stickers.
- Reads each sticker file from disk and encodes it as base64.
- Writes the bundle under the app export directory, such as `exports/stickers/`.
- Returns the exported path and warnings.

`import_sticker_pack`

- Request: `{ fileContent }`
- `fileContent` is the raw JSON text of a `.agentsticker` file, not base64.
- The first frontend implementation should use a browser file input and `await file.text()` to read the file content before invoking the command.
- Native Tauri file dialogs and drag-and-drop are optional future UI improvements, not required for the first implementation.
- Parses and validates JSON.
- Validates `format` and `version`.
- Validates all names and image payloads.
- Resolves pack name conflicts with `Name1`, `Name2`, `Name3`.
- Resolves sticker name conflicts inside the imported pack with the same no-underscore numeric suffix rule.
- Writes imported image files to a new local pack directory.
- Inserts pack and sticker metadata in one transaction where possible. If file writes fail, the command should not leave active DB rows pointing to missing files.

## Prompt Injection

For each agent call, query the agent's assigned undeleted packs and their undeleted stickers.

Inject `【可用的表情】` only when the agent has at least one available sticker.
Assigned packs with zero undeleted stickers must be excluded from the prompt section. If all assigned packs are empty, the section is omitted.

The same sticker prompt section should be added to:

- `PromptAssembler` for normal chat.
- `HistoryPromptAssembler` for history-mode replies.

Prompt text:

```text
【可用的表情】
你可以在回复消息中携带表情。表情不会替代文字内容，而是作为聊天中的情绪补充。

使用格式：
在回复内容中直接写入 <sticker>包名_表情名</sticker> 标签即可。系统会把该标签渲染成对应表情图片。
例如：早上好<sticker>猫_可爱</sticker>

使用建议：
- 如果你要使用表情，建议把 <sticker>...</sticker> 放在整条回复的开头或结尾。
- 一次回复中尽量最多只使用一个表情。
- 不要过于频繁地使用表情，只有在能自然增强语气、情绪或角色表现时再使用。
- 不要使用列表外的表情。
- 不要修改包名或表情名。
- 不要在标签内容里添加额外空格。
- 不要把表情标签拆开输出。

可用表情：
- 小黄人
  - 大哭：<sticker>小黄人_大哭</sticker>
  - 大笑：<sticker>小黄人_大笑</sticker>
- 猫
  - 可爱：<sticker>猫_可爱</sticker>
```

Backend message handling:

- Do not parse stickers when saving messages.
- Continue splitting messages only through existing `<br/>` handling.
- Tool-call content and fallback model content both preserve raw sticker tags.

## Frontend Rendering

Messages continue to carry plain `content: string`.

Add a sticker parser utility that recognizes complete tags:

```text
<sticker>packName_stickerName</sticker>
```

Parsing rules:

- Only complete tags are parsed.
- The inner text must contain exactly one `_`.
- Empty pack names or sticker names are invalid.
- Text outside sticker tags remains normal text.
- Invalid syntax remains normal text.
- Valid syntax that cannot resolve to an undeleted sticker renders as an invalid sticker label.

`MessageBubble.svelte` should render parsed content parts:

- Text parts as text.
- Valid sticker parts as images.
- Invalid resolved sticker parts as an invalid sticker label.

Sticker tags do not create separate bubbles. A message containing text and stickers remains one bubble. Existing `<br/>` splitting remains controlled by backend message creation.

Add `src/lib/stores/stickerStore.svelte.ts`:

- Loads available sticker packs and sticker metadata.
- Resolves refs through the local index loaded from `list_sticker_packs`.
- Optionally batches local cache misses through `resolve_sticker_refs` before rendering them as invalid.
- Refreshes after pack/sticker create, update, delete, import, or export.

## Settings Sticker Manager

Add a "Sticker Pack Configuration" entry in personal settings.

Capabilities:

- Create pack.
- Rename pack.
- Delete pack with warning about historical sticker loss.
- Add sticker image with required name.
- Show selected compression ratio and expected static-image dimensions before upload.
- Multi-select stickers for deletion.
- Rename a sticker with warning about historical sticker loss.
- Call `update_sticker` for sticker rename.
- Export a pack.
- Import a pack.
- Import reads raw JSON text from a selected `.agentsticker` file and passes that text as `fileContent`.

UI placement only needs to be defined at a functional level. Detailed visual styling is not part of this spec.

## Agent Sticker-Pack Tab

Add a "Sticker Packs" tab after:

- Role Configuration
- Relationship Settings
- Memory
- Timed Tasks

The tab uses a sticker-pack cover grid:

- Load all undeleted sticker packs.
- Load the current agent's assigned pack IDs.
- Each grid cell represents one sticker pack.
- The cover image uses the first undeleted sticker in that pack.
- Packs with no stickers show an empty or placeholder cover state and remain selectable.
- Clicking a grid cell toggles selected/unselected state.
- Entering the tab marks currently assigned packs as selected.
- Clicking confirm/save calls `set_agent_sticker_packs` with all selected pack IDs.
- The selection affects future prompt injection only and does not modify historical messages.

## Chat Input Sticker Picker

Add a sticker picker entry next to the chat input.

Behavior:

- Shows all configured undeleted sticker packs and undeleted stickers, not only the packs assigned to the current agent.
- Agent pack assignment controls only what the agent may use in prompt injection. It does not restrict the user's manual sticker picker.
- Clicking a sticker inserts `<sticker>packName_stickerName</sticker>` into the current input at the cursor position.
- If cursor insertion is difficult in the first implementation, appending to the current input is acceptable, but cursor insertion is preferred.
- The sent message stores the raw tag.
- User-inserted sticker tags appear in future history prompt context like any other message text.

## Historical Message Behavior

Historical messages store sticker tags by name, not by sticker ID.

Consequences:

- Deleting a sticker can make older messages with that tag lose the image.
- Deleting a sticker pack can make older messages with that pack lose images.
- Renaming a sticker or pack can make older messages with the old name lose images.
- These operations must show warnings before confirmation.
- The frontend renders unresolved historical sticker refs as an invalid sticker label.

This tradeoff keeps the message schema unchanged and avoids turning messages into rich content records.

## Testing

### Rust Tests

Migration and repository tests:

- New tables and indexes exist in `BASE_SCHEMA`.
- Creating a pack succeeds with a valid name.
- Creating a pack fails for empty names, names containing `_`, and duplicate active names.
- Creating a sticker succeeds for a valid pack and valid name.
- Creating a sticker fails for deleted packs, empty names, names containing `_`, and duplicate active names within a pack.
- `set_agent_sticker_packs` is idempotent when called repeatedly with the same pack IDs.
- Deleted packs and stickers are excluded from list results.
- `resolve_sticker_refs` returns valid items for active pack/sticker pairs.
- `resolve_sticker_refs` returns invalid items for deleted packs, deleted stickers, malformed refs, and unknown names.

Import/export tests:

- Exported bundle includes `format`, `version`, pack metadata, sticker metadata, and base64 image content.
- Import recreates pack and sticker metadata and writes files.
- Import duplicate pack names become `Name1`, `Name2`.
- Import duplicate sticker names inside the target pack become `Name1`, `Name2`.
- Import rejects bundle names containing `_`.

Prompt tests:

- Normal prompt includes `【可用的表情】` when the agent has assigned active stickers.
- Normal prompt omits the section when the agent has no assigned active stickers.
- Prompt includes only assigned packs for the current agent.
- Prompt excludes deleted packs and stickers.
- History prompt follows the same inclusion and exclusion rules.

### Frontend Tests

Parser and rendering tests:

- Pure text renders as text.
- Text plus one sticker renders text and image in one bubble.
- Multiple sticker tags in one content string parse into multiple sticker parts.
- Unresolved refs render as an invalid sticker label.
- Malformed tags remain text.

Interaction tests:

- Sticker picker inserts the exact `<sticker>packName_stickerName</sticker>` tag.
- Multi-select sticker deletion sends all selected sticker IDs.
- Agent sticker-pack tab loads selected and unselected states correctly.
- Agent sticker-pack tab toggles selection and saves the selected pack ID set.
- Settings manager refreshes sticker data after create, update, delete, import, and export.

## Risks and Mitigations

History invalidation:

- Risk: name-based references break after delete or rename.
- Mitigation: show warnings and render invalid sticker labels for unresolved refs.

Prompt size:

- Risk: assigning many large packs increases prompt length.
- Mitigation: first version injects all assigned sticker names; future work can add per-agent limits or usage ranking.

Large bundle files:

- Risk: base64 image data increases file size and memory use.
- Mitigation: validate image count, image size, and bundle parse errors with clear user-facing messages.

GIF processing:

- Risk: GIF resizing is more complex than static image resizing.
- Mitigation: first version supports GIF storage/render/import/export and allows compression to fall back to original GIF.

Filesystem consistency:

- Risk: DB rows can point to missing files after partial import/write failure.
- Mitigation: write files before inserting active rows where possible, use transactions for DB mutations, and surface import errors.

## Implementation Boundaries

Keep this feature independent from existing agent bundle import/export.

Do not modify the `messages` schema.

Do not move LLM calls to the frontend.

Do not change the existing frontend `invoke()` camelCase parameter rule.

Use Tauri commands through the Rust backend for all file, import/export, and image processing operations.
