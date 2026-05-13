# Pending Decisions

## CHAT-25: 消息上限重置按钮 — 未实现子需求

**日期**: 2026-05-13

### 背景
在实现 CHAT-25（消息上限重置按钮）时，需求文档中提到：
> 重新触发一次群聊对话（遵循CD）

### 当前状态
已实现：
- 当 `agent_message_count >= message_limit` 且 `message_limit_enabled` 为 true 时，ChatView 显示提示条"已达到消息上限，角色不再主动回复"和"重置限制"按钮。
- 点击"重置限制"按钮后，后端将对应 session 的 `agent_message_count` 重置为 0，并刷新前端配置状态。

**未实现**：
- 重置计数器后，**自动触发一次群聊对话**（即手动调用 Scheduler 触发角色回复）。

### 原因
手动触发 Scheduler 过于复杂，需要：
1. 确定要触发哪些角色参与回复
2. 遵循 CD（Conversation Driver）逻辑，可能涉及多个角色的轮询/调度
3. 可能需要新的 Tauri Command 或内部 API 来直接调用 Scheduler 的调度逻辑
4. 需要区分私聊和群聊的不同触发方式

### 后续行动
- 需要产品/架构确认：重置后自动触发对话是否是必须行为？
- 如果需要，建议单独开一个 ticket 来设计 Scheduler 的手动触发接口
- 当前实现已满足核心需求（让用户可以重置消息限制，使角色可以继续主动回复）
