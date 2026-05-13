# Pending Decisions

## 所有待确认需求已解决

**日期**: 2026-05-13

### 已解决事项

1. **CHAT-25: 消息上限重置按钮 — 自动触发群聊对话**
   - 方案：通过 Session Inbox 新架构实现。重置 `agent_message_count` 后，自动扫描并触发有未读消息的角色。
   - 状态：✅ 已纳入 CHAT-27 架构设计，无需单独实现。

2. **CHAT-27: Session Inbox 架构设计确认**
   - 方案：按 `session_id × agent_id` 维护未读消息层，上限冻结状态持久化到数据库。
   - 状态：✅ 已确认，进入开发阶段。

3. **PromptAssembler Layer 5 冗余问题**
   - 方案：去掉 Layer 5（`pending_messages`），统一按时间排序，添加 footer 引导语。
   - 状态：✅ 已确认，纳入 CHAT-28。

---

*此文档保留用于追溯历史决策。新需求请直接在 feature_list.md 中记录。*
