# AgentStage 参考项目分析

> 分析日期：2026-05-09  
> 分析范围：SillyTavern、TavernAI、RisuAI、text-generation-webui  
> 重点关注：Agent 搭建、角色扮演 Prompt 构建、历史消息拼接、工具搭建、技术栈选择

---

## 一、项目总览

| 项目名称 | 路径 | 定位 | 成熟度 | 社区活跃度 |
|---------|------|------|--------|-----------|
| **SillyTavern** | `reference/SillyTavern/` | 最成熟的 AI 角色扮演前端/客户端 | ⭐⭐⭐⭐⭐ | 极高 |
| **TavernAI** | `reference/TavernAI/` | SillyTavern 的前身，早期版本 | ⭐⭐ | 低（已被取代） |
| **RisuAI** | `reference/RisuAI/` | 新兴桌面端 AI 角色扮演客户端 | ⭐⭐⭐⭐ | 高 |
| **text-generation-webui** | `reference/text-generation-webui/` | LLM 推理后端 + WebUI，含角色扮演功能 | ⭐⭐⭐⭐⭐ | 极高 |

---

## 二、技术栈分析（对 AgentStage 的参考价值）

### 2.1 各项目技术栈

#### SillyTavern — Node.js + Express + 原生前端

| 层级 | 技术 | 说明 |
|------|------|------|
| 运行时 | Node.js >= 20 | 服务器端运行 |
| 后端框架 | Express 4.x | REST API 服务 |
| 前端 | 原生 JavaScript + jQuery | 无现代前端框架，传统 DOM 操作 |
| 打包 | 纯 Node.js 应用 + 可选 Electron | `src/electron/package.json` 提供 Electron 封装 |
| 数据存储 | 文件系统 (JSON/PNG/YAML) + node-persist | 角色卡存为 PNG 内嵌 JSON，聊天记录存 JSON |
| 加密 | 无内置数据库加密 | API Key 存储方式需查看具体实现 |
| 通信 | REST API + WebSocket (ws) | 前后端通过 HTTP/WebSocket 通信 |

**评价**：技术栈老旧但极其成熟稳定。没有使用现代前端框架导致代码量庞大（`openai.js` 单文件 299KB），但兼容性极好。AgentStage **不应直接采用**此技术栈，因为用户明确要求 Windows 桌面应用且现代体验，jQuery 方案不符合长期维护需求。

#### RisuAI — Tauri v2 + Svelte 5 + TypeScript

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | **Tauri v2** (Rust) | `src-tauri/tauri.conf.json` 配置，支持 nsis/deb/rpm/appimage/dmg |
| 前端框架 | **Svelte 5** | 响应式框架，配合 Svelte Store 管理状态 |
| 构建工具 | **Vite** | 现代前端构建 |
| 样式 | **TailwindCSS v4** | 原子化 CSS |
| 语言 | **TypeScript** | 全项目类型安全 |
| 数据存储 | localforage (IndexedDB) | 浏览器端持久化存储 |
| 后端通信 | 内嵌 HTTP 服务器 (Hono/Express) | `server/hono/` 提供可选后端服务 |
| 自动更新 | Tauri updater plugin | 支持自动更新检测 |
| 文件关联 | `.risum`, `.risup`, `.charx` | `tauri.conf.json` 中配置 |

**评价**：**这是 AgentStage 最值得参考的技术栈**。Tauri + Svelte 是轻量、现代、高性能的桌面端方案：
- Tauri v2 使用 Rust 编写原生层，但前端完全用 Web 技术，最终产物体积远小于 Electron
- 支持 Windows `.msi` / `.exe` 安装包（nsis target）
- Svelte 5 的响应式系统非常适合 IM 类实时 UI 更新
- TypeScript 全栈保证类型安全
- localforage/IndexedDB 作为本地存储方案可替代 SQLite（但 AgentStage 要求 SQLite，可以作为替代参考）

#### text-generation-webui — Python + Gradio + FastAPI

| 层级 | 技术 | 说明 |
|------|------|------|
| 后端语言 | Python 3.13 | 模型推理核心 |
| Web UI | **Gradio** (自定义 fork) | `requirements.txt` 中引用 oobabooga 的 gradio 定制版 |
| API 框架 | **FastAPI** + Flask | `fastapi==0.112.4`, `flask_cloudflared` |
| 模型推理 | PyTorch, Transformers, llama.cpp, exllamav3 | 支持多种后端 |
| 模板引擎 | **Jinja2** | Prompt 渲染核心 |
| 桌面封装 | Electron (辅助) | `desktop/package.json` 提供，但非主要使用方式 |

**评价**：这是**后端/模型推理**的标杆项目，不是前端角色扮演客户端。AgentStage 的定位是纯客户端（调用外部 API），不需要本地模型推理，因此 Python + Gradio 的方案**不适合**作为 AgentStage 的主技术栈。但其 Jinja2 模板处理 Prompt 的方式、MCP 工具系统值得参考。

#### TavernAI — Node.js + Express（ legacy ）

| 层级 | 技术 |
|------|------|
| 运行时 | Node.js |
| 后端 | Express 4.x |
| 前端 | 原生 JS |
| 图像处理 | sharp, webp-converter |

**评价**：已被 SillyTavern 完全取代，功能简单，参考价值低。

### 2.2 对 AgentStage 技术栈选择的建议

基于以上分析，AgentStage 的推荐技术栈组合：

