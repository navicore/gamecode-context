//! Session management and message handling

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::Result;
use crate::storage::SessionStorage;
use crate::compaction::CompactionStrategy;

/// Role of a message in the conversation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: Option<usize>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
            token_count: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new system message
    pub fn system(content: String) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// Create a new user message
    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create a new assistant message
    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// Create a new tool message
    pub fn tool(content: String) -> Self {
        Self::new(MessageRole::Tool, content)
    }

    /// Set token count for this message
    pub fn with_token_count(mut self, count: usize) -> Self {
        self.token_count = Some(count);
        self
    }

    /// Add metadata to this message
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Estimate token count if not already set
    pub fn estimate_tokens(&self) -> usize {
        if let Some(count) = self.token_count {
            count
        } else {
            // Simple estimation: ~4 characters per token
            (self.content.len() + 3) / 4
        }
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Session {
    /// Create a new session
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: format!("session-{}", now.format("%Y%m%d-%H%M%S")),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Create a new session with a custom name
    pub fn with_name(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: String) {
        self.add_message(Message::user(content));
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: String) {
        self.add_message(Message::assistant(content));
    }

    /// Add a system message
    pub fn add_system_message(&mut self, content: String) {
        self.add_message(Message::system(content));
    }

    /// Add a tool message
    pub fn add_tool_message(&mut self, content: String) {
        self.add_message(Message::tool(content));
    }

    /// Get total estimated token count
    pub fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.estimate_tokens()).sum()
    }

    /// Get messages since a certain timestamp
    pub fn messages_since(&self, since: DateTime<Utc>) -> Vec<&Message> {
        self.messages.iter()
            .filter(|m| m.timestamp > since)
            .collect()
    }

    /// Get the most recent N messages
    pub fn recent_messages(&self, count: usize) -> Vec<&Message> {
        if count >= self.messages.len() {
            self.messages.iter().collect()
        } else {
            self.messages.iter()
                .skip(self.messages.len() - count)
                .collect()
        }
    }

    /// Apply compaction strategy to reduce token count
    pub fn compact(&mut self, strategy: &CompactionStrategy, target_tokens: usize) -> Result<()> {
        if self.total_tokens() <= target_tokens {
            return Ok(());
        }

        match strategy {
            CompactionStrategy::Sliding { max_tokens } => {
                self.compact_sliding(*max_tokens)?;
            }
            CompactionStrategy::SystemAndRecent { system_tokens, recent_tokens } => {
                self.compact_system_and_recent(*system_tokens, *recent_tokens)?;
            }
            CompactionStrategy::Intelligent { target_tokens } => {
                self.compact_intelligent(*target_tokens)?;
            }
        }

        self.updated_at = Utc::now();
        Ok(())
    }

    fn compact_sliding(&mut self, max_tokens: usize) -> Result<()> {
        while self.total_tokens() > max_tokens && !self.messages.is_empty() {
            self.messages.remove(0);
        }
        Ok(())
    }

    fn compact_system_and_recent(&mut self, system_tokens: usize, recent_tokens: usize) -> Result<()> {
        // Keep system messages that fit in system_tokens budget
        let mut system_messages = Vec::new();
        let mut system_token_count = 0;

        for message in &self.messages {
            if message.role == MessageRole::System {
                let tokens = message.estimate_tokens();
                if system_token_count + tokens <= system_tokens {
                    system_messages.push(message.clone());
                    system_token_count += tokens;
                }
            }
        }

        // Keep recent messages that fit in recent_tokens budget
        let mut recent_messages = Vec::new();
        let mut recent_token_count = 0;

        for message in self.messages.iter().rev() {
            if message.role != MessageRole::System {
                let tokens = message.estimate_tokens();
                if recent_token_count + tokens <= recent_tokens {
                    recent_messages.insert(0, message.clone());
                    recent_token_count += tokens;
                } else {
                    break;
                }
            }
        }

        // Combine system and recent messages
        self.messages = system_messages;
        self.messages.extend(recent_messages);

        Ok(())
    }

    fn compact_intelligent(&mut self, target_tokens: usize) -> Result<()> {
        // For now, use system_and_recent strategy
        // TODO: Implement more sophisticated compaction
        let system_tokens = target_tokens / 4;
        let recent_tokens = (target_tokens * 3) / 4;
        self.compact_system_and_recent(system_tokens, recent_tokens)
    }
}

/// Session manager for loading, saving, and managing sessions
pub struct SessionManager {
    storage: Box<dyn SessionStorage>,
    compaction_strategy: CompactionStrategy,
    max_tokens: usize,
    auto_save: bool,
}

impl SessionManager {
    /// Create a new session manager with default storage
    pub fn new() -> Result<Self> {
        let storage = crate::storage::FileStorage::new()?;
        Ok(Self {
            storage: Box::new(storage),
            compaction_strategy: CompactionStrategy::default(),
            max_tokens: 8000,
            auto_save: true,
        })
    }

    /// Create a new session manager with custom configuration
    pub fn with_config(config: crate::Config) -> Result<Self> {
        let storage = match config.storage_dir {
            Some(dir) => crate::storage::FileStorage::with_directory(dir)?,
            None => crate::storage::FileStorage::new()?,
        };
        Ok(Self {
            storage: Box::new(storage),
            compaction_strategy: config.compaction_strategy,
            max_tokens: config.max_tokens,
            auto_save: config.auto_save,
        })
    }

    /// Load the most recent session
    pub fn load_latest(&mut self) -> Result<Session> {
        match self.storage.load_latest_session()? {
            Some(session) => Ok(session),
            None => {
                // Create a new session if none exists
                let session = Session::new();
                if self.auto_save {
                    self.storage.save_session(&session)?;
                }
                Ok(session)
            }
        }
    }

    /// Load a specific session by ID
    pub fn load_session(&mut self, session_id: &uuid::Uuid) -> Result<Session> {
        self.storage.load_session(session_id)
    }

    /// Save a session
    pub fn save_session(&mut self, session: &Session) -> Result<()> {
        self.storage.save_session(session)
    }

    /// Create a new session
    pub fn new_session(&mut self) -> Result<Session> {
        let session = Session::new();
        if self.auto_save {
            self.storage.save_session(&session)?;
        }
        Ok(session)
    }

    /// List all available sessions
    pub fn list_sessions(&self) -> Result<Vec<crate::storage::SessionInfo>> {
        self.storage.list_sessions()
    }

    /// Add a message to a session with automatic compaction and saving
    pub fn add_message(&mut self, session: &mut Session, message: Message) -> Result<()> {
        session.add_message(message);

        // Check if compaction is needed
        if session.total_tokens() > self.max_tokens {
            session.compact(&self.compaction_strategy, self.max_tokens)?;
        }

        // Auto-save if enabled
        if self.auto_save {
            self.storage.save_session(session)?;
        }

        Ok(())
    }
}