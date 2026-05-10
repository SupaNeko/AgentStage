# AgentStage 后端架构设计

> 本文档面向 AgentStage 的前端开发者和 AI 协作者，系统阐述后端的设计立场、关键决策与实现路径。
> 
> **阅读建议**：先通读第 1 节目标和第 2 节的问题拆解（每节都有明确立场），再按需深入第 3 节的专题讨论。

---

## 1. 目标

AgentStage 后端的核心目标是：**在单用户桌面环境中，安全、可靠、可扩展地驱动多角色 LLM 的自动对话与交互**。

具体分解为：

1. **数据持久化**：本地 SQLite 管理角色、会话、消息、好友关系与配置，支持版本迁移与软删除
2. **API 安全**：API Key 等敏感信息绝不离开 Rust 后端，加密存储，前端不可见
3. **调度与触发**：事件驱动地调度角色调用，支持私聊/群聊差异化触发逻辑
4. **上下文组装**：每次调用时准确组装 Prompt，包含人设、可见历史、参与者简介与最新消息
5. **反循环保护**：防止角色之间无限对话导致 API 费用失控
6. **实时通信**：通过 SSE 向前端推送消息，避免轮询
7. **工具调用**：强制 Function Calling，角色只能通过 `send_message` Tool 发送消息

---

## 2. 问题拆解与立场

我们将后端设计拆分为 13 个独立问题，每个问题给出**明确立场**和**设计动机**。

---

### 2.1 数据持久化：为什么用 SQLite 而非 Server 数据库？

**立场**：使用 SQLite 单文件数据库，启用 WAL 模式。

**动机**：
- AgentStage 是**单用户桌面应用**，没有多用户并发需求，SQLite 的零配置、零运维特性完美匹配
- 单文件便于备份（复制 `.db` 文件即可）和数据迁移
- 对比 PostgreSQL/MySQL：需要独立服务进程，增加安装包体积和部署复杂度
- WAL 模式提升并发写入性能，满足同时写入消息 + 更新会话状态的场景

**反对意见及回应**：
- "SQLite 不适合大量并发" → 单用户场景不存在高并发
- "SQLite 没有 JSON 类型" → 我们使用 `TEXT` 存储 JSON，应用层解析，简单场景足够

---

### 2.2 Schema 设计：为什么私聊和群聊要拆成独立表？

**立场**：使用 Class Table Inheritance——`sessions` 基表 + `private_sessions` / `group_sessions` 扩展表。

**动机**：
- 私聊和群聊的**配置字段差异大**：群聊有 `mute_enabled`、`name`（必填），私聊有 `agent_id`（对方角色），单表会导致大量 NULL 字段和语义污染
- **约束隔离**：群聊名称可设 `NOT NULL`，不影响私聊表
- **查询意图明确**：`SELECT * FROM group_sessions WHERE mute_enabled = 0` 不会返回私聊行
- **独立演进**：未来群聊可增加 `announcement` 字段，不影响私聊表结构

**代价**：查询会话列表时需要 LEFT JOIN 两张子表，但在 Rust 层一次查询即可，复杂度可控。

---

### 2.3 Schema 设计：为什么好友关系是独立表，而不是从会话推导？

**立场**：维护独立的 `friendships` 表，创建私聊时自动插入。

**动机**：
- **持久化**：删除私聊会话后好友关系仍然保留（"删会话不等于删好友"）
- **扩展性**：未来支持手动添加/删除好友（P2），不依赖会话存在
- **查询性能**：获取好友列表只需查 `friendships`，无需扫描所有私聊
- **Prompt 区分**："好友"和"群友"在参与者简介中需要不同标注，独立表让区分更简单

**设计细节**：
- 使用无序对约束 `CHECK(agent_id_1 < agent_id_2)` 防止 (A,B) 和 (B,A) 重复
- 好友关系是双向的，不需要方向性

---

### 2.4 Schema 设计：为什么消息上限用显式计数器字段维护？

**立场**：在 `private_sessions` 和 `group_sessions` 中使用 `agent_message_count` 和 `last_reset_at` 字段，由应用层维护。

**动机**：
- **O(1) 检查**：触发时直接读取字段，无需 `COUNT(*)` 查询
- **重置时机明确**：用户发消息、创建新群聊、清空会话、手动重置按钮——四种场景统一通过更新字段完成
- **避免 SQL 复杂度**：不需要每次触发时查询 `messages` 表统计条数