```
桌面框架: Tauri v2 (Rust)        ← 参考 RisuAI
前端框架: Svelte 5               ← 参考 RisuAI
构建工具: Vite                   ← 参考 RisuAI
样式方案: TailwindCSS            ← 参考 RisuAI
语言: TypeScript                 ← 参考 RisuAI
数据存储: SQLite (via Rust 或前端 sql.js)  ← 需要自行选择，RisuAI 用 IndexedDB
打包: Tauri bundler (nsis)       ← 参考 RisuAI tauri.conf.json
API 调用: 原生 fetch / tauri-http-plugin  ← 参考 RisuAI
```

**为什么不选 Electron**：Electron 体积大（>100MB），Tauri 产物通常 <10MB，且 Rust 后端性能更好。

**为什么不选 SillyTavern 的纯 Node.js**：纯 Node.js 需要用户安装 Node 环境或打包为便携版，不符合"安装包一键安装"的需求，且 jQuery 技术债太重。

---

## 三、Agent 搭建（角色创建与管理）

### 3.1 角色数据结构对比

#### SillyTavern — Tavern Card V2 标准

**核心文件**：
- `reference/SillyTavern/src/endpoints/characters.js` — 角色 CRUD API
- `reference/SillyTavern/src/validator/TavernCardValidator.js` — 角色卡校验
- `reference/SillyTavern/src/character-card-parser.js` — PNG 内嵌元数据解析
- `reference/SillyTavern/src/charx.js` — CharX 格式支持

**角色数据字段**（V2 标准）：
```javascript
// TavernCardValidator.js 中定义
const requiredFields = [
  'name', 'description', 'personality', 'scenario', 
  'first_mes', 'mes_example', 'creator_notes', 'system_prompt', 
  'post_history_instructions', 'alternate_greetings', 'tags', 
  'creator', 'character_version', 'extensions'
];
```

**关键设计**：
- **PNG 内嵌元数据**：角色卡以 PNG 图片形式存储，元数据嵌入 PNG chunk 中。这实现了"一张图片就是一个角色"的便携分享。
- **character_book (Lorebook)**：角色可自带知识书，在聊天中动态注入上下文。
- **extensions 字段**：扩展字段，供插件存储自定义数据。
- **avatars**：角色头像独立存储，支持多格式 (jpg/png/webp)。

**对 AgentStage 的参考价值**：
- ✅ **角色卡导入/导出**：AgentStage 应支持 Tavern Card V2 格式的导入，这是行业事实标准。
- ✅ **PNG 内嵌元数据**：参考 `src/character-card-parser.js` 的 PNG chunk 读写逻辑。
- ❌ 不需要 `mes_example`（示例对话），因为 AgentStage 的 Agent 是通过实际聊天构建历史的，不是通过静态示例。

#### RisuAI — 统一 characters 数组

**核心文件**：
- `reference/RisuAI/src/ts/characters.ts` — 角色/群聊创建与管理
- `reference/RisuAI/src/ts/storage/database.svelte.ts` — 数据库类型定义 (line 1304+)

**角色数据结构**（`character` interface）：
```typescript
export interface character{
    type?: "character"
    name: string
    image?: string           // 头像路径
    firstMessage: string     // 开场白
    desc: string             // 角色描述（对应详细人设）
    notes: string            // 备注
    chats: Chat[]            // 聊天记录数组
    chatPage: number         // 当前聊天页
    personality: string      // 性格描述
    scenario: string         // 场景设定
    systemPrompt: string     // 系统提示词覆盖
    postHistoryInstructions: string  // 历史后指令
    alternateGreetings: string[]     // 备用开场白
    tags: string[]
    creator: string
    characterVersion: string
    exampleMessage: string   // 示例消息（少用到）
    emotionImages: [string, string][]  // 表情图片 [名称, 路径]
    globalLore: loreBook[]   // 全局 Lorebook
    customscript: customscript[]
    triggerscript: triggerscript[]
    utilityBot: boolean      // 是否为工具型 Bot
    bias: [string, number][] // Logit bias
    ccAssets: Array<{type, uri, name, ext}>  // Character Card 资源
    // ... 更多字段
}
```

**群聊数据结构**（`groupChat` interface）：
```typescript
export interface groupChat{ 
    type: 'group'
    name: string
    characters: string[]        // 成员角色 ID 数组
    characterTalks: number[]    // 每个成员的发言欲望值 (0-1)
    characterActive: boolean[]  // 每个成员是否启用
    autoMode: boolean           // 自动模式开关
    orderByOrder: boolean       // 是否按固定顺序发言
    oneAtTime: boolean          // 是否一次只让一个角色说
    // ...
}
```

**关键设计**：
- **角色和群聊统一存储**：`db.characters` 数组中同时存放 `character` 和 `groupChat`，通过 `type` 字段区分。这简化了数据管理。
- **角色多聊天页**：`chats[]` + `chatPage` 支持一个角色多个独立会话线程。
- **chaId**：UUID 作为全局唯一标识。
- **ccAssets**：支持 Character Card 的附加资源（如不同服装立绘）。

**对 AgentStage 的参考价值**：
- ✅ **统一存储设计**：AgentStage 可以借鉴"角色和群聊统一管理"的思路。
- ✅ **角色字段设计**：`desc` (详细人设), `personality`, `scenario`, `systemPrompt` 等字段命名可直接参考。
- ✅ **多聊天页**：`chats[]` + `chatPage` 设计支持 1对1 多会话，AgentStage 需求中"一个 Agent 可拥有多个独立会话"与此一致。
- ⚠️ **缺失"简易人设"**：RisuAI 没有显式的"给其他 Agent 看的简介"字段，AgentStage 的双人设是创新点，但 RisuAI 的 `desc` 可作为详细人设参考。

#### text-generation-webui — 文件式角色管理

