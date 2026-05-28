# 主题开发指�?

> 本文档面向希望为 AgentStage 创建自定义主题的用户或开发者�?
> 阅读时间：约 10 分钟�?

---

## 1. 快速开始（只需 3 步）

AgentStage 的主题采�?*纯文件包**形式，无需编译或打包工具�?

### �?1 步：新建主题目录

�?`data/themes/user/` 下创建以主题 ID 命名的文件夹�?

```
data/themes/user/my-theme/
```

### �?2 步：编写 `theme.json` �?`style.css`

在目录中放入两个核心文件�?

```
data/themes/user/my-theme/
├── theme.json      �?主题元数据（名称、作者、标签等�?
├── style.css       �?主题样式（设计令�?+ 组件覆盖�?
└── preview.png     �?预览图（可选，推荐 256×160�?
```

`theme.json` 示例�?

```json
{
    "name": "我的主题",
    "id": "my-theme",
    "version": "1.0.0",
    "author": "你的名字",
    "description": "一段简短的主题描述",
    "tags": ["dark", "minimal"]
}
```

`style.css` 最小示例：

```css
@theme {
    --color-bg:             #1a1a2e;
    --color-surface:        #16213e;
    --color-border:         #0f3460;
    --color-text:           #e94560;
    --color-text-secondary: #a0a0a0;
    --color-primary:        #e94560;
    --color-primary-dark:   #c13651;
}
```

### �?3 步：重启 AgentStage 并切换主�?

保存文件后重启应用，进入**设置 �?主题**，你的主题会出现在列表中。点击即可即时生效�?

---

## 2. 文件结构说明

```
data/themes/
├── default/                    �?内置默认主题（勿删）
�?  ├── theme.json
�?  ├── style.css
�?  └── preview.png
├── wooden/             �?内置异世界告示板主题
�?  ├── theme.json
�?  ├── style.css
�?  └── preview.png
└── user/                       �?用户自定义主题目�?
    ├── my-theme/               �?你的主题
    �?  ├── theme.json
    �?  ├── style.css
    �?  └── preview.png
    └── another-theme/
        └── ...
```

| 文件 | 是否必填 | 说明 |
|------|---------|------|
| `theme.json` | **�?* | 主题元数据，决定名称、作者、标签等展示信息 |
| `style.css` | **�?* | 主题样式定义，通过 `@theme` 令牌和组件选择器覆盖默认外�?|
| `preview.png` | �?| 主题卡片预览图，推荐尺寸 **256×160**；缺失时显示渐变占位�?|

> **注意**：主�?ID（即目录名）必须全局唯一，且只能包含字母、数字、连字符 `-` 和下划线 `_`。不支持中文目录名�?

---

## 3. `theme.json` 格式详解

`theme.json` 是主题的"身份�?，AgentStage 通过它识别和展示主题�?

### 完整字段�?

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | `string` | �?| 主题显示名称，支持中文。例如：`"异世界告示板"` |
| `id` | `string` | �?| 主题唯一标识�?*必须与目录名完全一�?*。例如：`"wooden"` |
| `version` | `string` | �?| 语义化版本号。例如：`"1.0.0"` |
| `author` | `string` | �?| 作者名。例如：`"AgentStage"` �?`"你的名字"` |
| `description` | `string` | �?| 一句话描述，会显示在主题卡片上。建�?30 字以�?|
| `tags` | `string[]` | �?| 标签数组，用于分类筛选。例如：`["dark", "minimal", "fantasy"]` |

### 完整示例

```json
{
    "name": "深海暗色",
    "id": "deep-sea-dark",
    "version": "1.0.0",
    "author": "OceanDev",
    "description": "深蓝色调暗色主题，适合夜间长时间使�?,
    "tags": ["dark", "blue", "eye-care"]
}
```

### 常见错误

