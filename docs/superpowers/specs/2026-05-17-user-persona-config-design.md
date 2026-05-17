# 用户角色配置页设计文档

| 项 | 内容 |
|---|---|
| **文档版本** | V1.0 |
| **编写日期** | 2026-05-17 |
| **功能编号** | USR-01 |
| **优先级** | P1 |

---

## 1. 背景与目标

当前 `user_personas` 表仅支持单条默认人设，前端无管理界面。本设计实现：
- 在应用左上角新增 `[个人]` 入口，进入个人配置视图
- 支持用户创建、管理多套人设（角色名 + 简易人设）
- 支持一键切换启用人设，或关闭使用人设（回退到代码常量默认人设）
- 每套人设有独立头像，切换人设时头像实时同步
- 支持更换"默认头像"（即关闭使用人设时展示的头像）

---

## 2. 页面架构

### 2.1 视图切换

新增 `appState.currentView: 'agents' | 'chat' | 'history' | 'profile'`，默认值不变（`'chat'`）。

`LeftNav.svelte` 调整：
- 在最上方新增 `[个人]` 按钮（`User` 图标 from lucide-svelte）
- 点击后 `appState.switchView('profile')`
- 原 `Bot`/`MessageSquare`/`History` 依次下移
- 底部保留 `[设置齿轮]`（暂时保留，后续可整合到配置分类中）

### 2.2 Profile 视图布局

当 `currentView === 'profile'` 时，`App.svelte` 渲染：

```
┌──────────┬──────────────────┬─────────────────────────────┐
│ LeftNav  │  配置分类列表     │      配置详情区域           │
│ (64px)   │  (~200px)        │     (剩余宽度)              │
│          │                  │                             │
│ [👤个人] │ ┌──────────────┐ │ ┌─────────────────────────┐ │
│ [🤖角色] │ │ ▶ 用户角色配置│ │ │  ┌────┐  默认头像       │ │
│ [💬聊天] │ │   通用设置    │ │ │  │ 👤 │  [关闭使用人设]│ │
│ [📜历史] │ └──────────────┘ │ │  └────┘                 │ │
│          │                  │ │  ───────────────────────  │ │
│ [⚙️]     │                  │ │  ┌─────────────────────┐  │ │
│          │                  │ │  │ [👤] 人设A      [启用]│  │ │
│          │                  │ │  └─────────────────────┘  │ │
│          │                  │ │  ┌─────────────────────┐  │ │
│          │                  │ │  │ [👤] 人设B    [启用中]│  │ │
│          │                  │ │  │ 角色名: [伊莉雅    ]  │  │ │
│          │                  │ │  │ 简易人设: [......   ] │  │ │
│          │                  │ │  │ [使用默认头像] [保存] │  │ │
│          │                  │ │  └─────────────────────┘  │ │
│          │                  │ │  ┌─────────────────────┐  │ │
│          │                  │ │  │ [👤] 人设C      [启用]│  │ │
│          │                  │ │  └─────────────────────┘  │ │
│          │                  │ │  ┌─────────────────────┐  │ │
│          │                  │ │  │        [ + ]         │  │ │
│          │                  │ │  │    创建新人设        │  │ │
│          │                  │ │  └─────────────────────┘  │ │
└──────────┴──────────────────┴─────────────────────────────┘
```

### 2.3 配置分类列表（左中栏）

当前只实现一个分类，UI 结构预留扩展：

| 分类 ID | 名称 | 状态 |
|---------|------|------|
| `user_persona` | 用户角色配置 | ✅ 本次实现 |
| `general` | 通用设置 | ⏳ 预留 |
| `appearance` | 外观设置 | ⏳ 预留 |

- 当前默认选中 `user_persona`
- 点击分类切换右侧内容区域
- 未实现的分类点击后右侧显示占位提示"暂未开放"

---

## 3. 用户角色配置详情页

### 3.1 默认头像区域（顶部紧凑行）

- **布局**：单行，高度紧凑（约 56px）
- **左侧**：圆形头像（约 40px）
  - 已设置：显示 `resolveAvatarUrl(default_avatar_path)`
  - 未设置：显示灰色默认图标（`User` icon，灰色）
  - **点击头像**：打开 `AvatarUploadModal`（`target_type = 'user_default'`）
- **中间**：文字"默认头像"
- **右侧**：`[关闭使用人设]` 文字按钮
  - 点击后 `active_persona_id` 设为 `None`
  - 所有人设行的"启用中"变回"启用"

### 3.2 人设列表（手风琴 Accordion）

- 纵向列表，每项一行
- **折叠状态**：左侧头像（约 32px，可点击）+ 角色名 + 右侧"启用"/"启用中"按钮
- **点击整行（非按钮区域）**：展开/折叠该项
- **点击头像**：直接打开 `AvatarUploadModal`（`target_type = 'user_persona'`，`target_id = 该人设id`）
  - 上传成功后该人设的 `avatar_path` 立即更新，列表头像实时刷新