**核心文件**：
- `reference/text-generation-webui/modules/chat.py` — 聊天核心
- `reference/text-generation-webui/modules/utils.py` — `get_available_characters()`

**角色存储**：YAML/JSON 文件存放在 `characters/` 目录下。

**关键设计**：
- 角色数据包含：name, context, greeting, example_dialogue, personality, scenario 等。
- 通过 `name1` (用户), `name2` (角色) 双角色名系统构建对话。
- `user_bio`：用户的自我介绍，会注入 prompt。

**对 AgentStage 的参考价值**：
- ✅ **user_bio 概念**：类似 AgentStage 的"用户人设"，可作为 P2 功能的参考。
- ⚠️ 文件式存储不适合 AgentStage 的 IM 式管理需求。

---

## 四、角色扮演 Prompt 构建与历史消息拼接

### 4.1 Prompt 构建架构对比

#### SillyTavern — PromptManager + 多阶段转换

**核心文件**：
- `reference/SillyTavern/public/scripts/PromptManager.js` — Prompt 管理器
- `reference/SillyTavern/public/scripts/openai.js` — OpenAI API 调用与 Message 组装 (TokenHandler, Message, MessageCollection 类)
- `reference/SillyTavern/src/prompt-converters.js` — 模型格式转换器
- `reference/SillyTavern/public/scripts/world-info.js` — Lorebook 动态注入

**Prompt 构建流程**：

1. **PromptManager** (`PromptManager.js`):
   - `Prompt` 类：每个 prompt 块有 `identifier`, `role`, `content`, `position`, `injection_position` (RELATIVE=0 / ABSOLUTE=1), `injection_depth`, `injection_order`
   - 支持 prompt 预设（main, nsfw, jailbreak, dialogueExamples, charPersonality, scenario, chatHistory, worldInfoBefore, charDescription, worldInfoAfter 等）
   - 通过 `identifier` 和 `order` 控制 prompt 块的排列顺序
   - 支持 `forbid_overrides` 防止被覆盖

2. **Message 组装** (`openai.js`):
   - `TokenHandler` 类：管理 token 预算，按优先级截断消息
   - `Message` 类：单条消息对象，含 role, content, name, token count 等
   - `MessageCollection` 类：消息集合，支持排序、截断、预算检查
   - 组装为 ChatML 格式（OpenAI messages 数组）

3. **格式转换** (`src/prompt-converters.js`):
   - 内部统一使用 ChatML 格式
   - 调用 API 前通过 `postProcessPrompt()` 转换为各模型专有格式：
     - `PROMPT_PROCESSING_TYPE.MERGE` → 合并消息（Claude 兼容）
     - `PROMPT_PROCESSING_TYPE.STRICT` → 严格角色标记
     - `PROMPT_PROCESSING_TYPE.STRICT_TOOLS` → 严格角色标记 + 工具支持
   - 支持 Claude/Gemini/Cohere/Mistral/Custom 等多种适配

4. **Lorebook 注入** (`world-info.js`):
   - `WIEntry` 类：每条 lore 有 `keys`（触发关键词数组）、`content`（注入内容）
   - `WorldInfoScanData`：扫描当前上下文，匹配关键词后动态插入相关 lore
   - 支持按角色描述、人格、场景匹配
   - `world_info_character_strategy`：控制角色 lore 的插入策略

**对 AgentStage 的参考价值**：
- ✅ **Prompt 分块管理**：AgentStage 的 Prompt 拼接（系统→自身人设→参与者→历史→最新消息）可以参考 `Prompt` 类的分块思想，将每一层作为一个独立 prompt 块。
- ✅ **TokenHandler 预算管理**：当历史消息过长时，需要按优先级截断。SillyTavern 的 `TokenHandler` 提供了成熟的预算分配算法。
- ✅ **模型格式转换**：如果 AgentStage 计划支持多种模型（OpenAI/Claude/Gemini），`prompt-converters.js` 的转换逻辑非常值得参考。
- ✅ **Lorebook 动态注入**：AgentStage 的"可见消息历史"可以借鉴 Lorebook 的"按需注入"思想，但 AgentStage 更简单（全量注入关联角色信息）。
- ⚠️ **复杂度控制**：SillyTavern 的 Prompt 系统极其复杂（2144 行的 PromptManager），AgentStage 的 Prompt 拼接规则是固定的 5 层，不需要如此灵活的配置系统。

#### RisuAI — formatingOrder + PromptItem 类型系统

**核心文件**：
- `reference/RisuAI/src/ts/process/prompt.ts` — PromptItem 类型定义与 token 计算
- `reference/RisuAI/src/ts/process/index.svelte.ts` — `sendChat()` Prompt 组装核心 (line 67+)

**Prompt 构建流程**：

1. **PromptItem 类型系统** (`prompt.ts`)：
   ```typescript
   export type PromptItem = 
     | PromptItemPlain      // type: 'plain'|'jailbreak'|'cot'
     | PromptItemTyped      // type: 'persona'|'description'|'lorebook'|'postEverything'|'memory'
     | PromptItemChat       // type: 'chat', rangeStart/End
     | PromptItemAuthorNote // type: 'authornote'
     | PromptItemChatML     // type: 'chatML'
     | PromptItemCache;     // type: 'cache'
   ```
   - 每种类型都有 `role` (user/bot/system) 和可选 `name`
   - `tokenizePreset()` 计算整个 prompt 的 token 数
   - 支持 SillyTavern 预设导入 (`stChatConvert()`)

