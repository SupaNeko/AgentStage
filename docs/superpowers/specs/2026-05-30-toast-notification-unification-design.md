# Toast 通知统一化重构设计文档

**日期**: 2026-05-30  
**范围**: 前端消息提示机制（Toast Notification）  
**状态**: 已批准，待实现

---

## 1. 问题陈述

当前前端消息提示机制存在以下问题：

1. **API 混乱**: `toastStore.show()` 签名复杂（`autoDismissOrDuration: boolean | number`），调用方式不统一。部分调用传数字作为 duration，部分传布尔值，部分不传。
2. **行为不一致**: `error` 类型的 toast 在不同地方有时传了 duration（如 5000ms、10000ms），导致报错信息也会自动消失，与产品预期不符。
3. **存在未接入的提示**: `SessionSettingsPanel.svelte` 仍使用原生 `alert()`，破坏用户体验一致性。
4. **语义不清**: 开发者无法从调用代码直观判断该 toast 是否会自动消失。

## 2. 设计目标

1. **统一所有提示**都通过 Toast 气泡机制展示，消灭 `alert()`。
2. **行为由类型决定**: 
   - `success` / `info`（绿色/默认色）→ 自动消失，默认 5 秒，调用点可覆盖时长。
   - `error`（红色）→ 永久保留，必须用户手动关闭，**API 层面禁止传 duration**。
3. **API 语义化**: 让调用代码自我描述行为。
4. **最小侵入**: 保持现有的 UI 位置、样式、动画、关闭按钮。

## 3. 设计方案

### 3.1 API 重构

将 `ToastStore` 从单一的 `show()` 方法重构为三个语义化方法：

```ts
// src/lib/stores/toastStore.svelte.ts

export class ToastStore {
    items = $state<ToastItem[]>([]);
    private nextId = 0;
    private timers = new Map<number, { interval: ReturnType<typeof setInterval>; timeout: ReturnType<typeof setTimeout> }>();

    /** 绿色成功提示，默认 5s 后自动消失，可覆盖 duration（单位 ms） */
    success(message: string, duration = 5000) {
        this.add({ message, type: 'success', autoDismiss: true, duration });
    }

    /** 蓝色/默认信息提示，默认 5s 后自动消失，可覆盖 duration（单位 ms） */
    info(message: string, duration = 5000) {
        this.add({ message, type: 'info', autoDismiss: true, duration });
    }

    /** 红色错误提示，永久保留，必须手动关闭。不接受 duration 参数。 */
    error(message: string) {
        this.add({ message, type: 'error', autoDismiss: false, duration: 0 });
    }

    private add(item: Omit<ToastItem, 'id' | 'progress'>) { /* ... */ }
    remove(id: number) { /* ... */ }
}

export const toastStore = new ToastStore();
```

**关键约束**: `error()` 方法**不暴露 duration 参数**。这是本方案的核心设计决策——从 API 签名层面保证 error toast 不会意外被设成自动消失。

### 3.2 UI 渲染调整

`App.svelte` 中 Toast 渲染逻辑微调：

- **Error toast**: 不显示底部进度条（因为不自动消失）。
- **Success/Info toast**: 保留现有进度条倒计时动画。
- **关闭按钮**: 所有 toast 保留手动关闭按钮（用户随时可以主动关闭任何 toast）。
- **样式/位置/动画**: 保持不变。

### 3.3 调用点迁移策略

所有现有调用按以下规则替换：

| 原调用 | 新调用 | 说明 |
|--------|--------|------|
| `toastStore.show(msg, 'success', 2000)` | `toastStore.success(msg, 2000)` | 保留原 duration |
| `toastStore.show(msg, 'success')` | `toastStore.success(msg)` | 使用默认 5s |
| `toastStore.show(msg, 'info', true, 10000)` | `toastStore.info(msg, 10000)` | 简化参数 |
| `toastStore.show(msg, 'error', 5000)` | `toastStore.error(msg)` | **去掉 duration，error 永久保留** |
| `toastStore.show(msg, 'error')` | `toastStore.error(msg)` | 直接替换 |
| `alert(msg)` | `toastStore.error(msg)` | 统一为 toast |

**批量替换后的审查原则**:
- 所有 `toastStore.show` 调用必须消失。
- 所有 `alert(` 调用必须消失。
- `toastStore.error(...)` 后面**不能**跟 duration 数字。

### 3.4 涉及的文件清单

**核心修改**:
- `src/lib/stores/toastStore.svelte.ts` — API 重构
- `src/App.svelte` — 渲染逻辑微调（error 不显示进度条）

**调用点替换**（需逐一检查）:
- `src/lib/components/ChatView.svelte`
- `src/lib/components/UserPersonaItem.svelte`
- `src/lib/components/SwitchPersonaConfirmModal.svelte`
- `src/lib/components/ModelConfigPanel.svelte`
- `src/lib/components/SettingsPanel.svelte`
- `src/lib/components/AgentDetail.svelte`
- `src/lib/components/CreateAgentModal.svelte`
- `src/lib/components/UserPersonaConfig.svelte`
- `src/lib/components/TimerEditModal.svelte`
- `src/lib/components/SessionSettingsPanel.svelte`（含 `alert` 替换）
- `src/lib/components/PersonaGenerateModal.svelte`
- `src/lib/components/HistorySessionList.svelte`
- `src/lib/components/CreateGroupModal.svelte`
- `src/lib/components/CreateUserPersonaModal.svelte`
- `src/lib/components/AgentTimerPanel.svelte`
- `src/lib/components/AvatarUploadModal.svelte`
- `src/lib/components/AgentMemoryPanel.svelte`
- `src/lib/components/AgentRelationshipPanel.svelte`
- `src/lib/components/AddRelationshipModal.svelte`

## 4. 边界情况

1. **后端 SSE 推送的 error 事件**: `App.svelte` 中 `agent_error` 事件触发的 toast，当前调用是 `toastStore.show(payload.error || ..., 'error')`，替换为 `toastStore.error(...)`。这是合理的——后端报错信息应该让用户看到并手动关闭。
2. **系统通知类 info**: `App.svelte` 中 `system_notification` 当前调用是 `toastStore.show(..., 'info', true, 10000)`，替换为 `toastStore.info(..., 10000)`。这是合理的——系统通知 10 秒后自动消失。
3. **连续多次报错**: 如果短时间内触发多个 error toast，它们会堆叠显示。这是预期行为，用户可逐一关闭。
4. **页面切换后 toast 是否保留**: 当前 toast 挂载在 `App.svelte` 根级别，不受路由/页面切换影响，行为不变。

## 5. 验收标准

- [ ] `toastStore.show(...)` 方法被完全移除，所有调用点使用语义化方法。
- [ ] 项目中不存在任何 `alert(` 调用。
- [ ] `toastStore.error(...)` 调用后面**没有任何 duration 参数**。
- [ ] `success` / `info` toast 默认 5 秒后自动消失，进度条动画正常。
- [ ] `error` toast 不显示进度条，不自动消失，必须手动关闭。
- [ ] `cargo check` / `npx svelte-check` 无错误。
- [ ] E2E / 集成测试通过（如有）。

## 6. 风险与回滚

**风险**: 批量替换调用点时可能误改行为（如某个原本 error 自动消失的调用，替换后变成永久保留）。
**缓解**: 逐文件审查替换结果，确保语义正确。
**回滚**: 纯前端重构，无 schema / migration / 后端变更，回滚只需 `git revert`。
