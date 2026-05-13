pub const CREATE_MIGRATIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    applied_at INTEGER NOT NULL
);
"#;

pub const MIGRATION_V1: &str = r#"
-- ========== 3.2 角色/Agent 表 ==========
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    avatar_path TEXT,

    detailed_persona TEXT NOT NULL,
    simplified_persona TEXT NOT NULL,
    personality TEXT,
    scenario TEXT,
    example_messages TEXT,
    first_message TEXT,
    creator_notes TEXT,
    tags TEXT,

    model_provider TEXT,
    model_name TEXT,
    base_url TEXT,
    temperature REAL DEFAULT 0.7,
    max_tokens INTEGER DEFAULT 2048,
    top_p REAL DEFAULT 1.0,
    presence_penalty REAL DEFAULT 0.0,
    frequency_penalty REAL DEFAULT 0.0,
    api_key_encrypted BLOB,

    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER,

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ========== 3.3 会话公共基表 ==========
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    session_type TEXT NOT NULL CHECK(session_type IN ('private', 'group')),

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    last_message_at INTEGER,
    last_message_preview TEXT,
    unread_count INTEGER DEFAULT 0,

    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER
);

-- ========== 3.4 私聊会话表 ==========
CREATE TABLE IF NOT EXISTS private_sessions (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id),

    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),

    agent_message_count INTEGER DEFAULT 0,
    last_reset_at INTEGER DEFAULT 0,

    created_at INTEGER NOT NULL
);

-- ========== 3.5 群聊会话表 ==========
CREATE TABLE IF NOT EXISTS group_sessions (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    avatar_path TEXT,

    mute_enabled INTEGER DEFAULT 1 CHECK(mute_enabled IN (0, 1)),
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),

    agent_message_count INTEGER DEFAULT 0,
    last_reset_at INTEGER DEFAULT 0,

    created_at INTEGER NOT NULL
);

-- ========== 3.6 群聊成员表 ==========
CREATE TABLE IF NOT EXISTS group_members (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_id TEXT NOT NULL,
    joined_at INTEGER NOT NULL,

    talkness REAL DEFAULT 0.5 CHECK(talkness >= 0 AND talkness <= 1),
    is_active INTEGER DEFAULT 1 CHECK(is_active IN (0, 1)),

    user_persona_id TEXT,

    PRIMARY KEY (session_id, participant_id, participant_type)
);

-- ========== 3.7 消息表 ==========
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sender_type TEXT NOT NULL CHECK(sender_type IN ('user', 'agent', 'system')),
    sender_id TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,

    message_type TEXT DEFAULT 'text' CHECK(message_type IN ('text', 'image', 'file', 'tool_call', 'system_notice')),
    tool_call_data TEXT,
    generation_info TEXT,

    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1))
);

-- ========== 3.8 好友关系表 ==========
CREATE TABLE IF NOT EXISTS friendships (
    id TEXT PRIMARY KEY,
    agent_id_1 TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    agent_id_2 TEXT REFERENCES agents(id) ON DELETE CASCADE,
    participant_type_2 TEXT DEFAULT 'agent' CHECK(participant_type_2 IN ('agent', 'user')),
    created_at INTEGER NOT NULL,
    source_session_id TEXT REFERENCES sessions(id)
);

-- ========== 3.9 Agent 触发状态表 ==========
CREATE TABLE IF NOT EXISTS trigger_states (
    agent_id TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    last_trigger_time INTEGER DEFAULT 0,
    updated_at INTEGER NOT NULL
);

-- ========== 3.10 应用设置表（单例） ==========
CREATE TABLE IF NOT EXISTS app_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),

    global_min_trigger_interval INTEGER DEFAULT 30,
    private_message_limit_default INTEGER DEFAULT 20,
    group_message_limit_default INTEGER DEFAULT 30,
    private_limit_enabled_default INTEGER DEFAULT 1,
    group_limit_enabled_default INTEGER DEFAULT 1,

    theme TEXT DEFAULT 'system' CHECK(theme IN ('system', 'light', 'dark')),
    font_size TEXT DEFAULT 'medium' CHECK(font_size IN ('small', 'medium', 'large')),
    language TEXT DEFAULT 'zh-CN',

    enter_to_send INTEGER DEFAULT 1 CHECK(enter_to_send IN (0, 1)),
    launch_on_startup INTEGER DEFAULT 0,
    minimize_to_tray INTEGER DEFAULT 1,

    updated_at INTEGER NOT NULL
);