2. **sendChat 组装逻辑** (`index.svelte.ts:317+`)：
   ```typescript
   let unformated = {
     'main': ([] as OpenAIChat[]),          // 主系统提示词
     'jailbreak': ([] as OpenAIChat[]),     // 越狱提示词
     'chats': ([] as OpenAIChat[]),         // 聊天历史
     'lorebook': ([] as OpenAIChat[]),      // Lorebook 内容
     'globalNote': ([] as OpenAIChat[]),    // 全局备注
     'authorNote': ([] as OpenAIChat[]),    // 作者备注
     'lastChat': ([] as OpenAIChat[]),      // 最后一条消息
     'description': ([] as OpenAIChat[]),   // 角色描述
     'postEverything': ([] as OpenAIChat[]),// 后置指令
     'personaPrompt': ([] as OpenAIChat[])  // 用户人设
   }
   ```

3. **按 formatingOrder 排序** (`database.svelte.ts:65`)：
   ```typescript
   formatingOrder: ['main','description','personaPrompt','chats','lastChat','jailbreak','lorebook','globalNote','authorNote']
   ```
   - 将 `unformated` 中的各块按此顺序拼接为最终的 `OpenAIChat[]`

4. **角色描述组装** (`index.svelte.ts:430+`)：
   ```typescript
   let description = risuChatParser(
     (DBState.db.promptPreprocess ? DBState.db.descriptionPrefix : '') + currentChar.desc, 
     {chara: currentChar}
   )
   if(currentChar.personality) {
     description += risuChatParser("\n\nDescription of {{char}}: " + currentChar.personality, {chara: currentChar})
   }
   if(currentChar.scenario) {
     description += risuChatParser("\n\nCircumstances and context of the dialogue: " + currentChar.scenario, {chara: currentChar})
   }
   ```

5. **群聊特殊处理** (`index.svelte.ts:452+`)：
   ```typescript
   if(nowChatroom.type === 'group'){
     const systemMsg = `[Write the next reply only as ${currentChar.name}]`
     unformated.postEverything.push({ role: 'system', content: systemMsg })
   }
   ```

6. **Template 自定义** (`prompt.ts` + `index.svelte.ts`):
   - 支持用户自定义 `promptTemplate` (PromptItem 数组)
   - `utilityBot` 模式使用固定简化模板

**对 AgentStage 的参考价值**：
- ✅ **分层组装思想**：AgentStage 的 5 层 Prompt 拼接（系统→自身人设→参与者→历史→最新消息）与 RisuAI 的 `unformated` + `formatingOrder` 模式高度吻合。
- ✅ **`risuChatParser`**：变量替换系统（`{{char}}`, `{{user}}` 等），AgentStage 可以设计类似的模板变量（`{{char}}`, `{{user}}`, `{{group}}` 等）。
- ✅ **群聊身份锁定指令**：`[Write the next reply only as ${currentChar.name}]` 是非常实用的设计，AgentStage 应在群聊 Prompt 中加入类似指令防止角色串台。
- ✅ **Token 预算管理**：`ChatTokenizer` 类在 `index.svelte.ts` 中被用来管理 maxContext，AgentStage 需要类似的机制防止超长上下文。
- ⚠️ **formatingOrder 过于灵活**：AgentStage 的 Prompt 顺序是固定的，不需要用户可配置的 `formatingOrder`。

#### text-generation-webui — Jinja2 Template 渲染

**核心文件**：
- `reference/text-generation-webui/modules/chat.py` — `generate_chat_prompt()` (line 338+)

**Prompt 构建流程**：

1. **Jinja2 模板引擎**：
   ```python
   jinja_env = ImmutableSandboxedEnvironment(
       trim_blocks=True, lstrip_blocks=True, extensions=[loopcontrols]
   )
   ```
   - 使用 `ImmutableSandboxedEnvironment` 安全渲染用户提供的模板
   - `_template_cache` 缓存编译后的模板，避免重复编译

2. **双模板系统**：
   - `instruction_template_str`：指令模式模板（如 Alpaca, Vicuna, ChatML 等）
   - `chat_template_str`：聊天模式模板
   - 模板中包含变量：`{{ name1 }}`, `{{ name2 }}`, `{{ user_bio }}`, `{{ context }}`, `{{ messages }}` 等

3. **消息渲染器**：
   ```python
   instruct_renderer = partial(
       instruction_template.render,
       builtin_tools=None,
       tools=state['tools'] if 'tools' in state else None,
       add_generation_prompt=False,
       enable_thinking=state['enable_thinking'],
       ...
   )
   ```

4. **历史消息格式**：
   ```python
   history = history_data['internal']  # [[user_msg, assistant_msg, tool_msg, metadata], ...]
   ```
   - 历史以二维数组存储，每个元素是一个回合 `[user, assistant, tool, meta]`

**对 AgentStage 的参考价值**：
- ✅ **Jinja2 模板安全渲染**：如果 AgentStage 计划让用户自定义 Prompt 模板，text-generation-webui 的 `ImmutableSandboxedEnvironment` 是安全渲染的最佳实践。
- ✅ **instruction template 概念**：不同模型需要不同的 prompt wrapper（如 ChatML, Llama-2, Alpaca），text-generation-webui 的模板系统非常成熟。
- ⚠️ **Jinja2 是 Python 库**：AgentStage 如果用 Tauri/Rust，可以用 Rust 的模板引擎（如 Handlebars, Tera）替代。SillyTavern 前端就使用了 Handlebars (`package.json` 中有 `handlebars`)。

---

## 五、历史消息管理与可见性

### 5.1 各项目历史消息存储对比