**维护规则**：
1. 初始化：创建会话时 `agent_message_count = 0`, `last_reset_at = now`
2. 递增：角色发送消息后 `agent_message_count += 1`
3. 重置：用户发消息 / 新建群聊 / 清空会话 / 手动重置 → `agent_message_count = 0`

---

### 2.5 API Key 安全：为什么前端永远看不到 API Key？

**立场**：API Key 仅在 Rust 后端处理，加密存储，前端 DTO 中完全排除该字段。

**动机**：
- **防窃取**：前端代码可被 DevTools 查看，若 API Key 流经前端，用户可直接复制
- **防泄露**：即使应用被逆向，加密后的 Key 也需要密钥才能解密
- **信任边界**：Rust 后端是唯一可信执行环境

**实现**：
- 使用 AES-256-GCM 加密，`api_key_encrypted` 存储为 BLOB
- `AgentResponse` DTO 不包含 `api_key_encrypted` 字段
- 前端发送的 Key 只在 create/update 时传入，后端加密后立即丢弃明文

---

### 2.6 Tauri IPC：为什么命令设计是"粗写细读"？

**立场**：写操作（create/update/delete）使用粗粒度 DTO，读操作（get/list）按需返回字段。

**动机**：
- **减少往返**：创建角色时一次传入所有字段，不需要多次 IPC 调用
- **安全裁剪**：读操作时排除敏感字段（如 `api_key_encrypted`）
- **前端便利**：粗粒度写操作让前端表单提交更简单

**示例**：
- `create_agent` 接收 `CreateAgentRequest`（含所有字段）
- `get_agent` 返回 `AgentResponse`（不含 `api_key_encrypted`）
- `list_agents` 返回 `Vec<AgentResponse>`（精简字段）

---

### 2.7 反循环架构：为什么需要三层防护？

**立场**：使用**消息上限（硬限制）+ 全局时间间隔（软限制）+ sender 排除（逻辑限制）**三层防护。

**动机**：
- **消息上限**：达到阈值后**强制停止**，是最后一道保险，防止 API 费用失控
- **时间间隔**：每个角色全局 30 秒最小间隔，降低循环速度，模拟人类反应时间
- **sender 排除**：调度器查询时排除 `sender_id = 当前角色` 的消息，防止角色被自己的回复触发

**为什么三层缺一不可？**
- 只有时间间隔：极端情况下仍可能无限循环（虽然慢）
- 只有消息上限：用户需要频繁手动重置，体验差
- 只有 sender 排除：无法防止 A→B→A→B 的跨角色循环

---

### 2.8 Prompt 组装：为什么按固定五层顺序拼接？

**立场**：严格按 `System Prompt → 自身人设 → 参与者简介 → 历史消息 → 最新消息` 的顺序拼接。

**动机**：
- **优先级递减**：系统指令和人设必须最先被模型看到，历史消息可以截断
- **参与者简介动态筛选**：只包含好友 + 当前群友，避免无关角色干扰上下文
- **最新消息单独高亮**：让角色明确知道"这次触发是因为哪条消息"

**层级说明**：
1. **System Prompt**：全局固定，硬编码，所有角色共享（如"你是一个 IM 聊天参与者"）
2. **自身人设**：`detailed_persona`，角色的自我认知
3. **参与者简介**：使用其他角色的 `simplified_persona`，标注好友/群友关系
4. **历史消息**：按私聊/群聊分组，该角色参与的所有会话
5. **最新消息**：触发本次调用的消息，标注来源会话

---

### 2.9 Pending Queue：为什么不持久化？

**立场**：`pending_queue` 是**内存-only**数据结构，重启后通过 `messages.created_at > last_trigger_time` 重建。

**动机**：
- **消息已持久化**：`messages` 表是 source of truth
- `last_trigger_time` 已持久化在 `trigger_states` 表
- 重启后查询 `messages WHERE created_at > last_trigger_time` 即可恢复队列
- 避免额外表的维护复杂性

**边界情况**：应用崩溃时内存队列丢失，但消息不会丢失，重启后自动重建。

---

### 2.10 调度策略：为什么是事件驱动 + 定时器，而不是轮询？

**立场**：事件驱动（消息到达触发检查）+ 定时器（间隔到期自动触发）。

**动机**：
- **即时响应**：间隔满足时立即触发，不需要等待轮询周期
- **资源节约**：没有消息时调度器不工作，不消耗 CPU
- **批量处理**：间隔内积压的消息一次性组装，减少 API 调用次数

