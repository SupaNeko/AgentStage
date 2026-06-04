# Agent and User Persona Bundle Import/Export Design

## Goal

Add a dedicated import/export workflow for AgentStage configuration bundles. A bundle can contain AI agents, user personas, embedded avatars, and relationships/memory between included objects. The bundle is a single `.agentstage` file saved under the executable directory's bundle export folder.

## Scope

This feature covers:

- Exporting one or more AI agents.
- Exporting one or more user personas.
- Exporting embedded avatar assets for exported agents and user personas.
- Exporting relationships and memory only when both ends are included in the bundle.
- Importing agents and user personas from a `.agentstage` bundle.
- Rebuilding package-internal agent friendships, relationship descriptions, and memory during import.
- Allowing the user to choose existing local model configurations for imported AI agents.

This feature does not cover:

- Exporting model configurations.
- Exporting API keys.
- Editing detailed personas, simplified personas, relationship text, memory text, or friendships in the import preview screen.
- Automatically activating imported user personas.
- Exporting agent temperature overrides. Temperature is treated as model-bound configuration and is reset to empty during import.
- Exporting chat history or sessions.

## Bundle File

The exported file uses the `.agentstage` extension. It is parsed as JSON internally.

Recommended top-level structure:

```json
{
  "format": "agentstage.bundle",
  "version": 1,
  "exported_at": 1780000000000,
  "agents": [],
  "user_personas": [],
  "relationships": [],
  "friendships": [],
  "assets": []
}
```

All object IDs inside the bundle are bundle-local strings and must be unique across the entire bundle. IDs use type prefixes:

- Agent: `agent:<uuid-or-original-id>`
- User persona: `user_persona:<uuid-or-original-id>`
- Asset: `asset:<uuid>`

The importer must reject bundles with duplicate bundle-local IDs.

### Agents

Each exported agent includes non-model, non-secret role configuration:

- Original bundle-local ID
- Name
- Avatar asset reference
- Detailed persona
- Simplified persona
- Personality/scenario/example messages/first message/creator notes/tags
- Long-term memory
- Memory enabled flag
- Proactive session settings

Agent export intentionally excludes:

- `model_config_id`
- model provider/name/base URL/parameters
- API key or encrypted API key
- temperature override

Agent long-term memory is stored on the agent object itself as `agents[].long_term_memory`. This is distinct from relationship-level memory stored in `relationships[].memory_text`.

### User Personas

Each exported user persona includes:

- Original bundle-local ID
- Name
- Description
- Avatar asset reference

User persona descriptions are included in the bundle and restored during import, but they are not displayed or edited in the import preview UI.

### Relationships

Each `relationships[]` item represents one observer-to-target relationship or memory row from `agent_relationships`.

```json
{
  "observer_id": "agent:<id>",
  "target_id": "agent:<id> or user_persona:<id>",
  "target_type": "agent",
  "relationship_text": "主观关系描述",
  "memory_text": "关系层面的对他人记忆"
}
```

Rules:

- `observer_id` always references an included agent.
- `target_id` references an included agent or included user persona.
- `target_type` is retained for readability and must match the target ID prefix.
- `relationship_text` stores the relationship description.
- `memory_text` stores relationship-level memory only. It does not include `agents[].long_term_memory`.

### Friendships

Each `friendships[]` item represents a friendship between two included agents.

```json
{
  "agent_1_id": "agent:<id>",
  "agent_2_id": "agent:<id>"
}
```

Friendships do not carry relationship descriptions or memory. Descriptions and memory are stored only in `relationships[]`.

### Assets

Avatar assets are embedded in the bundle as base64. Each asset records:

- Asset ID
- Original relative path if available
- MIME type or inferred extension
- Base64 content

Agents and user personas reference avatars through `avatar_asset_id`. Import writes embedded avatar files into the local app avatar storage and updates the imported object's `avatar_path`.

## Relationship Filtering Rules

The bundle treats both AI agents and user personas as exportable objects.

Export a relationship or memory only if both ends are included:

- Agent A and Agent B are included: export A/B relationship descriptions and memory.
- Agent A and User Persona U are included: export A -> U relationship descriptions and memory.
- Agent A is included but Agent C is not: do not export A -> C relationship or memory.
- User Persona U is included but no related agent is included: export only U itself.

Export agent friendships only when both agents are included.

If export detects relationships, relationship-level memory, or friendships that point to objects not included in the bundle, it must warn before writing the file. This warning never refers to agent long-term memory because selected agents always keep their own long-term memory in `agents[].long_term_memory`.

Warning dialog:

- Title: `未包含的角色关系将会被忽略`
- Content example: `有 2 条关系描述、3 条关系记忆、1 条好友关系指向未选对象，导出包不会包含这些内容。`
- Buttons: `取消` / `仍然导出`

## Export Directory

The backend writes exported files to:

```text
<exe_dir>\exports\bundles\
```

In development mode this resolves to the project root equivalent:

```text
D:\code_project\AgentStage\exports\bundles\
```

The `exports/` directory must be ignored by Git so local exported bundles are not committed.

After export succeeds, the UI tells the user the file path:

```text
已导出到 D:\...\exports\bundles\xxx.agentstage
```

## Export UI

The export entry should live in the role/persona management area. At minimum, the role list page should expose:

- `导入`
- `导出`
- `新建`

Clicking `导出` enters selection mode.

Selection mode:

- Header title: `选择要导出的配置`
- Help text: `可以同时导出多个角色和用户人设。导出包会保留包内角色/人设之间的关系和记忆；未选对象相关的关系和记忆不会导出。`
- Sections:
  - `AI 角色`
  - `用户人设`
- Each item has a checkbox.
- Clicking a row toggles selection instead of opening details.
- The UI shows `已选择 X 项`.
- Actions: `取消` / `导出 X 项`.