| 项目 | 存储位置 | 消息结构 | 可见性设计 |
|------|---------|---------|-----------|
| **SillyTavern** | 每个角色/群聊独立的 JSON 文件 (`chats/`) | `{name, is_user, mes, send_date, extra, ...}` | 全局可见（单用户视角），群聊中所有成员共享同一份历史 |
| **RisuAI** | `character.chats[chatPage].message[]` | `{role, data, saying?, time?, generationInfo?}` | 群聊中每个角色独立调用，但共享同一份 `message[]` |
| **text-generation-webui** | `history['internal']` 二维数组 | `[user_msg, assistant_msg, tool_msg, metadata]` | 全局可见 |
| **AgentStage (设计)** | SQLite 表 | 待设计 | **每个 Agent 独立维护可见历史**（差异化设计） |

### 5.2 对 AgentStage 的参考

AgentStage 的核心差异化设计是"每个 Agent 独立维护可见消息历史"。这在参考项目中**没有完全相同的实现**，但有部分可参考的机制：

- **SillyTavern 的 `chat_metadata`**：每条消息有 `extra` 字段存储元数据，AgentStage 可以用类似的机制标记消息对哪些 Agent 可见。
- **RisuAI 的 `saying` 字段**：`message.saying` 记录消息是哪个角色说的（在群聊中很重要）。AgentStage 的消息表应包含 `sender_id`, `sender_type` 字段。
- **text-generation-webui 的 metadata**：`history[i][3]` 存储回合级元数据，AgentStage 可以在 metadata 中记录"这条消息是否已同步给某 Agent"。

---

## 六、工具搭建（Tool / Function Calling）

### 6.1 各项目工具系统对比

#### SillyTavern — ToolManager 类 + 前端注册

**核心文件**：
- `reference/SillyTavern/public/scripts/tool-calling.js` (1163 行)

**架构**：
```javascript
class ToolDefinition {
    #name; #displayName; #description; #parameters; #action; #formatMessage; #shouldRegister; #stealth;
    toFunctionOpenAI() { /* 转为 OpenAI function format */ }
    async invoke(parameters) { return await this.#action(parameters); }
}

class ToolManager {
    static #tools = new Map();           // 工具注册表
    static RECURSE_LIMIT = 5;            // 最大递归调用深度
    
    static registerFunctionTool({name, displayName, description, parameters, action, ...})
    static unregisterFunctionTool(name)
    static async registerFunctionToolsOpenAI(generate_data)  // 注入到 API 请求
    static async handleToolCalls(response)  // 处理模型返回的 tool_calls
}
```

**关键设计**：
- **前端注册**：工具在浏览器端注册，action 是前端 JavaScript 函数。
- **OpenAI Format**：`toFunctionOpenAI()` 生成标准 OpenAI function schema。
- **Stealth Tools**：`stealth=true` 时工具调用不显示在聊天中，也不触发后续生成。
- **Recurse Limit**：防止模型无限递归调用工具，默认限制 5 次。
- **Tool Invocation 展示**：工具调用结果以特殊消息格式展示在聊天中。
- **与 API 集成**：`openai.js:2780` 在生成前调用 `ToolManager.registerFunctionToolsOpenAI(generate_data)` 将工具注入请求。

**对 AgentStage 的参考价值**：
- ✅ **ToolManager 设计模式**：AgentStage 的 `send_message` 工具可以借鉴 `ToolDefinition` + `ToolManager` 的注册/调用模式。
- ✅ **Recurse Limit**：AgentStage 的防循环保护可以参考 `RECURSE_LIMIT = 5` 的思想（但 AgentStage 用的是消息上限+时间间隔，更粗粒度）。
- ✅ **Stealth 概念**：如果未来需要"系统级工具"（如自动总结），可以借鉴 stealth 设计。
- ⚠️ **前端 action 不适合 AgentStage**：SillyTavern 的工具 action 在前端执行，AgentStage 的 `send_message` 是系统级操作（写入数据库），应在后端/系统层执行。

#### RisuAI — MCP (Model Context Protocol) 客户端

**核心文件**：
- `reference/RisuAI/src/ts/process/mcp/mcp.ts` — MCP 客户端管理
- `reference/RisuAI/src/ts/process/mcp/mcplib.ts` — MCP 协议库
- `reference/RisuAI/src/ts/process/mcp/internalmcp.ts` — 内部 MCP 实现
- `reference/RisuAI/src/ts/process/mcp/risuaccess/` — RisuAI 专用 MCP 接口
- `reference/RisuAI/src/ts/process/infunctions.ts` — 内部函数工具

**架构**：
```typescript
// MCP 客户端注册表
export const MCPs: Record<string, MCPClient|MCPClientLike> = {};

// 初始化时连接各种 MCP 服务器
export async function initializeMCPs(additionalMCPs?: string[]) {
    // 1. internal: 内置 MCP (filesystem, risuai, aiaccess, googlesearch, graphmem, dice)
    // 2. stdio: 本地子进程 MCP (仅限 Tauri 桌面版)
    // 3. plugin: 插件提供的 MCP
}

// MCPClient 类处理 JSON-RPC 通信
class MCPClient {
    async callTool(name: string, args: any): Promise<RPCToolCallContent>
    async listTools(): Promise<MCPTool[]>
}
```

**内置 MCP 工具**：
- `internal:risuai` — RisuAI 内部数据访问（角色、聊天读写）
- `internal:fs` — 文件系统访问
- `internal:aiaccess` — AI 模型访问
- `internal:googlesearch` — 谷歌搜索
- `internal:graphmem` — 图记忆
- `internal:dice` — 骰子

