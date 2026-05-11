# Bug 诊断：修改后完全收不到消息

## 现象
- 用户反馈：在上次修改后，发送消息完全收不到任何回复
- 日志表现为无限循环的 `is_triggering=true, skip`
- 即使重启应用，仍然 `is_triggering=true, skip`

## 根因分析

### 1. UTF-8 切片 Panic（直接原因）
在 `scheduler/mod.rs` 和 `commands/message.rs` 中，使用 `&msg.content[..100]` 截断消息预览：

```rust
let preview = if msg.content.len() > 100 {
    format!("{}...", &msg.content[..100])
} else {
    msg.content.clone()
};
```

**问题：** `String::len()` 返回字节数，不是字符数。中文字符占 3 字节。当内容超过 100 字节（约 33 个汉字）时，`&content[..100]` 可能切在非 UTF-8 字符边界上，导致 **panic**。

**示例：**
```rust
let s = "你好".repeat(60); // 120 字节，60 个字符
let _ = &s[..100]; // panic! byte index 100 is not a char boundary
```

### 2. Panic 导致 `is_triggering` 死锁（根本原因）
`trigger_agent` 使用了 "finally" 模式：
```rust
let inner_result = self.trigger_agent_inner(agent_id, pending).await;
self.clear_triggering_flag(agent_id).await; // finally
```

**问题：** 在 Rust async/await 中，如果 Future 内部 panic，整个 Future 被终止，不会执行后续代码。`clear_triggering_flag` 永远不会被调用。

后果：
1. `trigger_agent_inner` panic → `is_triggering` 永远为 1
2. `start_background_scan` 的 `await` 永远阻塞，后台扫描停止
3. 所有后续 `try_trigger_agent` 检查到 `is_triggering=true`，全部 skip
4. **即使重启应用**，数据库中 `is_triggering=1` 仍然存在

### 3. 数据库残留
`trigger_states` 表持久化存储 `is_triggering` 字段。panic 后数据库记录未被更新。应用重启时读取旧值，仍然认为 agent 正在触发中。

## 证据链

```
[22:38:52] trigger_agent tool_calls_count=1
[22:38:52] ToolExecutor wrote message ...
           ← 这里应该继续：更新计数器、clear_triggering_flag
           ← 但日志完全中断，说明 panic 了
[22:41:55] send_user_message ...
[22:41:55] try_trigger_agent is_triggering=true, skip ← 死锁开始
[22:42:00] try_trigger_agent is_triggering=true, skip
[22:42:05] try_trigger_agent is_triggering=true, skip
...
[23:42:09] try_trigger_agent is_triggering=true, skip ← 重启后仍然 skip
```

## 修复方案

### 修复 1：安全字符串截断
将 `&content[..100]` 改为按字符截断：
```rust
fn truncate_preview(content: &str, max_chars: usize) -> String {
    if content.chars().count() > max_chars {
        content.chars().take(max_chars).collect::<String>() + "..."
    } else {
        content.to_string()
    }
}
```

影响文件：
- `scheduler/mod.rs`（`trigger_agent_inner` 中更新 session preview）
- `commands/message.rs`（`send_user_message` 中更新 session preview）

### 修复 2：应用启动时重置所有 `is_triggering`
在 `lib.rs` 的 `setup` 中，初始化 Scheduler 后立即执行：
```rust
let conn = db_state.0.lock().await;
conn.execute("UPDATE trigger_states SET is_triggering = 0", [])?;
```

这作为保险机制：即使之前 panic 导致数据库残留，重启后自动恢复。

### 修复 3：Stale `is_triggering` 检测（可选增强）
在 `try_trigger_agent` 中，如果 `is_triggering=true` 但 `updated_at` 超过 5 分钟，自动重置：
```rust
if is_triggering {
    // 检查是否 stale（超过 5 分钟）
    let updated_at: i64 = conn.query_row(
        "SELECT updated_at FROM trigger_states WHERE agent_id = ?1",
        [agent_id],
        |row| row.get(0),
    ).unwrap_or(0);
    let now = chrono::Utc::now().timestamp_millis();
    if now - updated_at > 5 * 60 * 1000 {
        // 自动重置并继续触发
    } else {
        return Ok(());
    }
}
```

## 验收标准
- [ ] 发送超过 33 个汉字的消息，不会 panic
- [ ] Agent 回复超过 33 个汉字，不会 panic
- [ ] 即使手动将 `is_triggering` 设为 1，重启应用后能自动恢复
- [ ] 后台扫描任务不会因 panic 而永久停止
- [ ] `cargo test` 通过（包含新的 panic 防护测试）
