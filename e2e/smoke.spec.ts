import { test, expect } from '@playwright/test';

// Mock Tauri IPC with test data
const MOCK_AGENTS = [
  {
    id: 'agent-1',
    name: '卫宫士郎',
    avatar_path: null,
    detailed_persona: '你是 Fate/stay night 中的卫宫士郎...',
    simplified_persona: '出自 Fate/stay night 的卫宫士郎，冬木市的见习魔术师。',
    personality: null,
    scenario: null,
    example_messages: null,
    first_message: null,
    creator_notes: null,
    tags: null,
    model_provider: 'openai',
    model_name: 'gpt-4o',
    base_url: null,
    temperature: 0.7,
    max_tokens: 2048,
    top_p: 1.0,
    presence_penalty: 0.0,
    frequency_penalty: 0.0,
    is_deleted: false,
    deleted_at: null,
    created_at: Date.now(),
    updated_at: Date.now(),
  },
  {
    id: 'agent-2',
    name: 'Saber',
    avatar_path: null,
    detailed_persona: '你是阿尔托莉雅·潘德拉贡...',
    simplified_persona: '出自 Fate/stay night 的 Saber，亚瑟王。',
    personality: null,
    scenario: null,
    example_messages: null,
    first_message: null,
    creator_notes: null,
    tags: null,
    model_provider: 'openai',
    model_name: 'gpt-4o',
    base_url: null,
    temperature: 0.7,
    max_tokens: 2048,
    top_p: 1.0,
    presence_penalty: 0.0,
    frequency_penalty: 0.0,
    is_deleted: false,
    deleted_at: null,
    created_at: Date.now(),
    updated_at: Date.now(),
  },
];

const MOCK_SESSIONS: any[] = [];
const MOCK_MESSAGES: Record<string, any[]> = {};

test.beforeEach(async ({ page }) => {
  // Inject mock Tauri internals before any page scripts run
  await page.addInitScript(({ agents, sessions, messages }: any) => {
    const mockAgents = JSON.parse(JSON.stringify(agents));
    const mockSessions = JSON.parse(JSON.stringify(sessions));
    const mockMessages = JSON.parse(JSON.stringify(messages));
    const eventListeners = new Map();
    const callbacks = new Map();

    function registerCallback(callback: any, once = false) {
      const identifier = window.crypto.getRandomValues(new Uint32Array(1))[0];
      callbacks.set(identifier, (data: any) => {
        if (once) {
          callbacks.delete(identifier);
        }
        return callback && callback(data);
      });
      return identifier;
    }

    function unregisterCallback(id: number) {
      callbacks.delete(id);
    }

    function runCallback(id: number, data: any) {
      const callback = callbacks.get(id);
      if (callback) {
        callback(data);
      }
    }

    function handleListen(args: any) {
      if (!eventListeners.has(args.event)) {
        eventListeners.set(args.event, []);
      }
      eventListeners.get(args.event).push(args.handler);
      return args.handler;
    }

    function handleEmit(args: any) {
      const listeners = eventListeners.get(args.event) || [];
      for (const handler of listeners) {
        runCallback(handler, args.payload);
      }
      return null;
    }

    function handleRemoveListener(args: any) {
      const listeners = eventListeners.get(args.event);
      if (listeners) {
        const index = listeners.indexOf(args.id);
        if (index !== -1) listeners.splice(index, 1);
      }
    }

    async function invoke(cmd: string, args?: any, _options?: any) {
      // Handle event plugin commands
      if (cmd === 'plugin:event|listen') return handleListen(args);
      if (cmd === 'plugin:event|emit') return handleEmit(args);
      if (cmd === 'plugin:event|unlisten') return handleRemoveListener(args);

      // Handle app commands
      switch (cmd) {
        case 'list_agents':
          return mockAgents.filter((a: any) => !a.is_deleted);

        case 'get_agent':
          return mockAgents.find((a: any) => a.id === args?.id) || null;

        case 'create_agent': {
          const newAgent = {
            id: 'agent-' + Date.now(),
            ...args?.req,
            is_deleted: false,
            deleted_at: null,
            created_at: Date.now(),
            updated_at: Date.now(),
          };
          mockAgents.push(newAgent);
          return newAgent;
        }

        case 'update_agent': {
          const idx = mockAgents.findIndex((a: any) => a.id === args?.req?.id);
          if (idx >= 0) {
            mockAgents[idx] = { ...mockAgents[idx], ...args.req, updated_at: Date.now() };
            return mockAgents[idx];
          }
          throw new Error('Agent not found');
        }

        case 'delete_agent': {
          const dIdx = mockAgents.findIndex((a: any) => a.id === args?.id);
          if (dIdx >= 0) {
            mockAgents[dIdx].is_deleted = true;
            mockAgents[dIdx].deleted_at = Date.now();
          }
          return true;
        }

        case 'create_private_session': {
          const sessionId = 'session-' + Date.now();
          const agent = mockAgents.find((a: any) => a.id === args?.req?.agent_id);
          const newSession = {
            id: sessionId,
            session_type: 'private',
            last_message_at: null,
            last_message_preview: null,
            unread_count: 0,
            agent_id: args?.req?.agent_id,
            agent_name: agent?.name || '未知角色',
            agent_avatar: agent?.avatar_path || null,
            group_name: null,
            group_avatar: null,
            mute_enabled: null,
          };
          mockSessions.push(newSession);
          return newSession;
        }

        case 'list_sessions':
          return mockSessions;

        case 'get_session':
          return mockSessions.find((s: any) => s.id === args?.id) || null;

        case 'delete_session': {
          const sIdx = mockSessions.findIndex((s: any) => s.id === args?.id);
          if (sIdx >= 0) mockSessions.splice(sIdx, 1);
          return true;
        }

        case 'get_session_messages':
          return mockMessages[args?.sessionId] || [];

        case 'send_user_message': {
          const msgId = 'msg-' + Date.now();
          const msg = {
            id: msgId,
            session_id: args?.sessionId,
            sender_type: 'user',
            sender_id: 'user',
            content: args?.content,
            created_at: Date.now(),
            message_type: 'text',
          };
          if (!mockMessages[args?.sessionId]) mockMessages[args?.sessionId] = [];
          mockMessages[args?.sessionId].push(msg);
          return msg;
        }

        default:
          console.warn('[Mock Invoke] Unhandled command:', cmd, args);
          throw new Error(`Unhandled command: ${cmd}`);
      }
    }

    (window as any).__TAURI_INTERNALS__ = {
      invoke,
      transformCallback: registerCallback,
      unregisterCallback,
      runCallback,
      callbacks,
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { windowLabel: 'main', label: 'main' }
      }
    };

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        unregisterCallback(id);
      }
    };
  }, { agents: MOCK_AGENTS, sessions: MOCK_SESSIONS, messages: MOCK_MESSAGES });
});