#### 展开后内容

```
┌─────────────────────────────────┐
│ [👤] 人设B              [启用中] │  ← 头部行（点击折叠）
│                                 │
│ 角色名: [伊莉雅            ]    │
│ 简易人设: [正在与你聊天的   ]    │
│           [真实用户        ]    │
│                                 │
│ [使用默认头像]  [保存] [取消]   │
└─────────────────────────────────┘
```

- **角色名**：单行输入框，必填
- **简易人设**：多行文本域（3-4 行），选填
- **使用默认头像**按钮：点击后将当前 `default_avatar_path` 复制到该人设的 `avatar_path`
- **保存**：调用 `update_user_persona`，成功后保持展开并刷新数据
- **取消**：恢复原始值并折叠该项

#### "启用"按钮交互

- **未启用**：主色背景/文字按钮，显示"启用"
- **已启用**：视觉上"按下去"的样式（如 darker background, inset shadow），显示"启用中"
- 点击"启用"：
  1. 调用 `activate_user_persona(id)`
  2. 上一个"启用中"按钮恢复为"启用"
  3. 当前按钮变为"启用中"
- 同时只能有**一个**人设处于"启用中"状态

### 3.3 创建新人设

- 列表底部有特殊的"创建新人设"卡片（+ 图标）
- **点击后弹出二级窗口**（类似 `CreateAgentModal`）
- 弹窗内容：
  - 标题："创建新人设"
  - 头像区域：默认显示灰色默认图标，点击更换
  - "使用默认头像"按钮
  - 角色名（必填）
  - 简易人设（选填）
  - [创建] [取消]
- 创建成功后弹窗关闭，列表新增一行（折叠状态）

---

## 4. 数据模型

### 4.1 数据库变更

**`user_personas` 表**（Migration V11 移除 `is_default`）：

```sql
-- 移除 is_default 列（不再使用）
ALTER TABLE user_personas DROP COLUMN is_default;

-- 删除旧的默认人设记录（默认人设改为代码常量）
DELETE FROM user_personas WHERE is_default = 1;
```

> **注意**：如果 `is_default` 列因 SQLite 限制无法 `DROP COLUMN`，则在代码层面忽略该列，不再读写。

**`app_settings` 表**（新增字段）：

```sql
ALTER TABLE app_settings ADD COLUMN active_persona_id TEXT;
ALTER TABLE app_settings ADD COLUMN default_avatar_path TEXT;
```

**数据迁移**：
```sql
-- 将旧的默认人设头像迁移为新的 default_avatar_path
UPDATE app_settings SET default_avatar_path = (
    SELECT avatar_path FROM user_personas WHERE is_default = 1 LIMIT 1
);
```

### 4.2 Rust Model

新建 `src-tauri/src/models/user_persona.rs`：

```rust
#[derive(Debug, serde::Serialize)]
pub struct UserPersona {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateUserPersonaRequest {
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>, // 可选，不传则使用默认头像或空
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUserPersonaRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct CurrentUserPersonaResponse {
    pub id: Option<String>, // None 表示使用默认人设
    pub name: String,
    pub description: String,
    pub avatar_path: Option<String>,
    pub is_custom: bool, // false 表示当前使用的是代码默认人设
}
```

### 4.3 代码常量

新建 `src-tauri/src/constants.rs`：

```rust
pub const DEFAULT_USER_NAME: &str = "用户";
pub const DEFAULT_USER_PERSONA: &str = "正在与你聊天的真实用户";
```

> 该常量同时替代 `prompt_templates.rs` 中的 `USER_NAME_DEFAULT` 和 `USER_PERSONA_DEFAULT`，避免分散维护。

---

## 5. 后端 API 设计

新建 `src-tauri/src/commands/user_persona.rs`：

| 命令 | 签名 | 说明 |
|------|------|------|
| `list_user_personas` | `() -> Vec<UserPersona>` | 列出所有用户创建的人设 |
| `create_user_persona` | `(CreateUserPersonaRequest) -> UserPersona` | 创建新人设 |
| `update_user_persona` | `(UpdateUserPersonaRequest) -> UserPersona` | 更新人设 |
| `delete_user_persona` | `(id: String) -> ()` | 删除人设 |
| `get_current_user_persona` | `() -> CurrentUserPersonaResponse` | 获取当前生效的人设（含默认回退逻辑） |
| `activate_user_persona` | `(id: Option<String>) -> ()` | 激活指定人设，`None` = 关闭使用人设 |

**`get_current_user_persona` 逻辑**：
1. 查询 `app_settings.active_persona_id`
2. 如果存在且对应 `user_personas` 记录存在 → 返回该记录
3. 否则 → 返回默认人设：
   - `id: None`, `name: DEFAULT_USER_NAME`, `description: DEFAULT_USER_PERSONA`
   - `avatar_path: app_settings.default_avatar_path`
   - `is_custom: false`