- `id` 与目录名不一�?�?主题无法被识�?
- `id` 包含特殊字符（如 `/`、`\`、`..`）→ 被系统拒绝加载（安全防护�?
- 缺少必填字段 �?主题不会出现在列表中

---

## 4. `style.css` 格式 —�?双层架构

主题 CSS �?*两个层级**组成，缺一不可�?

### Layer 1：设计令牌（Design Tokens�?

通过 Tailwind CSS v4 �?`@theme` 指令，覆写全局语义颜色变量�?

```css
@theme {
    --color-bg:             #f3f4f6;   /* 页面背景 */
    --color-surface:        #ffffff;   /* 卡片/面板背景 */
    --color-border:         #e5e7eb;   /* 边框/分割�?*/
    --color-text:           #1f2937;   /* 主要文字 */
    --color-text-secondary: #6b7280;   /* 次要文字 */
    --color-primary:        #3b82f6;   /* 主题�?强调�?*/
    --color-primary-dark:   #2563eb;   /* 主题色悬�?按下�?*/
}
```

**工作原理**：AgentStage 的组件大量使�?Tailwind 工具类（�?`bg-bg`、`text-primary`、`border-border`）。`@theme` 重新定义这些令牌后，所有组件会**自动**应用新颜色，无需逐个修改�?

### Layer 2：组件覆盖（Component Overrides�?

当仅靠颜色令牌不足以表达设计时，使用语义类名对具体组件进行精细化覆盖�?

```css
/* 示例：让左侧导航变成毛玻璃效�?*/
.left-nav {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(12px);
    border-right: 1px solid rgba(0, 0, 0, 0.08);
}

/* 示例：让消息气泡变成便签纸风�?*/
.msg-bubble {
    border-radius: 2px 12px 12px 12px;
    box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.1);
}
```

**为什么需要两层？**

| 层级 | 作用范围 | 使用场景 |
|------|---------|---------|
| Layer 1 `@theme` | 全局所有组�?| 换一套配色方案（如暗色模式） |
| Layer 2 组件选择�?| 特定 UI 区域 | 改变形状、字体、纹理、阴影、特殊布局 |

> **重要**：不需要写 `!important`。主�?CSS 是通过 `<style id="theme-active">` 注入�?`<head>` 的，它位�?Tailwind 编译后的 CSS **之后**，天然具有更高的层叠优先级�?

### 字体加载

如需使用网络字体，在 `style.css` 顶部通过 `@import` 引入�?

```css
@import url('https://fonts.googleapis.com/css2?family=Noto+Serif+SC:wght@400;600;700&display=swap');