**关键设计**：
- **MCP 是 Anthropic 推出的开放标准**，RisuAI 是角色扮演客户端中 MCP 支持最完整的。
- **stdio MCP**：通过 Tauri 的 `Command.create()` 启动本地子进程，实现 JSON-RPC 通信。这充分利用了 Tauri 桌面端的能力。
- **Plugin MCP**：允许第三方插件注册自定义 MCP 服务器。
- **请求层集成**：`request.ts` 中 `requestDataArgument` 支持 `tools?: MCPTool[]`，在调用 LLM 时传入工具定义。

**对 AgentStage 的参考价值**：
- ✅ **MCP 标准**：AgentStage 的工具系统可以**直接采用 MCP 标准**，这样未来可扩展性极强。RisuAI 的 `mcp.ts` 和 `mcplib.ts` 是完整的参考实现。
- ✅ **内部 MCP**：`internal:risuai` 的设计思路与 AgentStage 的 `send_message` 非常相似——提供一个内部 MCP 让 Agent 读写应用数据。
- ✅ **Tauri + stdio MCP**：如果 AgentStage 采用 Tauri，可以参考 RisuAI 如何用 `@tauri-apps/plugin-shell` 的 `Command.create()` 实现 stdio MCP。
- ⚠️ **MCP 复杂度**：MCP 是通用协议，AgentStage 当前只需要 `send_message` 一个工具，引入完整 MCP 可能过度设计。可以**先实现简化版 ToolManager**（类似 SillyTavern），未来再迁移到 MCP。

#### text-generation-webui — Python 脚本工具 + MCP 服务器

**核心文件**：
- `reference/text-generation-webui/modules/tool_use.py` — 工具加载与 MCP 连接
- `reference/text-generation-webui/modules/tool_parsing.py` — 工具调用解析

**架构**：
```python
# 1. 用户自定义 Python 工具
# user_data/tools/my_tool.py:
#   tool = { "type": "function", "function": { "name": "...", "description": "...", "parameters": {...} } }
#   def execute(arguments): ...

def load_tools(selected_names):
    """动态导入 user_data/tools/*.py"""
    
# 2. MCP 服务器连接
async def _connect_mcp_server(server):
    """连接 MCP 服务器并发现工具"""
    
# 3. 用户审批
def request_tool_approval(session_key, tool_name):
    """阻塞直到用户批准/拒绝工具调用"""
```

**工具解析** (`tool_parsing.py`)：
- 支持 **15+ 种 tool call 格式**：`<tool_call>`, `<function_call>`, `[TOOL_CALLS]`, `to=functions.`, `<|tool_call_begin|>`, channel-based 等。
- `streaming_tool_buffer_check()`：流式生成时检测是否出现 tool call 标记，决定是否缓冲输出。
- `_extract_balanced_json()`：从文本中提取完整的 JSON 参数对象。

**对 AgentStage 的参考价值**：
- ✅ **用户审批机制**：`request_tool_approval()` 的设计很有价值。AgentStage 可以考虑在敏感操作（如发送消息到跨群聊）前加入用户确认。
- ✅ **多格式工具解析**：如果 AgentStage 计划支持多种本地模型（而非仅 API），`tool_parsing.py` 对各种 tool call 格式的兼容处理非常值得参考。
- ⚠️ Python 动态加载：AgentStage 如果用 Tauri/TS，不需要 Python 的动态 import。

### 6.2 AgentStage 工具系统设计建议

综合以上分析，AgentStage 的工具系统可以采用**渐进式方案**：

**Phase 1（MVP）**：简化版 ToolManager（参考 SillyTavern）
```typescript
// 仅实现 send_message 工具
class ToolManager {
    static registerSendMessageTool()
    static async handleToolCalls(response): Promise<void>
    // 调用后直接将消息写入目标会话，不返回结果给 Agent
}
```

**Phase 2（扩展）**：迁移到 MCP（参考 RisuAI）
```typescript
// 引入 MCP 客户端，支持第三方工具
class MCPManager {
    async connectInternalMCP()  // internal:agentstage
    async connectStdioMCP(config)  // 仅限 Tauri 桌面版
}
```

---

## 七、群聊自动触发机制

### 7.1 各项目群聊设计对比

#### SillyTavern — Auto Mode + Activation Strategy

**核心文件**：
- `reference/SillyTavern/public/scripts/group-chats.js` (2490 行)

**群聊自动模式**：
```javascript
// Auto Mode Worker：定时轮询
function setAutoModeWorker() {
    const autoModeDelay = group?.auto_mode_delay ?? DEFAULT_AUTO_MODE_DELAY;  // 默认 5 秒
    autoModeWorker = setInterval(groupChatAutoModeWorker, autoModeDelay * 1000);
}

async function groupChatAutoModeWorker() {
    if (!is_group_automode_enabled || online_status === 'no_connection') return;
    // ... 检查状态
    await generateGroupWrapper(true, 'auto', { signal });
}
```

**Activation Strategy**（决定哪些角色被触发）：
```javascript
export const group_activation_strategy = {
    NATURAL: 0,   // 自然顺序：基于消息内容智能选择回应角色
    LIST: 1,      // 列表顺序：按成员列表固定顺序
    MANUAL: 2,    // 手动：仅当用户手动触发时
    POOLED: 3,    // 池化：所有成员并行决定，按响应顺序展示
};
```

**generateGroupWrapper**（逐个生成）：
```javascript
async function generateGroupWrapper(byAutoMode, type = null, params = {}) {
    // 1. 确定 activatedMembers（根据 activation strategy）
    // 2. 循环遍历 activatedMembers
    for (const chId of activatedMembers) {
        setCharacterId(chId);
        textResult = await Generate(generateType, { automatic_trigger: byAutoMode, ... });
    }
}
```

