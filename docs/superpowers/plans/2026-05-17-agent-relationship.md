# AGT-14：角色关系设定及维护 — 实现计划

*日期：2026-05-17*
*对应设计：2026-05-17-agent-relationship-design.md*

---

## 前置条件

- `cargo check` 通过
- `cargo check --tests` 通过
- Vitest 全部通过
- 当前工作区干净

---

## 实施阶段

### Phase 1：数据库与后端基础（约 40min）

**目标**：建立数据层，确保关系数据能存取。

| # | 任务 | 验证方式 |
|---|------|---------|
| 1.1 | 在 `src-tauri/src/db/schema.rs` 中新增 Migration V12，创建 `agent_relationships` 表 + 索引 | `cargo check` 通过 |
| 1.2 | 在 `src-tauri/src/models/` 新建 `agent_relationship.rs`，定义 `AgentRelationship` 和 `RelationshipItem` DTO | `cargo check` 通过 |
| 1.3 | 在 `src-tauri/src/db/` 新建 `agent_relationship.rs`，实现：<br>- `get_relationship(conn, observer_id, target_id, target_type) -> Result<String>`<br>- `upsert_relationship(conn, observer_id, target_id, target_type, text) -> Result<()>`<br>- `list_relationships_by_observer(conn, observer_id) -> Result<Vec<RelationshipItem>>` | `cargo check` 通过；写手动 runner 验证 CRUD |
| 1.4 | 在 `delete_user_persona` 中增加级联删除 `agent_relationships` 逻辑 | `cargo check` 通过 |

**验证**：Migration V12 能正确执行；repository 函数能正确读写关系数据。

---

### Phase 2：Prompt 层集成（约 30min）

**目标**：让关系描述出现在 Prompt 中。

| # | 任务 | 验证方式 |
|---|------|---------|
| 2.1 | 修改 `src-tauri/src/llm/prompt.rs` 中的 `get_participants()`：<br>- 对每一个参与者，调用 `agent_relationship::get_relationship()` 获取关系描述<br>- 用户参与者使用当前激活的 `user_persona_id` 查询 | 检查 `cargo check` |
| 2.2 | 修改 `assemble()` 中 Layer 3 拼接逻辑：<br>- 关系描述为空时维持原格式<br>- 非空时追加 `。主观关系认定：{text}` | 检查 `cargo check` |
| 2.3 | 在 `prompt_templates.rs` 中新增 `RELATIONSHIP_SUFFIX_PREFIX` 常量 | `cargo check` 通过 |

**验证**：打印 Prompt 日志，确认有/无关系描述时的格式正确。

---

### Phase 3：Tool 层实现（约 30min）

**目标**：让 Agent 能自主更新关系。

| # | 任务 | 验证方式 |
|---|------|---------|
| 3.1 | 在 `src-tauri/src/llm/tool.rs` 中新增 `update_relationship_tool_schema()` | `cargo check` 通过 |
| 3.2 | 在 `ToolExecutor` 中新增 `execute_update_relationship()` 方法：<br>- 校验 200 字限制<br>- old_text 匹配校验<br>- 调用 repository 更新 | `cargo check` 通过 |
| 3.3 | 在 `ToolExecutor::execute()` 的 match 中增加 `"update_relationship"` 分支 | `cargo check` 通过 |
| 3.4 | 在 `PromptAssembler` 的工具列表注入中增加 `update_relationship` 工具 | `cargo check` 通过 |

**验证**：模拟 Tool Call，测试 old_text 匹配/不匹配、超长文本等边界。

---

### Phase 4：Tauri Commands（约 20min）

**目标**：暴露 API 给前端。

| # | 任务 | 验证方式 |
|---|------|---------|
| 4.1 | 新建 `src-tauri/src/commands/agent_relationship.rs`，实现 `list_agent_relationships` 和 `update_agent_relationship` | `cargo check` 通过 |
| 4.2 | 在 `src-tauri/src/lib.rs` 中注册两个新 command | `cargo check` 通过 |
| 4.3 | 前端 `src/lib/types.ts` 中新增 `RelationshipItem` 类型定义 | `npx svelte-check` 无报错 |

---

### Phase 5：前端重构 — AgentDetail 标签页（约 40min）

**目标**：重构角色详情页，支持标签页切换。

| # | 任务 | 验证方式 |
|---|------|---------|
| 5.1 | 重构 `AgentDetail.svelte`：<br>- 新增 `activeTab: 'config' \| 'relationships'` 状态<br>- 顶部横向标签页按钮 `[角色配置] [关系设定]`<br>- 原表单内容移入"角色配置"标签页 | 手动测试：标签页能正常切换 |
| 5.2 | 新建 `src/lib/components/AgentRelationshipPanel.svelte`：<br>- 接收 `agentId` prop<br>- 调用 `list_agent_relationships` 加载列表<br>- 按"用户→好友→群友"排序展示 | 手动测试：列表正确渲染 |
| 5.3 | 每行包含：头像、名称、标签 badge、输入框、字数统计 | 手动测试：输入框正常显示 |
| 5.4 | 输入框 debounce（500ms）或 blur 时调用 `update_agent_relationship` 自动保存 | 手动测试：保存后刷新确认 |
| 5.5 | 空状态处理：无关联对象时显示提示文案 | 手动测试：新建 agent 显示空状态 |
| 5.6 | 保存按钮位置调整："角色配置"标签页保留保存按钮；"关系设定"无保存按钮（自动保存） | 手动测试：行为正确 |

**验证**：在 UI 上验证标签页切换、关系列表加载、输入保存、空状态。

---

### Phase 6：联动与边界处理（约 20min）

**目标**：处理用户人设切换、删除等联动场景。

| # | 任务 | 验证方式 |
|---|------|---------|
| 6.1 | 用户切换人设后，重新进入 AgentDetail 的"关系设定"页，列表自动刷新（调用 `list_agent_relationships`） | 手动测试 |
| 6.2 | 删除用户人设后，验证 `agent_relationships` 表中对应记录被清除 | 数据库直接查询验证 |
| 6.3 | Agent 被删除后，验证其作为 observer 的关系记录被级联删除 | 数据库直接查询验证 |

---

### Phase 7：回归测试（约 15min）

| # | 任务 | 验证方式 |
|---|------|---------|
| 7.1 | `cargo check` 通过 | 命令行 |
| 7.2 | `cargo check --tests` 通过 | 命令行 |
| 7.3 | `npx svelte-check --tsconfig ./tsconfig.json` 通过 | 命令行 |
| 7.4 | Vitest 全部通过 | `pnpm test` |
| 7.5 | 端到端手动测试：创建 agent → 进入关系设定 → 编辑关系 → 创建群聊 → 发送消息 → 检查 backend.log 中 Prompt 是否正确注入关系描述 | 手动 |

---

## 风险与回退

| 风险 | 缓解措施 |
|------|---------|
| Prompt 长度暴增 | 关系描述限制 200 字；为空时不追加任何文本 |
| Agent 滥用 update_relationship | old_text 匹配机制防止误改；200 字限制；提示词强调"只更新整体定位" |
| 前端状态管理复杂 | 关系设定页独立加载，不依赖 agentStore 的复杂状态 |
| Migration 冲突 | V12 是全新表，无数据回填，低风险 |

---

## 预计总耗时

约 **3.5 小时**（纯编码时间，不含调试和 review）。

---

*计划结束*
