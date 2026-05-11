# AgentStage

> 像使用微信一样轻松地与多个 AI 角色聊天、围观它们互动。

AgentStage 是一款 Windows 桌面多智能体角色扮演聊天应用。你可以创建多个 AI 角色，与它们进行 1 对 1 私聊，也可以把多个角色拉进同一个群聊，观看它们之间的自然互动。

---

## 技术栈

| 层级 | 技术 |
|-----|------|
| 桌面框架 | Tauri v2 (Rust + WebView2) |
| 前端 | Svelte 5 + Vite + TailwindCSS v4 |
| 后端 | Rust + SQLite (rusqlite) |
| 构建工具 | pnpm, Cargo |
| 测试 | Vitest (前端单元测试) + Playwright (E2E) |

---

## 目录结构

```
AgentStage/
├── src/                    # 前端源码 (Svelte 5)
│   ├── lib/                # 组件、Store、类型定义
│   ├── App.svelte          # 根组件
│   └── main.ts             # 入口文件
├── src-tauri/              # Rust 后端源码
│   ├── src/
│   │   ├── commands/       # Tauri IPC 命令 (暴露给前端调用)
│   │   ├── db/             # 数据库连接、Schema、迁移、Repository
│   │   ├── llm/            # LLM Provider、Prompt 组装、Tool 执行
│   │   ├── models/         # 数据结构定义
│   │   └── scheduler.rs    # 角色触发调度器
│   └── Cargo.toml
├── e2e/                    # Playwright 端到端测试
├── docs/                   # 产品文档 (PRD、功能清单、Schema 等)
├── reference/              # 参考项目源码 (SillyTavern、RisuAI 等)
├── static/                 # 静态资源 (图标等)
├── package.json            # 前端依赖与脚本
├── vite.config.js          # Vite 配置
├── tsconfig.json           # TypeScript 配置
├── playwright.config.ts    # Playwright 配置
└── vitest.config.js        # Vitest 配置
```

---

## 快速开始

### 环境要求

- Windows 10 (1809+) 或 Windows 11
- [Node.js](https://nodejs.org/) ≥ 18
- [pnpm](https://pnpm.io/) ≥ 8
- [Rust](https://rustup.rs/) 最新稳定版

### 安装依赖

```bash
# 安装前端依赖
pnpm install

# Rust 依赖在首次编译时自动下载
```

### 开发模式启动

```bash
# 一键启动完整开发环境（Vite + Rust 编译 + 打开窗口）
pnpm tauri dev
```

> 首次启动会自动创建本地数据库和日志目录在项目根目录的 `data/` 文件夹下。

### 常用命令

```bash
# 仅启动前端 Vite 开发服务器（不启动 Rust 后端）
pnpm dev

# 前端构建（输出到 dist/）
pnpm build

# Rust 类型检查（约 3 秒）
cd src-tauri && cargo check

# 仅运行 Rust 后端（会创建桌面窗口，但前端内容需自行提供）
cd src-tauri && cargo run

# Svelte 类型检查
npx svelte-check --tsconfig ./tsconfig.json

# 前端单元测试
pnpm test

# E2E 测试
pnpm exec playwright test e2e/
```

---

## 基础功能

### 1. 角色管理
- **创建角色**：填写名称、详细人设、简易人设，配置 LLM 模型和 API Key
- **双人设**：每个角色拥有「详细人设」（自己的 System Prompt）和「简易人设」（给其他角色看的简介）
- **模型配置**：支持 OpenAI、Claude、Kimi、MiniMax 等 OpenAI-compatible 端点
- **强制 Tool Calling**：不支持 Function Calling 的模型无法保存配置

### 2. 1 对 1 私聊
- 选择角色创建私聊会话
- 发送消息后，角色会在后台自动调用 LLM 生成回复
- 支持"正在输入中"提示

### 3. 群聊（基础支持）
- 创建群聊并添加多个角色
- 禁言开关：开启后仅记录消息，不自动触发角色；关闭后进入自动互动模式
- 群聊中多个角色会按全局最小间隔自然触发

### 4. 消息机制
- **全局最小触发间隔**：默认 30 秒，间隔内积压的消息一次性批量处理
- **消息上限**：私聊默认 20 条、群聊默认 30 条，达到上限后自动暂停，用户发消息即可续杯
- **乐观消息展示**：用户消息立即出现在气泡中，无需等待后端确认
- **send_message Tool**：角色通过 Function Calling 主动发送消息到指定会话

### 5. 数据存储
- **强制便携模式**：所有数据（数据库、日志、WebView2 缓存）统一存放在程序目录下的 `data/` 文件夹中
- 卸载时删除程序目录即可彻底清除所有数据
- 开发模式下数据存放在项目根目录 `data/`

---

## 测试

```bash
# 前端单元测试 (Vitest)
pnpm test

# E2E 测试 (Playwright)
pnpm exec playwright test e2e/

# Rust 单元测试
cd src-tauri && cargo test
```

---

## 注意事项

1. **API Key 安全**：API Key 在 Rust 后端使用 AES-256-GCM 加密存储，前端永远不会收到密文
2. **LLM 调用全部走后端**：前端不直接调用任何 LLM API，所有请求通过 Tauri IPC 由 Rust 代理执行
3. **数据库迁移**：修改 `src-tauri/src/db/schema.rs` 后，需要在 `src-tauri/src/db/migration.rs` 中添加迁移脚本
4. **Windows 专用**：当前仅支持 Windows 平台（依赖 WebView2）

---

## 相关文档

- [功能清单与实现状态](./docs/feature_list.md)
- [产品需求文档 (PRD)](./docs/PRD.md)
- [数据库 Schema](./docs/schema.md)
- [技术栈说明](./docs/tech-stack.md)
- [Agent 快速参考](./AGENTS.md)

---

*AgentStage — 让 AI 角色在聊天中自然相遇。*
