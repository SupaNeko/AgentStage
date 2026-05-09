# AgentStage 技术栈选择方案

> 文档版本：V1.0  
> 日期：2026-05-09  
> 依据：PRD V1.2、Reference 分析（SillyTavern / RisuAI / text-generation-webui）、skill 技术栈指南

---

## 一、选型原则

1. **Windows 桌面应用优先**：最终交付物是 `.msi`/`.exe`，不是 Web 应用
2. **现代可维护**：技术栈应有活跃社区，长期维护成本低
3. **性能优先**：IM 应用要求流畅的滚动、切换、消息收发体验
4. **类型安全**：复杂业务逻辑（Prompt 拼接、消息调度、群聊状态机）需要类型系统保障
5. **最小依赖**：AgentStage 是工具型应用，不应引入过度复杂的架构
6. **与参考项目对齐**：RisuAI（Tauri+Svelte）已被验证为角色扮演客户端的最佳实践

---

## 二、整体架构

```
┌─────────────────────────────────────────┐
│           AgentStage (Windows App)        │
│  ┌─────────────────────────────────────┐  │
│  │   Frontend (WebView2)               │  │
│  │   Svelte 5 + Vite + TailwindCSS     │  │
│  │   └─ 聊天界面 / Agent 配置 / 设置    │  │
│  └─────────────────────────────────────┘  │
│                    ↑ IPC ↓                  │
│  ┌─────────────────────────────────────┐  │
│  │   Backend (Rust) - Tauri Commands   │  │
│  │   └─ 数据库访问 / 文件系统 / 加密    │  │
│  └─────────────────────────────────────┘  │
│                    ↑ FFI / native           │
│  ┌─────────────────────────────────────┐  │
│  │   SQLite (本地文件)                  │  │
│  │   └─ 角色 / 会话 / 消息 / 设置       │  │
│  └─────────────────────────────────────┘  │
└─────────────────────────────────────────┘
                   ↑ HTTPS
            LLM API (OpenAI / Claude / etc)
```

---

## 三、分层技术选型

### 3.1 桌面框架层