**`upload_avatar` 命令扩展**：
- `target_type = 'user_default'`：更新 `app_settings.default_avatar_path`
- `target_type = 'user_persona'` + `target_id`：更新 `user_personas.avatar_path`

---

## 6. 前端设计

### 6.1 Store

新建 `src/lib/stores/userPersonaStore.svelte.ts`：

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface UserPersona {
    id: string;
    name: string;
    description?: string;
    avatar_path?: string;
}

class UserPersonaStore {
    personas = $state<UserPersona[]>([]);
    loading = $state(false);

    async loadPersonas() {
        this.loading = true;
        try {
            this.personas = await invoke<UserPersona[]>('list_user_personas');
        } finally {
            this.loading = false;
        }
    }

    async createPersona(data: { name: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('create_user_persona', { req: data });
        this.personas = [...this.personas, persona];
        return persona;
    }

    async updatePersona(data: { id: string; name?: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('update_user_persona', { req: data });
        this.personas = this.personas.map(p => p.id === persona.id ? persona : p);
        return persona;
    }

    async deletePersona(id: string) {
        await invoke('delete_user_persona', { id });
        this.personas = this.personas.filter(p => p.id !== id);
    }

    async activatePersona(id: string | null) {
        await invoke('activate_user_persona', { id });
        // 触发 settingsStore 刷新以更新 active_persona_id
        await settingsStore.load();
    }
}

export const userPersonaStore = new UserPersonaStore();
```

### 6.2 组件列表

| 组件 | 路径 | 说明 |
|------|------|------|
| `ProfileView.svelte` | `src/lib/components/ProfileView.svelte` | Profile 视图根组件，包含分类列表和详情区域 |
| `UserPersonaConfig.svelte` | `src/lib/components/UserPersonaConfig.svelte` | "用户角色配置"详情内容 |
| `UserPersonaItem.svelte` | `src/lib/components/UserPersonaItem.svelte` | 单个人设行（折叠/展开状态） |
| `CreateUserPersonaModal.svelte` | `src/lib/components/CreateUserPersonaModal.svelte` | 创建新人设弹窗 |

### 6.3 头像实时同步

- 头像上传成功后，后端返回新路径
- 前端直接更新 `userPersonaStore.personas` 中对应项的 `avatar_path`
- 所有使用 `resolveAvatarUrl()` 绑定头像的地方自动响应式更新
- **无需点击保存**：点击头像 → 弹窗上传 → 确认 → 直接 `update_user_persona({ id, avatar_path: newPath })` → 列表实时刷新

---

## 7. 对现有系统的影响

### 7.1 Prompt 组装逻辑

`src-tauri/src/llm/prompt.rs` 中 `get_user_persona()` 需要更新：

```rust
fn get_user_persona(conn: &Connection) -> (String, String) {
    // 1. 读取 settings.active_persona_id
    // 2. 如果有，查询 user_personas 对应记录
    // 3. 如果没有或查询失败，使用 DEFAULT_USER_NAME / DEFAULT_USER_PERSONA
}
```

### 7.2 现有 SettingsPanel

- `SettingsPanel.svelte` 中的"用户头像"区域移除（迁移到 Profile 视图的默认头像区域）
- 或暂时保留作为快捷入口，指向 Profile 视图

### 7.3 `upload_avatar` 命令

- 新增 `target_type = 'user_default'` 分支
- 现有 `target_type = 'user'` 可废弃或重定向

---

## 8. 迁移与兼容性

### Migration V11

```sql
-- app_settings 新增字段
ALTER TABLE app_settings ADD COLUMN active_persona_id TEXT;
ALTER TABLE app_settings ADD COLUMN default_avatar_path TEXT;

-- 将旧默认人设的头像路径迁移到 default_avatar_path
UPDATE app_settings SET default_avatar_path = (
    SELECT avatar_path FROM user_personas WHERE is_default = 1 LIMIT 1
);

-- 删除旧默认人设记录
DELETE FROM user_personas WHERE is_default = 1;
```

### 头像文件

- 旧默认人设的头像文件（如 `data/avatars/user/xxx.png`）保留原地
- `default_avatar_path` 指向该路径，确保升级后用户头像不丢失
- 后续用户上传新默认头像时，保存到新路径并更新 `default_avatar_path`

---

## 9. 测试要点

- [ ] 创建多个人设，验证列表显示
- [ ] 点击头像更换，验证实时生效
- [ ] 点击"启用"切换，验证互斥（只有一个启用中）
- [ ] 点击"关闭使用人设"，验证所有人设恢复"启用"
- [ ] 展开人设编辑，修改名称/人设，保存/取消
- [ ] 创建新人设弹窗，验证必填校验
- [ ] 删除人设，验证列表移除
- [ ] 验证 Prompt 组装中是否正确读取当前人设
- [ ] 验证默认头像在消息气泡中是否正确显示

---

*设计确认：已根据用户反馈调整（头像点击直接更换、创建弹窗、关闭使用人设为文字按钮、默认人设改为代码常量）。*