**关键设计**：
- **Auto Mode**：禁言关闭后，每隔 `auto_mode_delay` 秒（默认 5 秒）自动触发一次群聊生成。
- **逐个生成**：不是并行调用所有 Agent，而是依次生成每个角色的回复。
- **消息不自动触发**：Auto Mode 是定时轮询，不是"每条消息都触发"。

**对 AgentStage 的参考价值**：
- ✅ **Activation Strategy 概念**：AgentStage 可以参考"NATURAL/LIST/MANUAL"的策略分离，但 AgentStage 的设计更简单——每条消息触发所有其他 Agent（受间隔和上限约束）。
- ⚠️ **定时轮询 vs 事件驱动**：SillyTavern 的 Auto Mode 是定时轮询（每隔 N 秒），AgentStage 是事件驱动（每条消息触发）。AgentStage 的设计更符合 IM 自然体验，但需要更精细的防循环保护。
- ⚠️ **逐个生成 vs 并行**：SillyTavern 是逐个生成（串行），AgentStage 设计是并行触发所有 Agent。并行更符合"群聊秒回"的预期，但 API 并发成本更高。

#### RisuAI — talkness + 关键词匹配

**核心文件**：
- `reference/RisuAI/src/ts/process/group.ts`
- `reference/RisuAI/src/ts/process/index.svelte.ts` (群聊逻辑 line 263+)

**群聊发言顺序算法** (`group.ts:52`)：
```typescript
export function groupOrder(chars: GroupOrder[], input: string): GroupOrder[] {
    // 1. 关键词匹配：如果输入中包含某角色名，该角色优先
    for (const word of words) {
        for (let char of chars) {
            const charNameChunks = getWords(findCharacterbyId(char.id).name)
            if (charNameChunks.includes(word)) {
                order.push(char);  // 优先加入队列
                break;
            }
        }
    }
    
    // 2. 概率抽样：按 talkness 概率决定是否发言
    for (const char of shuffled) {
        const chance = char.talkness ?? 0.5
        if (chance >= Math.random()) {
            order.push(char);
        }
    }
    
    // 3. 保底：至少选一个角色
    while (order.length === 0) {
        order.push(chars[Math.floor(Math.random() * chars.length)]);
    }
    return order;
}
```

**群聊消息触发** (`index.svelte.ts:263+`)：
```typescript
if(nowChatroom.type === 'group'){
    // 计算发言顺序
    let order = groupOrder(order, lastMessage?.data)
        .filter(v => v.id !== lastMessage?.saying)  // 排除最后发言者
    
    // 依次调用 sendChat 为每个角色生成
    for(let i=0; i<order.length; i++){
        const r = await sendChat(order[i].index, { ... })
    }
}
```

**关键设计**：
- **talkness**：每个角色有发言欲望值（0-1），群聊时按概率决定是否回应。这实现了"不是每条消息都会得到回复"的自然节奏。
- **关键词匹配优先**：如果用户消息中提到了某角色名，该角色优先回应。这非常符合真实群聊的"@某人"行为。
- **排除最后发言者**：`v.id !== lastMessage?.saying` 防止同一角色连续自言自语。
- **characterActive**：`boolean[]` 控制每个成员是否启用。

**对 AgentStage 的参考价值**：
- ✅ **talkness 概率机制**：AgentStage 当前设计是"每条消息触发所有其他 Agent"，这可能导致回复过多。可以参考 RisuAI 的 `talkness` 思想，为每个 Agent 配置"回应概率"（但用户要求"自然触发"，概率机制可以增加自然感）。
- ✅ **关键词匹配优先**：AgentStage 可以在 Prompt 中注入"如果消息中提到了你，请优先回应"的指令，实现类似效果。
- ✅ **排除最后发言者**：AgentStage 应该防止同一 Agent 连续触发（这与全局最小间隔有重叠，但可以作为额外保护）。
- ✅ **characterActive**：AgentStage 的群聊也可以有"成员开关"，用户可临时禁用某 Agent 的自动触发（但保留在群中）。

---

## 八、Feature → 参考项目文件映射表