test.describe('AgentStage Smoke Tests', () => {
  test('page loads and shows agent list view by default', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Should show the left navigation
    await expect(page.locator('nav')).toBeVisible();

    // Should show "角色列表" in the middle panel header
    await expect(page.getByText('角色列表')).toBeVisible();

    // Should show agent cards (mock data has 2 agents)
    await expect(page.getByText('卫宫士郎')).toBeVisible();
    await expect(page.getByText('Saber')).toBeVisible();
  });

  test('can switch between views using left navigation', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Default view is agents
    await expect(page.getByText('角色列表')).toBeVisible();

    // Click chat icon in left nav (second button)
    const navButtons = page.locator('nav button');
    await expect(navButtons).toHaveCount(3); // agents, chat, history

    // Click chat (second nav button)
    await navButtons.nth(1).click();
    await page.waitForTimeout(300);

    // Should show session list header or empty state
    const hasSessionList = await page.getByText('会话列表').isVisible().catch(() => false);
    const hasEmptyState = await page.getByText('还没有会话').isVisible().catch(() => false);
    expect(hasSessionList || hasEmptyState).toBe(true);

    // Click back to agents (first nav button)
    await navButtons.nth(0).click();
    await page.waitForTimeout(300);
    await expect(page.getByText('角色列表')).toBeVisible();
  });

  test('can view agent detail and start chat', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Click on an agent card
    await page.getByText('卫宫士郎').click();
    await page.waitForTimeout(300);

    // Should show agent detail with name
    await expect(page.getByRole('heading', { name: '卫宫士郎' })).toBeVisible();

    // Should show "开始聊天" button
    const startChatButton = page.getByRole('button', { name: '开始聊天' });
    await expect(startChatButton).toBeVisible();

    // Click start chat
    await startChatButton.click();
    await page.waitForTimeout(500);

    // Should switch to chat view and show the session header
    await expect(page.locator('header').getByText('卫宫士郎')).toBeVisible();
  });

  test('can create new agent', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Find and click "新建" button
    const newButton = page.getByRole('button', { name: /新建|新建角色/ }).first();
    await expect(newButton).toBeVisible();
    await newButton.click();
    await page.waitForTimeout(300);

    // Fill the form
    await page.getByLabel(/角色名称/).fill('测试角色');
    await page.getByLabel(/详细人设/).fill('这是一个测试角色的详细人设。');
    await page.getByLabel(/简易人设/).fill('测试角色简介。');
    await page.getByLabel(/模型名称/).fill('gpt-4o');
    await page.getByLabel(/API Key/).fill('sk-test-key');

    // Submit
    const submitButton = page.getByRole('button', { name: '创建' });
    await submitButton.click();
    await page.waitForTimeout(500);

    // Should show the new agent in the list
    await expect(page.getByText('测试角色')).toBeVisible();
  });

  test('chat view shows empty state when no session selected', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Switch to chat view
    const navButtons = page.locator('nav button');
    await navButtons.nth(1).click();
    await page.waitForTimeout(300);

    // Should show empty state in main content area
    await expect(page.getByText('选择一个会话开始聊天').first()).toBeVisible();
  });
});