html, body {
    font-family: 'Noto Serif SC', Georgia, serif;
}
```

如需离线字体，将 `.woff2` 文件放入主题目录，用 `@font-face` 引用�?

```css
@font-face {
    font-family: 'MyFont';
    src: url('./myfont.woff2') format('woff2');
    font-weight: 400;
}
```

---

## 5. 语义类名对照�?

AgentStage 在关键组件上预留了语义类名，主题 CSS 可通过这些类名精准控制对应区域�?

| 类名 | 所在组�?| 目标元素 | 主题可控制的内容 |
|------|---------|---------|----------------|
| `.left-nav` | `LeftNav` | 左侧导航栏根元素 `<nav>` | 背景色、边框、宽度、阴�?|
| `.nav-tab` | `LeftNav` | 每个导航标签按钮 | 图标颜色、激活态背景、悬停效�?|
| `.mid-panel` | `AgentList` / `SessionList` / `HistorySessionList` | 中间列表面板根元�?`<div>` | 背景、边框、阴�?|
| `.list-item` | `AgentList` / `SessionList` / `HistorySessionList` | 每一行列表项 | 悬停色、选中态、内边距、边�?|
| `.chat-view` | `ChatView` | 聊天区域根元�?`<div>` | 整体布局、背�?|
| `.chat-header` | `ChatView` | 顶部会话标题�?| 背景、边框、阴影、字�?|
| `.chat-input-area` | `ChatView` | 底部输入�?| 输入框背景、边框样式、阴�?|
| `.msg-bubble` | `MessageBubble` | 消息气泡根元�?| 形状（圆角）、阴影、最大宽度、字�?|
| `.msg-self` | `MessageBubble` | "�?发送的消息 | 对齐方式、专属颜色、边�?|
| `.msg-other` | `MessageBubble` | 对方发送的消息 | 对齐方式、专属颜色、边�?|
| `.modal-overlay` | 所有弹窗组�?| 弹窗遮罩�?| 背景�?透明度、模糊效�?|
| `.modal-card` | 所有弹窗组�?| 弹窗卡片主体 | 边框、阴影、圆�?|
| `.btn-primary` | `ChatView` �?| 主要操作按钮 | 形状、边框、阴影、按下动�?|
| `.input-field` | `ChatView` �?| 文本输入�?| 边框、圆角、聚焦光�?|

### 类名命名规则

- 全小写，单词间用连字�?`-` 连接（kebab-case�?
- **避免�?Tailwind 工具类重�?*（如不使�?`.flex`、`.text`、`.block` 等）
- 在组件中，语义类名附加在现有 Tailwind 类之后：`class="flex items-center gap-2 left-nav"`
- 主题 CSS 中可直接按单一类名选择：`.left-nav { ... }`

---

## 6. 常见定制技�?

### 6.1 修改配色（最简单）

只需调整 Layer 1 �?7 个颜色令牌，即可快速生成一套新主题�?

```css
@theme {
    --color-bg:             #0f172a;   /* 深蓝灰背�?*/
    --color-surface:        #1e293b;   /* 稍浅的卡片背�?*/
    --color-border:         #334155;   /* 低对比度边框 */
    --color-text:           #f1f5f9;   /* 近白文字 */
    --color-text-secondary: #94a3b8;   /* 灰色次要文字 */
    --color-primary:        #38bdf8;   /* 亮蓝强调�?*/
    --color-primary-dark:   #0ea5e9;   /* 悬停态深�?*/
}
```

**配色建议**�?
- 背景与卡片之间保持足够的明度差（建议 ΔL �?8�?
- 主题色（primary）占视觉面积不超�?10%，用于按钮、选中态、链�?
- 文字与背景的对比度建�?�?4.5:1，确保可读�?

### 6.2 修改字体

全局字体�?

```css
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

html, body {
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
}
```

仅修改消息气泡字体（保留系统字体用于 UI）：

```css
.msg-bubble {
    font-family: 'Georgia', 'Noto Serif SC', serif;
}
```

### 6.3 修改阴影风格

现代扁平风格（柔和弥散阴影）�?

```css
.msg-bubble {
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1);
}

.modal-card {
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
}
```

复古硬阴影风格：

```css
.msg-bubble {
    box-shadow: 4px 4px 0 rgba(0, 0, 0, 0.2);
}

.btn-primary {
    box-shadow: 0 4px 0 var(--color-primary-dark), 0 6px 12px rgba(0, 0, 0, 0.15);
}