**工作流程**：
1. 消息到达 → 检查 `now - last_trigger_time >= interval`
2. 满足 → 立即触发，更新 `last_trigger_time = now`
3. 不满足 → 加入 pending_queue，设置定时器在 `last_trigger_time + interval` 时触发

---

### 2.11 SSE：为什么用 SSE 而不是 WebSocket 或轮询？

**立场**：使用 SSE（Server-Sent Events）从 Rust 后端向前端推送消息。

**动机**：
- **单向通信足够**：后端→前端推送消息，前端→后端使用 Tauri invoke
- **比 WebSocket 轻量**：SSE 基于 HTTP，不需要额外的 WebSocket 握手和帧协议
- **比轮询高效**：消息到达立即推送，不需要定期请求
- **Tauri 支持**：Tauri v2 的 Event API 可以模拟 SSE 行为

**实现方式**：Rust 后端通过 `app_handle.emit()` 发送事件，前端通过 `listen()` 接收。

---

### 2.12 Function Calling：为什么强制要求，不提供 fallback？

**立场**：AgentStage **只支持**支持 Function Calling / Tool Use 的模型，配置时检测，不支持则阻断。

**动机**：
- **架构依赖**：`send_message` Tool 是角色发送消息的唯一途径，没有 fallback 的架构更简单
- **行为可控**：Tool 调用有明确的 Schema 和参数，比纯文本解析更可靠
- **一致性**：所有角色使用统一的交互协议

**检测方式**：维护一个支持 Tool Calling 的模型白名单，或通过 API capability 检测。

---

### 2.13 批量处理：为什么间隔内积压的消息要一次性组装？

**立场**：30 秒间隔内到达的所有消息，一次性拼接进 Prompt 的"最新消息"层。

**动机**：
- **降低 API 成本**：从"每条消息调一次"变为"每 30 秒调一次"
- **自然节奏**：模拟人类"看完几条消息后一起回复"的行为
- **上下文完整**：角色可以看到间隔内所有新消息，综合决定回复策略

**示例**：
```
【最新消息】
- 用户在私聊 A 中说："你好"（t=10）
- 角色 B 在群聊 C 中说："大家晚上好"（t=15）
- 用户在同一私聊 A 中说："在吗？"（t=25）
```

---

## 3. 复杂专题深入

对于上述问题中需要更多篇幅才能说清的部分，本节给出专题讨论。

---

### 3.1 反循环架构详解

#### 问题背景

多角色自动对话最大的风险是**无限循环**：A 回复 B，B 回复 A，A 又回复 B……如果不加限制，API 调用和费用会无限增长。

#### 三层防护机制

| 层级 | 机制 | 作用时机 | 效果 |
|------|------|---------|------|
| 第一层 | 消息上限 (`agent_message_count`) | 每次角色发送消息后检查 | 达到上限后**强制停止**自动触发，用户必须发消息或手动重置才能继续 |
| 第二层 | 全局最小触发间隔 (`last_trigger_time`) | 每次触发前检查 | 每个角色最多每 30 秒响应一次，降低循环速度 |
| 第三层 | Sender 排除 | 调度器查询消息时 | 排除 `sender_id = 当前角色` 的消息，防止角色被自己的回复触发 |

#### 群聊 vs 私聊的差异

**私聊**：
- 用户发消息 → 触发对方角色（检查间隔和上限）
- 角色 A 发消息给角色 B → 触发角色 B（检查间隔和上限）
- 角色 B 回复 → 触发角色 A
- 理论上 A↔B 可以无限循环，但受 30 秒间隔和消息上限约束

**群聊**（禁言关闭时）：
- 任何消息发送后，**并行触发群聊中除发送者外的所有其他角色**
- 每个角色独立检查自己的间隔和上限
- 发送者自己不会被触发（Sender 排除）

#### 为什么 sender 排除在调度器层而非数据库层？

数据库查询可以写成：
```sql
AND NOT (m.sender_type = 'agent' AND m.sender_id = 'agent_xxx')
```

但这只是**查询优化**，真正的防循环逻辑在调度器层：
- 私聊：触发的是接收方，不是发送方
- 群聊：触发时遍历群成员列表，显式排除 `sender_id`

数据库层的排除是**冗余保险**，防止意外情况。

#### 代码片段：触发决策逻辑