The export UI should not show detailed persona, relationship, or memory contents.

## Import UI

Clicking `导入` opens a file picker for `.agentstage` files. The file is parsed and validated before anything is written to the database.

The import preview page or modal shows:

- Title: `导入角色配置`
- Summary:
  - `AI 角色：N 个`
  - `用户人设：M 个`

Agent rows show:

- Avatar preview
- Editable name input
- Existing local model configuration dropdown

The agent section also provides a batch action:

- `应用模型到全部 AI 角色`

This lets the user select one local model configuration and apply it to every imported AI agent in the preview. Individual rows can still be changed afterward.

User persona rows show:

- Avatar preview
- Editable name input

The preview screen does not show or allow editing:

- Detailed persona
- Simplified persona
- User persona description
- Long-term memory
- Relationship text
- Memory text
- Friendships

Those details can be edited after import in the existing agent/user persona screens.

## Import Behavior

Import has two phases:

1. Parse/preview: validate bundle and compute suggested names.
2. Confirm: write imported objects with user-confirmed names and selected model configurations.

On confirm:

- Create new IDs for every imported agent and user persona.
- Use the names from the preview page.
- Use a shared name namespace across agents and user personas. An imported agent cannot keep a name already used by an existing user persona, and an imported user persona cannot keep a name already used by an existing agent.
- If the preview default name collides with any existing agent or user persona, auto-fill a renamed value such as `Alice (导入)` or `Alice (导入 2)`.
- Show the rename in the preview page and allow the user to change it.
- Allow every imported AI agent to select a local model configuration, but do not require it.
- If no local model configurations exist, the model dropdowns show an empty state such as `暂无可用模型，导入后可在角色配置中选择模型`, and import remains allowed.
- Imported agent `model_config_id` is set to the selected local model configuration when one is chosen. If none is selected, `model_config_id` is `NULL`.
- Imported agent temperature override is always `NULL` because temperature is not exported and is considered model-bound configuration.
- Write embedded avatar assets locally and set `avatar_path`.
- Rebuild package-internal:
  - Agent-Agent friendships
  - Agent-Agent relationship descriptions and memory
  - Agent-UserPersona relationship descriptions and memory
- Do not automatically activate imported user personas.

Import success message:

```text
已导入 3 个角色、1 个用户人设。
```

If objects were auto-renamed, show an additional concise notice recommending review, for example:

```text
部分名称已自动调整，建议导入后检查并按需修改。
```

## Backend Design

Add a focused backend module for bundle logic. It should not be mixed into the existing `agent.rs` or `agent_relationship.rs` repositories.

Responsibilities:

- Build export preview/warnings from selected object IDs.
- Serialize bundle JSON.
- Resolve avatar paths and embed base64 assets.
- Parse and validate bundle files.
- Compute import preview names.
- Import bundle with a mapping from original bundle IDs to newly created DB IDs.

Tauri commands:

- `preview_agent_bundle_export`
  - Input: selected agent IDs and user persona IDs.
  - Output: counts and warnings.
- `export_agent_bundle`
  - Input: selected IDs plus confirmation flag.
  - Output: exported file path.
- `preview_agent_bundle_import`
  - Input: `.agentstage` file content as a string.
  - Output: agents/personas with suggested names and avatar previews.
- `import_agent_bundle`
  - Input: `.agentstage` file content as a string, final names, and optional per-agent `model_config_id`.
  - Output: imported agent/persona counts and rename notices.

The exact split may be adjusted during implementation, but the two-phase import behavior must remain.

All import preview commands receive file content as a string, not a file path. The frontend reads the selected `.agentstage` file and sends its text content to the backend. This avoids backend file-read permission ambiguity and keeps the file picker responsibility in the frontend.

## Error Handling

Export errors:

- Empty selection: show `请选择至少一个角色或用户人设`.
- No exportable selected objects: show `没有可导出的配置`.
- Avatar file missing: export continues without that avatar and records a warning.
- File write failure: show the OS error and do not claim success.

Import errors:

- Wrong extension or invalid JSON: show `导入文件格式错误`.
- Unsupported `format` or `version`: show `不支持的导入文件版本`.
- Missing required fields: show `导入文件缺少必要字段`.
- No local model configurations exist: allow import; imported agents have no model selected.
- No local model selected for an imported agent: allow import; that imported agent has no model selected.
- Avatar decode/write failure: import continues for text configuration and records a warning.

Avatar write behavior:

- Imported avatar files are always written using new UUID-based filenames.
- Existing avatar files are never overwritten.
- If a filename collision somehow occurs, generate another UUID filename and retry.

## Testing

Backend tests:

- Exporting A/B from A/B/C only includes A/B internal relationship and memory.
- Exporting A/B/UserPersona includes Agent-Agent and Agent-UserPersona relationship/memory.
- Export warns when selected agents have relationships or memory pointing to unselected objects.
- Import creates new IDs and rebuilds internal relationships/memory using the new IDs.
- Import rebuilds internal friendships using the new IDs.
- Import does not set imported user personas as active.
- Import applies supplied model config IDs to agents when selected.
- Import allows agents with no selected model config.
- Import resets agent temperature override to empty.
- Import auto-renames on name collision.

Frontend checks:

- Export selection mode shows helper text and grouped AI/user persona items.
- Export warning dialog uses title `未包含的角色关系将会被忽略`.
- Import preview allows editing names and choosing agent models.
- Import preview provides `应用模型到全部 AI 角色`.
- Import preview does not expose detailed persona, memory, or relationship editors.

## Open Decisions

No open decisions remain. The chosen approach is a single `.agentstage` JSON bundle with embedded base64 avatars, no exported model configuration, optional import-time model assignment, and no exported temperature override.