| 候选方案 | 代表项目 | 产物体积 | 性能 | 打包难度 | 结论 |
|---------|---------|---------|------|---------|------|
| **Tauri v2** | RisuAI | ~5-15 MB | ⭐⭐⭐⭐⭐（Rust 原生 + WebView2） | 低（内置 nsis） | ✅ **推荐** |
| Electron | SillyTavern | ~150-300 MB | ⭐⭐⭐（Chromium 全量） | 低 | ❌ 体积过大 |
| Flutter Desktop | — | ~20-40 MB | ⭐⭐⭐⭐ | 中 | ⚠️ Dart 生态与 IM UI 组件少 |
| WPF (C#) | — | ~5-10 MB | ⭐⭐⭐⭐⭐ | 低 | ⚠️ 开发效率高但跨平台差，无现代生态 |
| WinUI 3 | — | ~10-20 MB | ⭐⭐⭐⭐⭐ | 中 | ⚠️ Windows 专属，生态不成熟 |

**选择：Tauri v2**

理由：
- RisuAI 已验证此方案在角色扮演客户端中的可行性
- WebView2 是 Windows 10/11 内置组件，无需额外携带浏览器内核
- Rust 后端提供高性能文件 IO、加密、SQLite 绑定
- `tauri.conf.json` 原生支持 `nsis` target，一键生成 Windows 安装包
- 自动更新插件 (`tauri-apps/plugin-updater`) 内置支持

**关键配置参考**：
```json
// tauri.conf.json
{
  "bundle": {
    "targets": ["nsis"],
    "windows": {
      "installMode": "passive"
    }
  },
  "plugins": {
    "updater": { /* 自动更新配置 */ }
  }
}
```
> 参考：`reference/RisuAI/src-tauri/tauri.conf.json`

---

### 3.2 前端框架层

| 候选方案 | 代表项目 | 响应式性能 | 学习曲线 | IM UI 生态 | 结论 |
|---------|---------|-----------|---------|-----------|------|
| **Svelte 5** | RisuAI | ⭐⭐⭐⭐⭐（编译时优化，无虚拟 DOM） | 低 | ⭐⭐⭐（组件库需自建） | ✅ **推荐** |
| React 18 | SillyTavern (新版探索中) | ⭐⭐⭐（虚拟 DOM 开销） | 中 | ⭐⭐⭐⭐⭐（生态最丰富） | ⚠️ 需要更多运行时开销 |
| Vue 3 | — | ⭐⭐⭐⭐ | 低 | ⭐⭐⭐⭐ | ⚠️ 与 Tauri 集成成熟但不如 Svelte 轻量 |
| SolidJS | — | ⭐⭐⭐⭐⭐ | 中 | ⭐⭐ | ⚠️ 生态太小 |

**选择：Svelte 5**

理由：
- RisuAI 已验证 Svelte 5 + Tauri 的组合在桌面端的高性能表现
- 编译时框架，运行时极小，消息列表滚动性能优于 React/Vue
- 内置细粒度响应式 (`$state`, `$derived`, `$effect`)，非常适合 IM 的实时消息更新
- `runes` 模式让状态管理极其直观，不需要额外的状态库

**核心状态管理方案**：
```typescript
// 使用 Svelte 5 Runes，无需 Redux/Zustand
// src/lib/stores/db.svelte.ts
class DatabaseState {
    characters = $state<Character[]>([])
    sessions = $state<Session[]>([])
    currentSessionId = $state<string | null>(null)
    
    get currentSession() {
        return $derived(this.sessions.find(s => s.id === this.currentSessionId))
    }
}
export const dbState = new DatabaseState()
```
> 参考：RisuAI 的 `src/ts/stores.svelte.ts` 使用 Svelte Store，AgentStage 可用 Svelte 5 Runes 升级

---

### 3.3 构建与开发工具层

| 工具 | 选择 | 理由 |
|------|------|------|
| **构建工具** | **Vite** | Tauri 官方推荐，与 Svelte 集成最佳，HMR 极快 |
| **语言** | **TypeScript** | 复杂业务逻辑（Prompt 拼接、调度器、状态机）的类型安全必需 |
| **样式方案** | **TailwindCSS v4** | RisuAI 验证，原子化 CSS 适合 IM 的大量细粒度 UI 调整 |
| **图标** | **Lucide** | 轻量、开源，RisuAI 使用 `@lucide/svelte` |
| **代码规范** | **ESLint + Prettier** | 标准配置 |
| **组件库** | **自建 + Bits UI** | 角色扮演客户端 UI 高度定制，第三方组件库难以满足；Bits UI (Svelte Headless) 提供无障碍基础组件 |

---

### 3.4 数据持久化层

| 候选方案 | 代表项目 | 查询能力 | 体积 | 加密 | 结论 |
|---------|---------|---------|------|------|------|
| **SQLite + rusqlite** | — | ⭐⭐⭐⭐⭐（完整 SQL） | ~1 MB | 需额外配置 | ✅ **推荐** |
| IndexedDB + localforage | RisuAI | ⭐⭐（键值 + 索引） | 0 | 无 | ⚠️ 复杂关系查询困难 |
| PGLite (WASM PostgreSQL) | — | ⭐⭐⭐⭐⭐ | ~5 MB | 无 | ⚠️ 过度设计 |
| json-server / 纯 JSON | SillyTavern | ⭐ | 0 | 无 | ❌ 无法处理关系查询和并发 |

**选择：SQLite（通过 Tauri Rust 后端访问）**

理由：
- PRD 明确要求 SQLite
- AgentStage 有大量关系型数据：角色 ↔ 会话 ↔ 消息 ↔ 群聊成员 ↔ 好友关系
- SQLite 是单文件数据库，备份/迁移极其简单（直接复制 `.db` 文件）
- Rust 侧使用 `rusqlite` 库，前端通过 Tauri Command 调用

**架构设计**：
```
Frontend (TS)  --IPC (invoke)-->  Tauri Command (Rust)  --FFI-->  SQLite (local file)
     ↑                                                                     ↓
     └────────────────── 返回序列化数据 (JSON) ────────────────────────────┘
```

**备选：如果希望前端直接访问**
- `sql.js` (SQLite WASM)：前端可直接运行 SQL，但大查询会阻塞主线程
- `wa-sqlite`：配合 OPFS，支持大容量存储
- **推荐仍用 Rust 后端**：Rust 可以处理异步 IO、连接池、加密，且 API Key 等敏感数据不经过前端

---

### 3.5 API 通信层

| 候选方案 | 场景 | 结论 |
|---------|------|------|
| **原生 fetch + Tauri HTTP Plugin** | 调用 OpenAI/Claude 等外部 API | ✅ **推荐** |
| Axios | 外部 API 调用 | ⚠️ 不需要，fetch 足够 |
| Tauri Events (WebSocket-like) | 前端 ↔ Rust 后端实时通信 | ✅ 用于消息推送、生成进度 |

**设计要点**：
- 外部 LLM API 调用建议走 **Rust 后端**而非前端直接调用：
  - API Key 保存在 Rust 侧，不暴露给前端
  - Rust 可以管理连接池、重试逻辑、流式响应转发
  - 防止前端被篡改导致 API Key 泄露
- 流式响应 (SSE)：Rust 后端接收 LLM 的 SSE 流，通过 Tauri Event 实时推送到前端

**Rust 侧 API 调用示意**：
```rust
// src-tauri/src/llm/mod.rs
#[tauri::command]
async fn send_message(
    state: tauri::State<'_, AppState>,
    session_id: String,
    content: String
) -> Result<StreamResponse, Error> {
    // 1. 写消息到 SQLite
    // 2. 组装 Prompt
    // 3. 调用 LLM API (reqwest)
    // 4. 流式返回
}
```

---

### 3.6 加密与安全层

| 数据类型 | 方案 | 实现位置 |
|---------|------|---------|
| **API Key** | **Windows DPAPI** 或 Rust `aes-gcm` | Rust 后端 |
| **SQLite 文件** | **SQLCipher** (SQLite 加密扩展) | 数据库层 |
| **应用配置** | Tauri 安全存储 (`tauri-apps/plugin-store`) | Rust 后端 |

**参考**：SillyTavern 使用 Windows 凭据管理器或 AES 加密存储 API Key；text-generation-webui 使用 Python keyring。

---

### 3.7 辅助工具库

| 用途 | 库 | 理由 |
|------|---|------|
| **日期时间** | `date-fns` | 轻量，IM 消息时间格式化 |
| **虚拟滚动** | `svelte-virtual` 或自建 | 消息列表长时性能 |
| **Markdown 渲染** | `marked` + `DOMPurify` | Agent 消息可能含 Markdown |
| **语法高亮** | `highlight.js` | 代码块高亮（参考 SillyTavern） |
| **Tokenizer** | `@dqbd/tiktoken` (WASM) | 本地计算 token 数（参考 RisuAI） |
| **UUID** | `crypto.randomUUID()` | 原生支持 |
| **PNG 元数据读写** | Rust `png` crate | 角色卡导入/导出 |
| **YAML/JSON 解析** | `serde_json` / `serde_yaml` | Rust 标准选择 |

---

## 四、关键技术决策详解

### 决策 1：为什么前端不直接调用 LLM API？

**争议点**：很多前端项目（如 SillyTavern）直接从前端调用 OpenAI API。

**AgentStage 选择走 Rust 后端的原因**：
1. **API Key 安全**：如果前端直接调用，API Key 必须存储在前端（即使是加密存储，解密后仍在 JS 内存中可被提取）。Rust 后端调用则 Key 完全不暴露给渲染进程。
2. **Prompt 拼接安全**：AgentStage 的 Prompt 包含多个 Agent 的私密人设，如果在前端拼接，用户可以通过 DevTools 查看其他 Agent 的详细人设。Rust 后端拼接可以防止这一点。
3. **调度器性能**：全局最小触发间隔、消息积压队列、消息上限计数等调度逻辑在 Rust 中运行更可靠，不受前端页面刷新/崩溃影响。
4. **流式响应转发**：Rust 可以持续接收 SSE 流，即使前端暂时断开（如切换页面），也不会中断生成。

**妥协**：开发调试阶段可以允许前端直连，生产构建强制走后端。

### 决策 2：为什么不用 MCP（Model Context Protocol）作为工具系统？

**争议点**：RisuAI 和 text-generation-webui 都支持 MCP，这是 Anthropic 推动的开放标准。

**AgentStage 选择先自研简化版 ToolManager 的原因**：
1. **当前需求单一**：AgentStage 目前只需要 `send_message` 一个工具，引入完整 MCP 客户端是过度设计。
2. **MCP 依赖复杂**：stdio MCP 需要本地子进程管理，HTTP MCP 需要网络通信，增加故障点。
3. **进度控制**：MCP 是 Phase 2 扩展点。MVP 阶段先实现内建的 `send_message`，验证核心循环后再考虑 MCP 兼容。

**扩展路径**：
```
Phase 1 (MVP): 内建 send_message 工具（类似 SillyTavern ToolManager）
Phase 2 (扩展): 引入 MCP 客户端，兼容外部工具（参考 RisuAI mcp.ts）
```

### 决策 3：为什么 SQLite 不走 ORM？

**争议点**：TypeScript 侧可以用 Drizzle ORM 或 Prisma（配合 WASM SQLite）。

**AgentStage 选择原生 SQL + 手写 Repository 的原因**：
1. **Rust 后端是数据层**：前端不直接操作数据库，所有 SQL 在 Rust 侧执行。Rust 的 ORM（如 Diesel、SeaORM）对初学者不够友好。
2. **Schema 稳定**：AgentStage 的数据库 Schema 在 PRD 阶段已基本确定（角色、会话、消息、设置），变化不会太大。
3. **性能敏感**：IM 应用的消息查询需要精细优化（分页、索引、批量插入），手写 SQL 更可控。
4. **SQL 简单**：AgentStage 没有复杂 JOIN 或事务，主要是 CRUD + 分页。

**数据访问架构**：
```
Rust 层 (rusqlite)
  ├── src/db/connection.rs    // 连接管理
  ├── src/db/schema.rs        // Schema 定义 + 迁移
  ├── src/db/character.rs     // 角色 Repository
  ├── src/db/session.rs       // 会话 Repository
  ├── src/db/message.rs       // 消息 Repository
  └── src/db/settings.rs      // 设置 Repository

Tauri Commands
  ├── src/commands/character.rs
  ├── src/commands/session.rs
  ├── src/commands/chat.rs
  └── src/commands/settings.rs
```

**Tauri Command 设计规范（混合模式）**：
- **写操作粗粒度**：一个 Command 封装完整业务流程，减少前端调用次数和事务边界复杂度。
  - 例：`send_message(session_id, content)` 一次性完成"写消息 → 更新会话状态 → 触发 Agent → 返回流式响应"。
- **读操作细粒度**：按需查询，方便前端组合数据。
  - 例：`get_session_list()`、`get_messages(session_id, limit)`、`get_agent_details(agent_id)` 独立提供。
- **错误处理**：Rust 侧统一返回 `Result<T, AppError>`，通过 Tauri IPC 序列化为 `{ ok: T } | { error: { code, message } }`，前端统一拦截处理。

### 决策 4：Svelte 5 Runes vs Svelte 4 Store？

**选择：Svelte 5 Runes**

理由：
- `$state`, `$derived`, `$effect` 让状态管理更直观，不需要学习 Store API
- 类式状态管理（`class DatabaseState { ... }`）天然适合复杂业务对象
- RisuAI 使用的是 Svelte 4 Store，AgentStage 可以用 Svelte 5 升级体验

---

## 五、技术栈总览表

| 层级 | 技术 | 版本/说明 | 负责内容 |
|------|------|----------|---------|
| **桌面框架** | Tauri v2 | latest | 窗口管理、系统菜单、安装包、自动更新 |
| **前端框架** | Svelte 5 | latest | UI 渲染、响应式状态、组件系统 |
| **构建工具** | Vite | v6+ | 开发服务器、HMR、打包 |
| **语言** | TypeScript | 5.6+ | 类型安全 |
| **样式** | TailwindCSS | v4 | 原子化 CSS |
| **Rust 后端** | Rust | 1.80+ | Tauri Commands、数据库访问、API 调用、加密 |
| **数据库** | SQLite | 3.40+ | 本地数据持久化 |
| **Rust SQLite** | rusqlite | latest | Rust 侧 SQLite 绑定 |
| **HTTP 客户端** | reqwest (Rust) | latest | 后端调用 LLM API |
| **前端存储** | Tauri Store Plugin | v2 | 应用级配置存储 |
| **图标** | Lucide | latest | SVG 图标 |
| **Markdown** | marked + DOMPurify | latest | 消息内容渲染 |
| **Token 计算** | tiktoken (WASM) | latest | 本地 token 预算管理 |
| **打包** | Tauri Bundler | nsis target | Windows .msi/.exe |

---

## 六、与参考项目的对标

| 维度 | SillyTavern | RisuAI | text-generation-webui | **AgentStage (选型)** |
|------|------------|--------|----------------------|---------------------|
| 桌面框架 | Electron (可选) | **Tauri v2** | Electron (辅助) | **Tauri v2** |
| 前端框架 | 原生 JS/jQuery | **Svelte 5** | Gradio (Python) | **Svelte 5** |
| 语言 | JavaScript | **TypeScript** | Python | **TypeScript + Rust** |
| 数据库 | 文件 JSON | IndexedDB | 文件/内存 | **SQLite** |
| API 调用 | 前端直接调用 | 前端直接调用 | 后端 | **Rust 后端代理** |
| 产物体积 | ~200MB | ~10MB | ~500MB+ | **~10-15MB** |

---

## 七、开发环境要求

### 必需安装

```bash
# 1. Rust + cargo
#    https://rustup.rs/

# 2. Node.js >= 20
#    https://nodejs.org/

# 3. pnpm (推荐) 或 npm
npm install -g pnpm

# 4. Windows 依赖（Tauri 需要）
#    Microsoft Visual Studio C++ Build Tools
#    或安装 Visual Studio 2022 + "Desktop development with C++" workload

# 5. WebView2 Runtime
#    Windows 10/11 通常已内置，开发时需要 Edge WebView2 Evergreen Standalone
```

### 项目初始化命令

```bash
# 使用 Tauri 官方脚手架
cargo install create-tauri-app --locked
create-tauri-app agentstage --template svelte-ts --manager pnpm

# 后续添加依赖
cd agentstage
pnpm install -D tailwindcss @tailwindcss/vite
pnpm install marked dompurify date-fns lucide-svelte

# Rust 依赖 (在 src-tauri/Cargo.toml 中)
# rusqlite = { version = "0.32", features = ["bundled", "chrono", "uuid"] }
# reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"] }
# serde = { version = "1.0", features = ["derive"] }
# tokio = { version = "1", features = ["full"] }
# aes-gcm = "0.10"  # API Key 加密
```

---

## 八、风险评估

| 风险点 | 影响 | 缓解措施 |
|--------|------|---------|
| Tauri v2 生态成熟度 | 中 | 核心功能稳定，边缘插件（如某些系统 API）需测试 |
| Svelte 5 新语法学习 | 低 | 团队熟悉后开发效率极高，文档完善 |
| Rust 异步编程复杂度 | 中 | LLM 调用用 `tokio`，SQLite 用 `rusqlite` 同步模式（单连接） |
| WebView2 兼容性 | 低 | Windows 10 1809+ 内置 WebView2，覆盖 99%+ 用户 |
| SQLite 并发写入 | 低 | 单用户桌面应用，并发极低，rusqlite WAL 模式足够 |
| 安装包签名 | 低 | 开发阶段自签名，发布时购买代码签名证书 |

---

## 九、下一步行动

技术栈确认后，可按以下顺序启动开发：

1. **项目脚手架搭建**：`create-tauri-app` 初始化，配置 TailwindCSS、路径别名
2. **数据库 Schema 设计**：基于 PRD 设计 SQLite 表结构（角色、会话、消息、设置）
3. **Rust 后端骨架**：Tauri Commands 层、rusqlite 连接、基础 Repository 模式
4. **前端基础框架**：页面状态管理（Svelte 5 Runes 条件渲染，无需路由库）、主题系统、组件库搭建
5. **Agent 配置 UI**：表单、验证、Tavern Card 导入（PNG 元数据解析）
6. **1对1 聊天核心**：消息列表、输入框、Prompt 拼接、LLM API 调用、流式展示
7. **群聊扩展**：禁言开关、多 Agent 调度器、消息上限、全局间隔

---

*文档结束*