| AgentStage 功能 | 参考项目 | 参考文件路径 | 说明 |
|----------------|---------|-------------|------|
| **Agent 创建/编辑** | SillyTavern | `src/endpoints/characters.js` | 角色 CRUD API，PNG 内嵌元数据 |
| **Agent 创建/编辑** | RisuAI | `src/ts/characters.ts` | `createNewCharacter()`, `createNewGroup()` |
| **角色数据结构** | RisuAI | `src/ts/storage/database.svelte.ts:1304` | `character` / `groupChat` interface |
| **角色卡导入** | SillyTavern | `src/character-card-parser.js` | PNG chunk 解析 |
| **角色卡校验** | SillyTavern | `src/validator/TavernCardValidator.js` | V1/V2 校验逻辑 |
| **双人设（详细+简易）** | — | — | **无直接参考**，AgentStage 的创新点 |
| **Prompt 分层组装** | RisuAI | `src/ts/process/index.svelte.ts:317` | `unformated` + `formatingOrder` 模式 |
| **Prompt 类型系统** | RisuAI | `src/ts/process/prompt.ts` | `PromptItem` 接口定义 |
| **Prompt 分块管理** | SillyTavern | `public/scripts/PromptManager.js` | `Prompt` 类，注入位置/深度/排序 |
| **Token 预算管理** | SillyTavern | `public/scripts/openai.js:3325` | `TokenHandler` / `MessageCollection` 类 |
| **模型格式转换** | SillyTavern | `src/prompt-converters.js` | Claude/OpenAI/Gemini/Cohere 转换 |
| **Lorebook 动态注入** | SillyTavern | `public/scripts/world-info.js` | 关键词匹配 + 动态插入 |
| **群聊自动模式** | SillyTavern | `public/scripts/group-chats.js:1398` | `groupChatAutoModeWorker`, `generateGroupWrapper` |
| **群聊发言顺序** | RisuAI | `src/ts/process/group.ts:52` | `groupOrder()`, talkness + 关键词匹配 |
| **群聊身份锁定** | RisuAI | `src/ts/process/index.svelte.ts:452` | `[Write the next reply only as ${name}]` |
| **工具注册/调用** | SillyTavern | `public/scripts/tool-calling.js` | `ToolManager`, `ToolDefinition` |
| **MCP 工具系统** | RisuAI | `src/ts/process/mcp/mcp.ts` | MCP 客户端管理 |
| **MCP 协议实现** | RisuAI | `src/ts/process/mcp/mcplib.ts` | JSON-RPC 通信 |
| **工具解析（多格式）** | text-generation-webui | `modules/tool_parsing.py` | 15+ 种 tool call 格式解析 |
| **用户审批机制** | text-generation-webui | `modules/tool_use.py:54` | `request_tool_approval()` |
| **Jinja2 模板渲染** | text-generation-webui | `modules/chat.py:115` | `ImmutableSandboxedEnvironment` |
| **历史消息存储** | RisuAI | `src/ts/storage/database.svelte.ts` | `Chat.message[]` 结构 |
| **Tauri 桌面配置** | RisuAI | `src-tauri/tauri.conf.json` | nsis 打包、自动更新、deep link |
| **Svelte 状态管理** | RisuAI | `src/ts/stores.svelte.ts` | Svelte Store + `DBState` |
| **API 请求封装** | RisuAI | `src/ts/process/request/request.ts` | 统一 request 接口 |
| **变量替换系统** | RisuAI | `src/ts/process/index.svelte.ts` | `risuChatParser()` — `{{char}}`, `{{user}}` |

---

## 九、关键结论与建议

### 9.1 最值得深度参考的项目

1. **RisuAI**（⭐⭐⭐⭐⭐）
   - **技术栈完全匹配**：Tauri + Svelte + TS 是 AgentStage 的最佳选择。
   - **架构最接近**：角色/群聊统一存储、Prompt 分层组装、群聊自动触发。
   - **代码质量高**：TypeScript 类型完整，模块化清晰。
   - **文件路径**：`src/ts/characters.ts`, `src/ts/process/index.svelte.ts`, `src/ts/storage/database.svelte.ts`, `src-tauri/tauri.conf.json`

2. **SillyTavern**（⭐⭐⭐⭐）
   - **功能最全面**：Token 预算管理、模型格式转换、Lorebook、PromptManager 等非常成熟。
   - **生态标准**：Tavern Card V2 是角色卡的事实标准。
   - **注意**：前端技术栈老旧，主要参考其**业务逻辑和算法**，而非代码直接复用。
   - **文件路径**：`public/scripts/tool-calling.js`, `src/prompt-converters.js`, `public/scripts/PromptManager.js`, `public/scripts/world-info.js`

3. **text-generation-webui**（⭐⭐⭐）
   - **工具系统最完整**：MCP 支持、用户审批、多格式解析。
   - **Prompt 模板系统**：Jinja2 安全渲染是业界最佳实践。
   - **注意**：定位是推理后端，不是角色扮演客户端，前端架构不参考。
   - **文件路径**：`modules/tool_use.py`, `modules/tool_parsing.py`, `modules/chat.py`

### 9.2 AgentStage 的差异化设计点（无直接参考）

以下功能是 AgentStage 的核心创新，在参考项目中**没有完全相同的实现**：

| 功能 | 说明 | 建议实现方式 |
|------|------|-------------|
| **每个 Agent 独立可见历史** | 参考项目都是全局共享一份历史 | 在消息表中增加 `visible_to_agent_ids` 字段，或按 Agent 维护独立视图 |
| **好友关系驱动参与者简介** | 参考项目群聊中所有成员互相可见 | 在数据库中维护 `friendships` 表，Prompt 组装时按好友+群友筛选 |
| **全局最小触发间隔 + 消息积压** | SillyTavern 有 auto_mode_delay（固定轮询），RisuAI 无间隔 | 每个 Agent 维护 `last_trigger_time` + `pending_queue`，定时器到期后批量处理 |
| **私聊/群聊消息上限 + 用户重置** | 参考项目无此机制 | 会话级计数器，用户消息重置 |
| **双人设（详细+简易）** | 参考项目只有单一人设 | `character` 表中增加 `detailed_persona` 和 `simplified_persona` 两个字段 |
| **强制 Function Calling + 无 Fallback** | 参考项目有 fallback 或兼容模式 | 配置时检测模型能力，不支持则阻断 |

### 9.3 下一步行动建议

1. **技术栈锁定**：基于 RisuAI 的参考，确认 Tauri v2 + Svelte 5 + Vite + TypeScript 方案。
2. **数据库 Schema 设计**：参考 RisuAI 的 `database.svelte.ts`，设计 SQLite 表结构（角色表、群聊表、消息表、好友关系表、会话设置表）。
3. **Prompt 组装原型**：参考 RisuAI 的 `unformated` + `formatingOrder` 模式，实现 AgentStage 的 5 层 Prompt 拼接原型。
4. **群聊调度器原型**：参考 SillyTavern 的 `generateGroupWrapper` + RisuAI 的 `groupOrder`，实现全局间隔 + 消息积压的调度器。
5. **角色卡导入**：参考 SillyTavern 的 `character-card-parser.js`，实现 Tavern Card V2 导入功能。

---

*文档结束*
