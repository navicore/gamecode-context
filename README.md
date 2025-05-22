[![Dependabot Updates](https://github.com/navicore/gamecode-context/actions/workflows/dependabot/dependabot-updates/badge.svg)](https://github.com/navicore/gamecode-context/actions/workflows/dependabot/dependabot-updates)
[![Rust CI](https://github.com/navicore/gamecode-context/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/navicore/gamecode-context/actions/workflows/rust-ci.yml)

# gamecode-context

A Rust library for LLM context management and session persistence, designed for gamecode applications.

## Features

- **Session Management**: Create, load, save, and manage conversation sessions
- **Message Handling**: Support for different message roles (System, User, Assistant)
- **Context Compaction**: Intelligent strategies to reduce token count while preserving important information
- **Cross-Platform Storage**: File-based persistence with platform-specific config directories
- **Format Abstraction**: Support for different LLM API formats (Bedrock, OpenAI)
- **Token Estimation**: Built-in token counting for context management

## Quick Start

```rust
use gamecode_context::{SessionManager, session::{Session, Message, MessageRole}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a session manager
    let mut manager = SessionManager::new()?;

    // Load the latest session (or create new if none exists)
    let mut session = manager.load_latest()?;

    // Add messages to the session
    let message = Message::new(MessageRole::User, "Hello, can you help me with this code?".to_string());
    manager.add_message(&mut session, message)?;

    // The session will be automatically saved
    Ok(())
}
```

## Session Storage

Sessions are stored in platform-specific directories:
- **macOS**: `~/Library/Application Support/gamecode/sessions/`
- **Linux**: `~/.config/gamecode/sessions/`
- **Windows**: `%APPDATA%/gamecode/sessions/`

## Context Compaction

The library provides intelligent context compaction strategies:
- **LRU (Least Recently Used)**: Removes oldest messages
- **Priority-based**: Preserves important messages (system prompts, recent context)
- **Intelligent**: Combines multiple strategies for optimal results

## Configuration

Customize behavior with the `Config` struct:

```rust
use gamecode_context::{Config, CompactionStrategy};

let config = Config {
    max_tokens: 4000,
    compaction_strategy: CompactionStrategy::Intelligent,
    auto_save: true,
    storage_dir: None, // Use default
};

let manager = SessionManager::with_config(config)?;
```

## Format Support

Convert sessions to different LLM API formats:

```rust
use gamecode_context::format::{BedrockFormat, MessageFormat};

let bedrock_format = BedrockFormat::new();
let bedrock_messages = bedrock_format.from_session(&session)?;
```

## Error Handling

The library uses `anyhow::Result` for error handling and provides detailed error types through `ContextError`.

## License

This project is licensed under the MIT License.
