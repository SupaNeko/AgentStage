# 会话管理、头像上传与人设自生成 UI 设计文档

**日期**: 2026-05-16
**范围**: 功能1(群聊解散/彻底删除/配置简化) + 功能2(Agent-Agent历史禁用输入) + 功能3(头像上传) + 功能4(人设自生成UI占位)

---

## 1. 群聊解散与彻底删除

### 1.1 群聊解散

**问题**: 当前 `disband_group` 把 `sessions.is_deleted` 设为 1，导致群聊从历史记录中消失。

**方案**: 引入独立的 `is_dissolved` 状态字段。

- **Migration V9**: `ALTER TABLE group_sessions ADD COLUMN is_dissolved INTEGER DEFAULT 0`
- `disband_group` 改为 `UPDATE group_sessions SET is_dissolved = 1`（不动 `sessions.is_deleted`）
- `SessionResponse` / `Session` / `GroupSession` 增加 `is_dissolved: bool`
- `list_sessions` SQL 返回 `is_dissolved`
- `send_user_message` / `send_history_message` 检查 `is_dissolved`，拒绝发送并返回错误

**前端行为**:
- `sessionStore` 过滤掉 `is_dissolved` 群聊（当前列表不显示）
- `historyStore` 保留 `is_dissolved` 群聊
- `ChatView` 中 `isDissolved` 时禁用输入区，显示"该群聊已解散，无法发送消息"
- `SessionSettingsPanel` 群聊模式下保留解散按钮和二次确认

### 1.2 历史会话中彻底删除

**机制**: 复用已有的 `delete_session` 软删除命令。

**前端行为**:
- `HistorySessionList.svelte` 中群聊标签增加右键菜单
- 菜单项"彻底删除"，二次确认后调用 `delete_session`
- 彻底删除后该群聊从历史列表中消失（`is_deleted = 1`）

### 1.3 历史会话配置简化

`SessionSettingsPanel.svelte` 新增 `mode: 'chat' | 'history'` prop。

**历史模式下的群聊配置**:
- 隐藏禁言开关
- 成员管理仅显示成员列表，隐藏移除按钮和"添加成员"按钮
- 隐藏"重置群聊"按钮
- 隐藏"解散群聊"按钮

**历史模式下的私聊配置**:
- 隐藏禁言开关
- 隐藏"重置会话"按钮

---

## 2. Agent-to-Agent 历史会话禁用输入

**当前**: `ChatView` 在 chat 模式下已禁用 Agent-Agent 输入。

**新增**: `mode === 'history' && isAgentAgentPrivate` 时同样禁用输入，显示提示文案。

---

## 3. 头像上传

### 3.1 后端

新增 `upload_avatar` Tauri 命令:

```rust
#[tauri::command]
pub async fn upload_avatar(
    state: State<'_, DbState>,
    req: UploadAvatarRequest,
) -> Result<String, String>
```

- `req.target_type`: `"user" | "agent" | "group"`
- `req.target_id`: 对应 ID
- `req.image_data_base64`: base64 编码的图片数据

**处理逻辑**:
1. 解码 base64，确定图片格式（png/jpg/webp）
2. 保存到 `data/avatars/{target_type}/{target_id}.{ext}`
3. 更新对应表的 `avatar_path` 字段
4. 返回保存后的文件路径

### 3.2 前端

**Agent 头像**:
- `AgentDetail.svelte`: 头像区域点击 → 弹出头像管理窗口（查看大图 + 上传新头像）
- `CreateAgentModal.svelte`: 同上

**群聊头像**:
- `SessionSettingsPanel.svelte` (群聊模式): 增加"更换群聊头像"按钮 → 上传弹窗

**用户头像**:
- `SettingsPanel.svelte`: 增加用户头像区域 + 上传按钮

**头像弹窗组件** `AvatarUploadModal.svelte`:
- 显示当前头像大图
- `<input type="file" accept="image/*">` 选择图片
- FileReader 读取为 base64
- 调用 `upload_avatar` 上传

---

## 4. 人设自生成 UI（仅占位）

**AgentDetail.svelte**:
- 增加"人设自生成"按钮
- 点击弹出模态窗口（空窗口，仅标题和关闭按钮，功能后续实现）

**CreateAgentModal.svelte**:
- 增加"人设自生成"按钮
- 点击时展开/折叠以下内容：
  - "参考角色"输入框
  - "补充信息"输入框
  - "生成"按钮（禁用状态）

---

## 5. 暂存需求

**Page 级成员快照**: 每次 `reset_session` 时保存当前成员列表到 `page_member_snapshots` 表，历史模式下按 page 显示对应成员。实现复杂度高，本次不做，记入 feature_list.md。

---

## 实现顺序

1. 功能1（群聊解散/彻底删除/配置简化）
2. 功能2（Agent-Agent 历史禁用输入）
3. 功能3（头像上传）
4. 功能4（人设自生成 UI）
