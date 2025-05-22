//! # gamecode-context
//!
//! LLM context management and session persistence for gamecode applications.
//!
//! This crate provides:
//! - Session management with automatic persistence
//! - Context compaction strategies to manage token limits
//! - Format abstraction for different LLM APIs
//! - Cross-platform session file handling
//!
//! ## Quick Start
//!
//! ```rust
//! use gamecode_context::{SessionManager, session::{Session, Message, MessageRole}};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a session manager
//! let mut manager = SessionManager::new()?;
//!
//! // Load the latest session (or create new if none exists)
//! let mut session = manager.load_latest()?;
//!
//! // Add a message to the session
//! let message = Message::new(MessageRole::User, "Hello, can you help me with this code?".to_string());
//! manager.add_message(&mut session, message)?;
//!
//! // The session will be automatically saved
//! # Ok(())
//! # }
//! ```

pub mod session;
pub mod compaction;
pub mod format;
pub mod storage;
pub mod error;

pub use session::{Session, SessionManager, Message, MessageRole};
pub use compaction::{CompactionStrategy, ContextCompactor};
pub use format::MessageFormat;
pub use storage::SessionStorage;
pub use error::{ContextError, Result};

/// Default configuration for session management
pub struct Config {
    /// Maximum number of tokens before compaction is triggered
    pub max_tokens: usize,
    /// Default compaction strategy
    pub compaction_strategy: CompactionStrategy,
    /// Base directory for session storage
    pub storage_dir: Option<std::path::PathBuf>,
    /// Whether to auto-save sessions after each message
    pub auto_save: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_tokens: 8000, // Conservative default for most models
            compaction_strategy: CompactionStrategy::SystemAndRecent {
                system_tokens: 1000,
                recent_tokens: 6000,
            },
            storage_dir: None, // Will use default user config dir
            auto_save: true,
        }
    }
}