```rust
enum TriggerDecision {
    Proceed(Vec<Message>),
    Blocked(Reason),
}

fn decide_trigger(agent_id: &str, session: &Session) -> TriggerDecision {
    // 1. 检查消息上限
    if session.message_limit_enabled && session.agent_message_count >= session.message_limit {
        return TriggerDecision::Blocked(Reason::MessageLimit);
    }
    
    // 2. 检查全局间隔
    let last_trigger = get_last_trigger_time(agent_id);
    if now - last_trigger < global_min_interval {
        return TriggerDecision::Blocked(Reason::IntervalNotMet);
    }
    
    // 3. 获取待处理消息（已排除 sender = 当前角色）
    let pending = get_pending_messages(agent_id);
    if pending.is_empty() {
        return TriggerDecision::Blocked(Reason::NoNewMessages);
    }
    
    TriggerDecision::Proceed(pending)
}
```

#### 总结

反循环是 AgentStage 的**安全核心**。消息上限提供硬刹车，时间间隔提供软减速，sender 排除防止自触发。三层机制独立工作，任何一层触发都能阻止失控。

---

### 3.2 Prompt Assembly 详解

#### 问题背景

每次调用 LLM 时，需要将角色的上下文组装成一个完整的 Prompt。这个 Prompt 的质量直接决定角色的回复质量。

#### 五层结构

```
第 1 层：System Prompt（全局固定）
第 2 层：自身人设（detailed_persona）
第 3 层：参与者简介（动态筛选）
第 4 层：历史消息（按会话分组）
第 5 层：最新消息（触发源）
```

#### 各层详细说明

**第 1 层：System Prompt**

硬编码，所有角色共享：
```
你是一个正在参与即时通讯聊天的 AI 角色。请根据上下文自然地回应。
你可以同时参与多个私聊和群聊，在回复时请根据上下文判断应该回复哪个会话。
如果需要回复多个会话，可以多次调用 send_message 工具。
```

**第 2 层：自身人设**

使用角色的 `detailed_persona`：
```
你是卫宫士郎，出自 Fate/stay night...
```

**第 3 层：参与者简介**

动态查询，只包含两类角色：
- **好友**：`friendships` 表中与该角色有关联的角色
- **群友**：与该角色在同一个群聊中的其他角色

使用 `simplified_persona` 作为简介：
```
你当前在群聊 '冬木市互助群' 中，参与者有：
- 远坂凛（你的好友）：冬木市的天才魔术师，性格傲娇但内心善良。
- Saber（群友）：不列颠的骑士王，正直威严，正在寻找圣杯。
- 间桐慎二（群友）：你的同学，性格自大但内心脆弱。
```

**查询 SQL**（群聊场景）：
```sql
SELECT 
    a.id, a.name, a.simplified_persona,
    CASE WHEN f.agent_id_1 IS NOT NULL THEN 1 ELSE 0 END as is_friend
FROM group_members gm
JOIN agents a ON gm.participant_id = a.id
LEFT JOIN friendships f ON 
    (f.agent_id_1 = 'agent_xxx' AND f.agent_id_2 = a.id) OR
    (f.agent_id_2 = 'agent_xxx' AND f.agent_id_1 = a.id)
WHERE gm.session_id = 'group_session_id'
  AND gm.participant_type = 'agent'
  AND gm.participant_id != 'agent_xxx'
ORDER BY is_friend DESC, a.name;
```

**第 4 层：历史消息**

按私聊/群聊分组，包含该角色参与的所有会话：
```
【私聊记录】
与 远坂凛 的私聊：
- 远坂凛: "士郎，今晚来我家吃饭吗？"
- 你: "好啊，需要我带什么吗？"

【群聊记录】
群聊 '冬木市互助群'：
- Saber: "各位，今晚有空的请到学校仓库集合。"
- 远坂凛: "什么事？"
- 你: "我刚好有空，需要准备什么吗？"
```

**查询 SQL**：
```sql
SELECT m.*, s.session_type, COALESCE(gs.name, a.name) as session_name
FROM messages m
JOIN sessions s ON m.session_id = s.id
LEFT JOIN group_sessions gs ON s.id = gs.session_id AND s.session_type = 'group'
LEFT JOIN private_sessions ps ON s.id = ps.session_id AND s.session_type = 'private'
LEFT JOIN agents a ON ps.agent_id = a.id
WHERE m.session_id IN (
    SELECT session_id FROM private_sessions WHERE agent_id = 'agent_xxx'
    UNION
    SELECT session_id FROM group_members 
    WHERE participant_id = 'agent_xxx' AND participant_type = 'agent'
)
  AND m.is_deleted = 0
ORDER BY s.session_type, m.created_at;
```

