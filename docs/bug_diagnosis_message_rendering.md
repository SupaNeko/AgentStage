# 消息系统 Bug 诊断与修复文档

> 日期：2026-05-10
> 调查范围：前端消息渲染、Scheduler 触发机制、Agent 自我触发、输入中提示

---

## 一、问题汇总

| # | 问题描述 | 严重程度 |
|---|---------|---------|
| 1 | **前端不显示 Agent 回复**：第二次发送消息后，Agent 已回复（后端日志确认），但前端未渲染；发送第三条消息时才突然显示上一条回复 | 高 |
| 2 | **第三次发送无回复**：超过 30s 间隔后发送第三条消息，后端日志显示 `is_triggering=true, skip`，Agent 无响应 | 高 |
| 3 | **"正在输入中"提示缺失**：流式/异步回复过程中无 UI 反馈 | 中 |

---

## 二、根因分析

### Bug 2（核心阻塞问题）：`is_triggering` 标志未清除

**证据链：**
```
22:38:35.354 - trigger_agent 开始，pending_messages=2
22:38:52.362 - ToolExecutor 成功写入消息
[日志到此中断，无 agent_completed、无 clear_triggering_flag 记录]

22:41:55.946 - try_trigger_agent: is_triggering=true, skip
```

**分析：**
- `trigger_agent` 在 `ToolExecutor` 成功后的后续步骤（更新计数器 / update_trigger_time / clear_triggering_flag）中某处失败或阻塞
- 失败后函数提前返回 `Err`，但 `is_triggering=1` 没有被重置
- 此后该 Agent 被永久锁定，`try_trigger_agent` 永远 `skip`

**根因假设：**
- `trigger_agent` 内部使用多个 `.map_err(|e| e.to_string())?`，任何一步失败都会导致提前返回，且没有 `finally` 机制保证 `is_triggering` 被清除
- `conn.execute("UPDATE private_sessions ...", [&msg.session_id])` 的参数传递或 SQL 执行可能存在隐蔽失败

### Bug 1（前端渲染问题）：消息事件不可靠

**证据链：**
```
22:38:10 - 用户发送第二条消息
22:38:15 - 用户切换到 AgentDetail 视图
22:38:29 - 用户切回 ChatView
22:38:52 - Agent 回复，后端 emit new_message
[前端日志中无 ChatView.listen new_message 记录]

22:41:55 - 用户发送第三条消息
22:41:55 - loadMessages count=5（包含第二条 Agent 回复）
```

**分析：**
- 用户切换视图导致 `ChatView` 卸载并重新挂载
- 虽然 `onMount` 重新注册了 `listen('new_message')`，但事件驱动模型本身不可靠（事件可能在监听器注册前到达，或因为 WebView 状态问题丢失）
- 前端只有 `new_message` 这一种消息获取途径，没有兜底刷新机制

**根因：**
- 缺乏事件丢失的兜底方案：当 `agent_completed` 或 `new_message` 到达时，前端应该刷新消息列表作为保险
- `App.svelte` 的 `new_message` 监听器在当前会话时不执行任何操作

### 设计缺陷：Agent 自我触发

**证据链：**
```
22:38:35.354 - trigger_agent pending_messages=2
```

**分析：**
- 私聊中 Agent 回复后，`trigger_agent` 将回复消息推入 `target_agent_id` 的 `pending_queue`
- 对于私聊，`target_agent_id` 就是该 Agent 自己
- 结果：Agent 的回复被推回自己的 pending_queue，形成自我触发循环
- `pending_messages=2` 说明队列中积压了 Agent 自己的上一条回复 + 用户的新消息
- 这导致 Prompt 越来越长，且可能快速耗尽消息上限

**根因：**
- `trigger_agent` 阶段 7 的触发链逻辑未区分"谁是接收方"，无脑将消息推入 `target_agent_id` 的 queue
- Phase 1（私聊）中，Agent 回复的接收方是 user，user 不应被自动触发

### 需求遗漏："正在输入中"提示

- 产品需求中未明确记录该功能，但在交互上属于基础 IM 体验
- 实现成本低（利用已有的 `agent_triggered` / `agent_completed` 事件即可）

---

## 三、修复方案

### 修复 A：防御性 `finally` 模式（解决 Bug 2）

将 `trigger_agent` 重构为：
```rust
async fn trigger_agent(&self, agent_id: &str) -> Result<(), String> {
    // 1. 原子取出 pending
    // 2. 设置 is_triggering=1
    
    let result = self.trigger_agent_inner(agent_id, pending).await;
    
    // 无论成功与否，总是清除 is_triggering
    let _ = self.clear_triggering_flag(agent_id).await;
    
    result
}
```

同时把 `update_trigger_time` 的 `ON CONFLICT DO UPDATE` 也改为更新 `is_triggering=0`，作为双重保险。

### 修复 B：停止 Agent 自我触发（解决 pending 累积）

在 `trigger_agent` 阶段 7：
```rust
// 不要把 Agent 自己的回复推回自己的 pending_queue
if let Some(target_agent_id) = target_agent_id {
    if target_agent_id != agent_id {
        // 只有回复给其他 Agent（群聊场景）时才推入 queue
        let mut queue = self.pending_queue.lock().await;
        queue.entry(target_agent_id)
            .or_insert_with(Vec::new)
            .push(PendingMessage::from(msg.clone()));
    }
}
```

### 修复 C：前端兜底刷新（解决 Bug 1）

1. `App.svelte` 监听 `agent_completed` 事件，收到后调用 `messageStore.loadMessages`
2. `App.svelte` 的 `new_message` 监听器在当前会话时也刷新消息列表（避免重复添加）
3. `ChatView.svelte` 的 `new_message` 监听器添加去重逻辑（通过 message id 判断）

### 修复 D：正在输入中提示

后端：
- `trigger_agent` 开始时 emit `agent_typing` 事件
- `trigger_agent` 结束时 emit `agent_completed`（已有）

前端：
- `ChatView.svelte` 监听 `agent_typing`，在当前会话时显示"正在输入中..."
- 监听 `agent_completed` 或 `new_message`，隐藏提示

---

## 四、验收标准

- [ ] 连续发送多条消息，每条都能在 30s 内（满足触发条件时）收到 Agent 回复
- [ ] 切换视图后返回 ChatView，Agent 回复仍然正确显示
- [ ] `is_triggering` 标志在异常情况后能被正确清除（通过 finally 模式）
- [ ] Agent 不会自我触发（pending_messages 不会包含 Agent 自己的回复）
- [ ] 前端显示"正在输入中..."提示，在 Agent 回复完成后消失
- [ ] 支持无限多轮对话
