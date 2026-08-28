# 搜索 API + 虚拟时间 设计文档

日期：2026-08-28
状态：已获用户批准（搜索阶段工具调用上限为 20 轮）

## 功能 1：搜索 API + ReAct 人设生成

### 需求
- 通用设置页新增"搜索 API"设置：下拉选择国内厂商（博查 / 智谱 / Kimi），填入 API Key
- AI 生成人设页面新增"启用搜索"勾选框：未配置时置灰不可选
- 勾选后，人设生成逻辑改为 ReAct 机制：AI 可多轮调用 `web_search` 工具搜集资料（上限 20 轮），最终输出人设
- 网络超时 / 连接失败 / API Key 无效等错误必须显式告知用户

### 数据层（MIGRATION_V26）
`app_settings` 表新增列：
- `search_provider TEXT` — `'bocha' | 'zhipu' | 'kimi'`
- `search_api_key_encrypted TEXT` — 复用现有 AES-256-GCM 加密；`SettingsResponse` 只暴露 `search_provider` + `search_api_key_set: bool`，永不返回明文 Key

### 后端
- 新增 `src-tauri/src/search/` 模块：
  - `SearchProvider` trait：`async fn search(&self, query: &str) -> Result<String, SearchError>`，返回格式化搜索结果文本（标题/链接/摘要）
  - 三个实现：博查 web-search API、智谱 web_search API、Kimi(Moonshot) `$web_search` 内置工具
  - `SearchError` 枚举：网络超时/连接失败（15s 超时）、Key 无效（401/403）、限流（429）、其他厂商错误，各自带明确中文提示
- 新增 `web_search` 工具 schema，仅供人设生成使用，不加入聊天的 `get_all_tool_schemas()`
- 搜索阶段为前置独立阶段：在原有两步（提取字段 → 写人设）之前插入 ReAct 循环，搜集资料汇总为 `<search_material>` 注入 Step1 用户消息；现有两步逻辑不动
- 网络/Key 错误中断生成并显式返回前端；搜索无结果作为工具结果返回，让 AI 换关键词继续
- 新增 `test_search_api` 命令，供设置页"测试连接"按钮主动验证网络和 Key
- 切换厂商时清空已存 Key

### 前端
- `SettingsPanel` 新增"搜索 API"区块：厂商下拉 → Key 输入 → 保存 + 测试连接（显示成功/具体失败原因）
- `PersonaGenerateModal` / `CreateAgentModal`：新增"启用搜索"勾选框；未配置时置灰并提示"请先在通用设置中配置搜索 API"；生成失败时显示后端返回的具体错误

## 功能 2：虚拟时间

### 需求
- 通用设置页新增虚拟时间功能：自定义设定时间 + 自定义流速（现实 1 分钟 = 虚拟 N 分钟，整数）
- 虚拟时间随流速持续变更；用户可随时更新设定时间和流速
- 勾选启用后，给 AI 角色注入的时间以虚拟时间为准（仅提示词注入范围；定时器、免打扰等仍用真实时间）

### 数据层（同 MIGRATION_V26）
`app_settings` 新增列：
- `virtual_time_enabled INTEGER DEFAULT 0`
- `virtual_time_base INTEGER` — 用户设定的虚拟时间（ms 时间戳）
- `virtual_time_set_at INTEGER` — 设定那一刻的真实时间（ms 时间戳）
- `virtual_time_rate INTEGER DEFAULT 1` — 现实 1 分钟 = 虚拟 N 分钟

### 后端
- 纯函数计算：`虚拟当前时间 = base + (真实now - set_at) × rate`
- `prompt.rs` 注入点读设置：启用时系统提示词 `{current_time}` 与消息历史时间戳均用虚拟时间换算；未启用维持 `Local::now()`

### 前端
- `SettingsPanel` 新增"虚拟时间"区块：启用勾选 → 日期时间选择器 → 流速整数输入（≥1）→ 实时跳动的"当前虚拟时间"预览（前端用同一公式每秒本地刷新，不轮询后端）
- 连续性保证：只改流速时，前端把当前正在显示的虚拟时间作为新 base 提交，时间不跳变

## 测试
- Rust 单测：虚拟时间换算公式（含改流速连续性）、搜索错误分类映射、search material 注入
- 设置读写：加密 Key 不泄露、切换厂商清 Key
- 测试随功能代码一起提交