-- ========== 3.11 用户人设表（P2 功能预留） ==========
CREATE TABLE IF NOT EXISTS user_personas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    avatar_path TEXT,
    is_default INTEGER DEFAULT 0 CHECK(is_default IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ========== 索引设计 ==========
CREATE INDEX IF NOT EXISTS idx_messages_session_time
    ON messages(session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_messages_session_sender_time
    ON messages(session_id, sender_type, sender_id, created_at);

CREATE INDEX IF NOT EXISTS idx_messages_system
    ON messages(sender_type, created_at) WHERE sender_type = 'system';

CREATE INDEX IF NOT EXISTS idx_sessions_last_message
    ON sessions(last_message_at DESC) WHERE is_deleted = 0;

CREATE INDEX IF NOT EXISTS idx_sessions_type
    ON sessions(session_type, last_message_at DESC) WHERE is_deleted = 0;

CREATE INDEX IF NOT EXISTS idx_sessions_deleted
    ON sessions(deleted_at DESC) WHERE is_deleted = 1;

CREATE INDEX IF NOT EXISTS idx_private_sessions_agent
    ON private_sessions(agent_id);

CREATE INDEX IF NOT EXISTS idx_group_members_session
    ON group_members(session_id);

CREATE INDEX IF NOT EXISTS idx_group_members_agent
    ON group_members(participant_id, participant_type);

CREATE INDEX IF NOT EXISTS idx_friendships_a1
    ON friendships(agent_id_1);

CREATE INDEX IF NOT EXISTS idx_friendships_a2
    ON friendships(agent_id_2);

CREATE INDEX IF NOT EXISTS idx_friendships_type
    ON friendships(participant_type_2);
"#;

pub const MIGRATION_V2: &str = r#"
-- V2: 修改 friendships 表以支持用户-角色关系
ALTER TABLE friendships RENAME TO friendships_old;

CREATE TABLE friendships (
    id TEXT PRIMARY KEY,
    agent_id_1 TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    agent_id_2 TEXT REFERENCES agents(id) ON DELETE CASCADE,
    participant_type_2 TEXT DEFAULT 'agent' CHECK(participant_type_2 IN ('agent', 'user')),
    created_at INTEGER NOT NULL,
    source_session_id TEXT REFERENCES sessions(id)
);

INSERT INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
    SELECT lower(hex(randomblob(16))), agent_id_1, agent_id_2, 'agent', created_at, source_session_id
    FROM friendships_old;

DROP TABLE friendships_old;

CREATE INDEX idx_friendships_a1 ON friendships(agent_id_1);
CREATE INDEX idx_friendships_a2 ON friendships(agent_id_2);
CREATE INDEX idx_friendships_type ON friendships(participant_type_2);
"#;

pub const MIGRATION_V3: &str = r#"
-- V3: 消息系统 V2 升级
-- 1. messages 表增加 extra 字段
ALTER TABLE messages ADD COLUMN extra TEXT DEFAULT '{}';

-- 2. trigger_states 表增加 is_triggering 字段
ALTER TABLE trigger_states ADD COLUMN is_triggering INTEGER DEFAULT 0;

-- 3. private_sessions 表增加 current_chat_page 字段
ALTER TABLE private_sessions ADD COLUMN current_chat_page INTEGER DEFAULT 0;

-- 4. 创建 agent_message_views 表
CREATE TABLE IF NOT EXISTS agent_message_views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    is_visible INTEGER DEFAULT 1 CHECK(is_visible IN (0, 1)),
    viewed_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_agent_views_agent_session ON agent_message_views(agent_id, session_id, created_at DESC);
CREATE INDEX idx_agent_views_message ON agent_message_views(message_id);

-- 5. 创建 chat_pages 表
CREATE TABLE IF NOT EXISTS chat_pages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    page_index INTEGER NOT NULL DEFAULT 0,
    name TEXT,
    is_active INTEGER DEFAULT 1 CHECK(is_active IN (0, 1)),
    message_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(session_id, page_index)
);

CREATE INDEX idx_chat_pages_session ON chat_pages(session_id);
"#;

pub const MIGRATION_V5: &str = r#"
-- V5: Add missing current_chat_page to group_sessions
ALTER TABLE group_sessions ADD COLUMN current_chat_page INTEGER DEFAULT 0;
"#;

pub const MIGRATION_V4: &str = r#"
-- V4: Session configuration panel
-- 1. Create unified session_settings table
CREATE TABLE IF NOT EXISTS session_settings (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    history_limit INTEGER,
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    mute_enabled INTEGER DEFAULT 0 CHECK(mute_enabled IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 2. Migrate existing config from private_sessions and group_sessions
INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
SELECT 
    ps.session_id,
    NULL as history_limit,
    ps.message_limit,
    ps.message_limit_enabled,
    0 as mute_enabled,
    ps.created_at,
    ps.created_at
FROM private_sessions ps
LEFT JOIN session_settings ss ON ps.session_id = ss.session_id
WHERE ss.session_id IS NULL;

INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
SELECT 
    gs.session_id,
    NULL as history_limit,
    gs.message_limit,
    gs.message_limit_enabled,
    gs.mute_enabled,
    gs.created_at,
    gs.created_at
FROM group_sessions gs
LEFT JOIN session_settings ss ON gs.session_id = ss.session_id
WHERE ss.session_id IS NULL;

-- 3. Add page_index to messages for chat page support
ALTER TABLE messages ADD COLUMN page_index INTEGER DEFAULT 0;

-- 4. Initialize default chat_pages for existing sessions
INSERT OR IGNORE INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
SELECT 
    lower(hex(randomblob(16))),
    s.id,
    0,
    '默认',
    1,
    0,
    s.created_at,
    s.created_at
FROM sessions s
LEFT JOIN chat_pages cp ON s.id = cp.session_id AND cp.page_index = 0
WHERE cp.id IS NULL;
"#;

pub const MIGRATION_V6: &str = r#"
-- V6: Session Inbox architecture
-- 1. Create session_frozen_states table
CREATE TABLE IF NOT EXISTS session_frozen_states (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    is_frozen INTEGER DEFAULT 0 CHECK(is_frozen IN (0, 1)),
    frozen_at INTEGER,
    updated_at INTEGER
);

-- 2. Create agent_unread_queue table
CREATE TABLE IF NOT EXISTS agent_unread_queue (
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, agent_id, message_id)
);

-- 3. Create indexes for agent_unread_queue
CREATE INDEX IF NOT EXISTS idx_agent_unread_session_agent ON agent_unread_queue(session_id, agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_unread_agent ON agent_unread_queue(agent_id);
"#;
