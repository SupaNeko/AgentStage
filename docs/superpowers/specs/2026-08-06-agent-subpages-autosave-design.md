# 角色配置子页面 UI 统一 + 自动保存 设计文档

日期：2026-08-06
状态：已获用户批准（方案 B：自动保存 + 保留手动保存按钮）

## 背景与目标

角色详情页（`AgentDetail.svelte`）包含六个 tab：角色配置、关系设定、记忆、定时任务、表情包、语音。目前各子页面保存方式不一致：

- 角色配置（参照格式）：底部固定 footer（不随内容滚动），保存按钮在右下角，手动保存。
- 关系设定、记忆：无保存按钮，已有防抖 + 失焦自动保存。
- 定时任务：列表型页面，增删改通过弹窗/按钮即时落库，无表单保存按钮。
- 表情包：保存按钮内嵌在滚动内容区，无自动保存。
- 语音：保存/删除按钮内嵌在滚动内容区，无自动保存。

目标：五个子页面的保存按钮统一固定到页面右下角 footer（与角色配置页格式一致、不参与滚动），并全面增加自动保存；文本框内容在失焦后保存；自动保存成功后通过现有 `toastStore` 顶部滑动弹窗提示"已自动保存"。

## 1. 底部固定 Footer（AgentDetail.svelte）

现有布局已满足"不参与翻页"：内容区 `flex-1 overflow-y-auto`，footer 固定在底部。改动：

- 保存按钮不再只在"角色配置"tab 显示。**关系设定 / 记忆 / 表情包 / 语音** 四个 tab 的右下角都显示同款保存按钮（图标 + 文字、样式、位置与角色配置页一致）。
- 手动保存按钮通过 `bind:this` 调用子面板暴露的 `saveAll()` 方法，立即保存该面板当前全部内容，成功后 toast 提示（不受自动保存 toast 去重限制）。
- **定时任务** tab 不显示保存按钮：列表型页面无可保存表单，增删改已即时落库。
- 语音 tab 的 footer 左侧放红色"删除配置"按钮（仅当已存在语音配置时显示），格局与角色配置页一致：左侧辅助按钮、右侧 `[取消] [保存]`。

## 2. 各面板行为

### 关系设定（AgentRelationshipPanel.svelte）
- 保留现有防抖 500ms + 失焦立即保存。
- 自动保存成功后 toast 提示"已自动保存"（带去重，见第 3 节）。
- 新增 `saveAll()` 导出方法：清除所有待执行防抖，立即保存全部关系条目。

### 记忆（AgentMemoryPanel.svelte）
- 保留现有防抖 + 失焦保存；启用开关、重置记忆维持即时生效。
- 自动保存成功 toast 提示。
- 新增 `saveAll()`：立即保存长期记忆 + 全部"对他人的记忆"条目。

### 定时任务（AgentTimerPanel.svelte）
- 不改。footer 对该 tab 不显示保存按钮。

### 表情包（AgentStickerPackPanel.svelte）
- 移除内容区内嵌的保存按钮。
- 勾选/取消勾选变更即自动保存（300ms 防抖，合并快速连续点击），成功 toast 提示。
- 新增 `saveAll()`：立即保存当前勾选集合。

### 语音（AgentVoicePanel.svelte）
- 移除内容区"操作"行中的保存、删除配置按钮（挪到 footer）；"查看语音缓存"开关保留在内容区。
- 自动保存触发时机：
  - 下拉框（模型、说话人、输出语言、翻译模型）、复选框（自动翻译）、单选（生成时机）：`change` 即保存。
  - 语速滑块：`change`（松手）后保存，拖拽中不保存。
  - 情感参数文本框：失焦后保存。
- 校验：未选择模型时自动保存跳过，toast 提示需先选择模型；手动保存保留现有的错误提示行为。
- 新增 `saveAll()`：等同于现有 `handleSave()`。

## 3. 自动保存提示（toast）

- 复用现有 `toastStore`：成功后 `toastStore.success('已自动保存', 1500)`。
- 去重：同一面板 2 秒内重复触发的自动保存不再重复弹 toast（避免连续输入刷屏），各面板记录 `lastAutoSaveToastAt` 时间戳判断。
- 手动保存按钮的提示不受去重限制。
- 保存失败沿用 `toastStore.error`（红色、需手动关闭）。

## 4. 实现要点

- 父子通信：Svelte 5 `bind:this` 引用子组件实例，子组件 `export function saveAll()`。`AgentDetail.svelte` 为每个 tab 保存对应引用，footer 按钮点击时调用当前 tab 的 `saveAll()`。
- 语音面板"删除配置"是否可见由面板内部 `hasExisting` 状态决定，通过 `export function` 或 `$bindable` prop 暴露给父组件控制 footer 渲染。
- 不改动任何 Rust 后端与 IPC 命令；仅前端改动。

## 5. 测试

- 前端 Vitest：
  - 表情包面板：勾选变更触发防抖自动保存（mock `invoke`，使用假定时器验证防抖与 `set_agent_sticker_packs` 调用）；`saveAll()` 立即保存。
  - 语音面板：失焦触发保存；未选模型时跳过保存；`saveAll()` 调用 `voiceStore.saveAgentVoice`。
- 验证命令：`npx svelte-check --tsconfig ./tsconfig.json`、`pnpm test`、`pnpm build`。

## 6. 影响范围

仅前端组件：`AgentDetail.svelte`、`AgentRelationshipPanel.svelte`、`AgentMemoryPanel.svelte`、`AgentStickerPackPanel.svelte`、`AgentVoicePanel.svelte`；新增测试文件。无后端、无数据库变更。