**第 5 层：最新消息**

单独高亮触发本次调用的消息：
```
【最新消息】
远坂凛 在群聊 '冬木市互助群' 中说："士郎，今晚来我家吃饭吗？"
```

#### Token 预算管理（P1 实现）

MVP 阶段暂不实现动态截断。P1 阶段引入：
- 使用 tiktoken 本地计算各层 token 数
- 当总 Prompt 接近模型上下文上限时，优先截断历史消息（第 4 层）
- 保留系统提示词、人设、参与者简介和最新消息

#### 总结

Prompt Assembly 的核心是**分层 + 动态筛选**。系统提示和人设固定不变，参与者简介和历史消息动态构建，最新消息单独高亮。这种结构保证了角色既有稳定的自我认知，又能准确感知当前上下文。

---

### 3.3 调度器与消息准确性

#### 问题背景

调度器需要保证：**每次调用角色时，pending_queue 中包含所有应该被处理的新消息，且不包含已处理过的消息**。

#### 核心机制：时间戳窗口

```
last_trigger_time ────────────────────── now
                    ↑ 待处理消息范围 ↑
```

**查询**：
```sql
SELECT * FROM messages
WHERE session_id IN (
    SELECT session_id FROM private_sessions WHERE agent_id = 'agent_X'
    UNION
    SELECT session_id FROM group_members 
    WHERE participant_id = 'agent_X' AND participant_type = 'agent'
)
  AND is_deleted = 0
  AND created_at > (
      SELECT last_trigger_time FROM trigger_states WHERE agent_id = 'agent_X'
  )
  AND NOT (sender_type = 'agent' AND sender_id = 'agent_X')
ORDER BY created_at;
```

#### 边界情况分析

**情况 A：触发过程中有新消息到达**

- t=30：触发角色 X，`last_trigger_time=0`，获取 M1-M5
- t=32：API 调用过程中，群聊收到 M6
- t=35：生成完成，更新 `last_trigger_time=35`
- 下次查询 `created_at > 35` → M6 会被处理

**结果**：M6 延迟到下次触发（t=60），没有遗漏。**这是符合预期的**——不应在角色生成回复时中断并重新组装 Prompt。

**情况 B：应用崩溃后恢复**

- t=10：触发角色 X，处理 M1-M3，更新 `last_trigger_time=10`
- t=15：收到 M4，加入内存 pending_queue
- t=20：**应用崩溃**，pending_queue 丢失
- 重启后：`last_trigger_time=10`（已持久化），M4 仍在 `messages` 表（`created_at=15`）
- 查询 `created_at > 10` → 获取 M4

**结果**：pending_queue 不持久化不会导致消息丢失。消息表是 source of truth。

**情况 C：多条消息在同一毫秒到达**

SQLite 的 `INTEGER` 时间戳精度为毫秒。即使有同一毫秒的消息，`>` 操作符也不会遗漏（只要 `last_trigger_time` 是上一轮触发开始时的时间，不是某条消息的时间）。

#### 潜在风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 触发过程中到达的消息延迟 30 秒 | 低 | 符合"人类反应时间"设计目标 |
| 时间戳精度不足导致消息遗漏 | 极低 | 毫秒级精度足够 |
| 角色回复被当作新消息自触发 | 中 | 调度器层排除 sender_id = 当前角色 |
| 崩溃后 pending_queue 丢失 | 无 | 消息已持久化，重启后重建 |

#### 如果要彻底消除"触发过程中消息延迟"

需要引入**Snapshot 模式**：
1. 触发开始时记录 `snapshot_time = now`
2. 查询 `created_at > last_trigger_time AND created_at <= snapshot_time`
3. 生成完成后检查 snapshot_time 之后是否有新消息
4. 如果有，重新组装 Prompt（包含所有消息）再次调用

**评价**：实现复杂度高，API 成本翻倍，对用户体验提升有限。**AgentStage 不需要此方案**。当前的时间戳窗口方案已足够。

---

### 3.4 SSE 实现方案

#### 问题背景

前端需要实时接收角色发送的消息、系统通知（如"达到消息上限"）和状态更新（如"禁言开关变化"）。

#### 为什么不用 WebSocket

