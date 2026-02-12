-- Sessions table
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL, -- UUID or session key like "telegram:12345"
    title TEXT,
    summary TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Messages table
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL, -- UUID
    session_id TEXT NOT NULL,     -- FK to sessions.id
    sender_id TEXT NOT NULL,      -- "user", "assistant", "system", "tool:<name>"
    role TEXT NOT NULL,           -- "user", "assistant", "system", "tool"
    content TEXT NOT NULL,
    metadata JSON NOT NULL DEFAULT '{}',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reply_to TEXT,                -- Optional UUID of parent message
    
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- Index for session history lookup
CREATE INDEX idx_messages_session_created ON messages(session_id, created_at);