.btn-primary:active {
    transform: translateY(2px);
    box-shadow: 0 2px 0 var(--color-primary-dark), 0 3px 6px rgba(0, 0, 0, 0.15);
}
```

### 6.4 添加背景纹理（纯 CSS�?

```css
body {
    background-image:
        radial-gradient(circle at 1px 1px, rgba(0,0,0,0.05) 1px, transparent 0);
    background-size: 20px 20px;
}
```

### 6.5 自定义滚动条

```css
::-webkit-scrollbar {
    width: 8px;
}
::-webkit-scrollbar-track {
    background: transparent;
}
::-webkit-scrollbar-thumb {
    background: var(--color-border);
    border-radius: 4px;
}
::-webkit-scrollbar-thumb:hover {
    background: var(--color-text-secondary);
}
```

---

## 7. 验证清单

完成主题开发后，请按以下清单逐项检查，确保主题能正确加载和显示�?

### 基础结构

- [ ] 主题目录位于 `data/themes/user/{theme-id}/`
- [ ] 目录名与 `theme.json` 中的 `id` 字段完全一�?
- [ ] 包含 `theme.json` �?`style.css` 两个必填文件
- [ ] `theme.json` 中所有必填字段（`name`、`id`、`version`、`author`、`description`）均已填�?
- [ ] `theme.json` 为合法的 JSON 格式（可�?[jsonlint.com](https://jsonlint.com) 检查）

### CSS 语法

- [ ] `style.css` 中包�?`@theme { ... }` 块，且至少定义了 7 个核心颜色令�?
- [ ] `@theme` 内的变量名正确（`--color-bg`、`--color-surface`、`--color-border`、`--color-text`、`--color-text-secondary`、`--color-primary`、`--color-primary-dark`�?
- [ ] CSS 语法正确，无未闭合的括号或引�?
- [ ] **未使�?`!important`**（除非处理极特殊情况�?
- [ ] 如有 `@import`，它位于文件最顶部、任何其他规则之�?

### 运行时验�?

- [ ] 重启 AgentStage 后，主题出现�?*设置 �?主题**列表�?
- [ ] 点击主题卡片后，UI **立即**更新（无闪烁、无需刷新�?
- [ ] 7 个核心颜色令牌均生效（背景、卡片、边框、文字、次要文字、主题色、主题色暗态）
- [ ] 如有组件覆盖，至少验�?`.left-nav` �?`.msg-bubble` 的效�?
- [ ] 关闭设置面板后，主题保持应用状�?
- [ ] 重启应用后，上次选择的主题自动恢�?

### 预览图（如提供）

- [ ] `preview.png` 尺寸建议�?**256×160**�?6:10�?
- [ ] 图片格式�?PNG �?JPG
- [ ] 主题卡片上预览图正常显示，无裂图

### 性能与兼容�?

- [ ] 网络字体有系统字�?fallback（防止离线时文字无法显示�?
- [ ] 未引用主题目录外的绝对路径资源（确保主题包可迁移�?
- [ ] CSS 选择器权重合理（优先使用单一类名，避免过度嵌套）

---

## 附录：完整主题示�?

以下是一个可直接复制使用的暗色主题完整示例�?

**目录结构�?*

```
data/themes/user/midnight/
├── theme.json
└── style.css
```

**`theme.json`�?*

```json
{
    "name": "午夜�?,
    "id": "midnight",
    "version": "1.0.0",
    "author": "AgentStage Community",
    "description": "深邃的午夜蓝色暗色主题，低对比度护眼",
    "tags": ["dark", "blue", "eye-care"]
}
```

**`style.css`�?*

```css
@theme {
    --color-bg:             #0b1120;
    --color-surface:        #151e32;
    --color-border:         #1e293b;
    --color-text:           #e2e8f0;
    --color-text-secondary: #64748b;
    --color-primary:        #60a5fa;
    --color-primary-dark:   #3b82f6;
}

html, body {
    font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
}

.left-nav {
    background: linear-gradient(180deg, #0f172a, #1e293b);
    border-right: 1px solid #1e293b;
}

.mid-panel {
    background-color: var(--color-surface);
    border-right: 1px solid var(--color-border);
}

.msg-bubble {
    border-radius: 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.btn-primary {
    border-radius: 8px;
    transition: all 0.15s ease;
}

.btn-primary:hover {
    filter: brightness(1.1);
}

.modal-overlay {
    background-color: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
}

::-webkit-scrollbar {
    width: 8px;
}
::-webkit-scrollbar-thumb {
    background: #334155;
    border-radius: 4px;
}
::-webkit-scrollbar-thumb:hover {
    background: #475569;
}
```

保存后重�?AgentStage，进入设置即可看到并切换该主题�?

---

*本文档对�?AgentStage 主题系统 v1.0。如有疑问，请参�?`docs/superpowers/specs/2026-05-24-theme-system-design.md` 设计文档�?