- WebSocket 需要额外的握手和帧协议，对于单向推送（后端→前端）过重
- SSE 基于 HTTP，更轻量，且 Tauri 的 Event API 天然支持这种模型

#### Tauri v2 Event 方案

Rust 端发送事件：
```rust
use tauri::Emitter;

// 发送新消息事件
app_handle.emit("new_message", json!({
    "session_id": session_id,
    "message": message_dto
}))?;

// 发送系统通知
app_handle.emit("system_notice", json!({
    "session_id": session_id,
    "type": "message_limit_reached",
    "content": "已达到消息上限，自动对话已暂停"
}))?;
```

前端监听事件：
```typescript
import { listen } from '@tauri-apps/api/event';

listen('new_message', (event) => {
    const { session_id, message } = event.payload;
    // 更新对应会话的消息列表
});

listen('system_notice', (event) => {
    const { session_id, type, content } = event.payload;
    // 显示系统通知
});
```

#### 事件类型清单

| 事件名 | 方向 | 说明 |
|--------|------|------|
| `new_message` | 后端→前端 | 新消息到达（用户或角色发送） |
| `system_notice` | 后端→前端 | 系统通知（上限 reached、角色加入群聊等） |
| `agent_triggered` | 后端→前端 | 角色被触发开始生成（可选，用于"正在输入"提示） |
| `agent_completed` | 后端→前端 | 角色生成完成（可选） |
| `session_updated` | 后端→前端 | 会话状态更新（未读数、最后消息等） |

#### 总结

SSE（通过 Tauri Event）是 AgentStage 实时通信的最佳选择。它轻量、单向、与 Tauri 生态集成良好，足以满足消息推送和状态通知的需求。

---

## 4. 关键代码片段

### 4.1 数据库连接初始化（WAL 模式）

```rust
use rusqlite::Connection;

fn init_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    
    // WAL 模式：提升并发写入性能
    conn.execute("PRAGMA journal_mode = WAL", [])?;
    
    // 外键约束
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    
    // 同步模式：NORMAL 在 WAL 下足够安全
    conn.execute("PRAGMA synchronous = NORMAL", [])?;
    
    Ok(conn)
}
```

### 4.2 触发状态更新（原子操作）

```rust
fn update_trigger_time(conn: &Connection, agent_id: &str) -> Result<()> {
    let now = current_timestamp_ms();
    conn.execute(
        "INSERT INTO trigger_states (agent_id, last_trigger_time, updated_at)
         VALUES (?1, ?2, ?2)
         ON CONFLICT(agent_id) DO UPDATE SET
         last_trigger_time = excluded.last_trigger_time,
         updated_at = excluded.updated_at",
        [agent_id, now.to_string()],
    )?;
    Ok(())
}
```

### 4.3 获取待处理消息（含 sender 排除）

```rust
fn get_pending_messages(
    conn: &Connection, 
    agent_id: &str
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT m.* FROM messages m
         WHERE m.session_id IN (
             SELECT session_id FROM private_sessions WHERE agent_id = ?1
             UNION
             SELECT session_id FROM group_members 
             WHERE participant_id = ?1 AND participant_type = 'agent'
         )
         AND m.is_deleted = 0
         AND m.created_at > (
             SELECT last_trigger_time FROM trigger_states WHERE agent_id = ?1
         )
         AND NOT (m.sender_type = 'agent' AND m.sender_id = ?1)
         ORDER BY m.created_at"
    )?;
    
    let messages = stmt.query_map([agent_id], |row| {
        Ok(Message {
            id: row.get(0)?,
            session_id: row.get(1)?,
            sender_type: row.get(2)?,
            sender_id: row.get(3)?,
            content: row.get(4)?,
            created_at: row.get(5)?,
            message_type: row.get(6)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    
    Ok(messages)
}
```

---

## 5. 参考文献

- [AgentStage PRD](../PRD.md) — 产品需求文档
- [AgentStage Schema](../schema.md) — 数据库 Schema 设计
- [AgentStage Feature List](../feature_list.md) — 功能清单
- [Tauri v2 Documentation](https://tauri.app/) — Tauri 官方文档
- [rusqlite Documentation](https://docs.rs/rusqlite/) — Rust SQLite 绑定
- OpenAI Function Calling: https://platform.openai.com/docs/guides/function-calling
- Anthropic Tool Use: https://docs.anthropic.com/en/docs/build-with-claude/tool-use

---

*文档结束*
