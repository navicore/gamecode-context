//! Context compaction strategies

use crate::session::{Session, Message};
use crate::error::Result;

/// Strategies for compacting conversation context when approaching token limits
#[derive(Debug, Clone)]
pub enum CompactionStrategy {
    /// Remove oldest messages beyond token limit
    Sliding { max_tokens: usize },
    
    /// Keep system messages + recent conversation
    SystemAndRecent { 
        system_tokens: usize, 
        recent_tokens: usize 
    },
    
    /// Smart compaction preserving important messages
    Intelligent { target_tokens: usize },
}

impl Default for CompactionStrategy {
    fn default() -> Self {
        Self::SystemAndRecent {
            system_tokens: 1000,
            recent_tokens: 6000,
        }
    }
}

/// Trait for implementing custom compaction strategies
pub trait ContextCompactor: Send + Sync {
    /// Compact a session to fit within the target token count
    fn compact(&self, session: &mut Session, target_tokens: usize) -> Result<()>;
    
    /// Estimate the priority of a message (higher = more important to keep)
    fn message_priority(&self, message: &Message, context: &Session) -> f64;
}

/// Smart compactor that preserves high-priority messages
pub struct IntelligentCompactor {
    /// Minimum number of recent messages to always keep
    pub min_recent_messages: usize,
    /// Weight for recency in priority calculation
    pub recency_weight: f64,
    /// Weight for role in priority calculation
    pub role_weight: f64,
    /// Weight for length/content in priority calculation
    pub content_weight: f64,
}

impl Default for IntelligentCompactor {
    fn default() -> Self {
        Self {
            min_recent_messages: 5,
            recency_weight: 1.0,
            role_weight: 0.5,
            content_weight: 0.3,
        }
    }
}

impl ContextCompactor for IntelligentCompactor {
    fn compact(&self, session: &mut Session, target_tokens: usize) -> Result<()> {
        if session.total_tokens() <= target_tokens {
            return Ok(());
        }

        // Always keep the most recent messages
        let keep_recent = std::cmp::min(self.min_recent_messages, session.messages.len());
        let mut kept_messages = Vec::new();
        
        // Calculate priorities for all messages except the most recent ones
        let mut message_priorities: Vec<(usize, f64)> = Vec::new();
        let messages_to_consider = session.messages.len().saturating_sub(keep_recent);
        
        for (i, message) in session.messages.iter().take(messages_to_consider).enumerate() {
            let priority = self.message_priority(message, session);
            message_priorities.push((i, priority));
        }
        
        // Sort by priority (highest first)
        message_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Add messages starting with highest priority until we hit token limit
        let mut token_count = 0;
        
        // First, add the recent messages (always kept)
        for message in session.messages.iter().skip(messages_to_consider) {
            token_count += message.estimate_tokens();
        }
        
        // Then add high-priority older messages
        let mut indices_to_keep = Vec::new();
        for (original_index, _priority) in message_priorities {
            let message = &session.messages[original_index];
            let message_tokens = message.estimate_tokens();
            
            if token_count + message_tokens <= target_tokens {
                token_count += message_tokens;
                indices_to_keep.push(original_index);
            }
        }
        
        // Sort indices to maintain chronological order
        indices_to_keep.sort();
        
        // Build the final message list
        for &index in &indices_to_keep {
            kept_messages.push(session.messages[index].clone());
        }
        
        // Add the recent messages at the end
        for message in session.messages.iter().skip(messages_to_consider) {
            kept_messages.push(message.clone());
        }
        
        session.messages = kept_messages;
        Ok(())
    }
    
    fn message_priority(&self, message: &Message, session: &Session) -> f64 {
        let mut priority = 0.0;
        
        // Recency: more recent messages have higher priority
        let total_messages = session.messages.len() as f64;
        let message_position = session.messages.iter()
            .position(|m| m.id == message.id)
            .unwrap_or(0) as f64;
        let recency_score = message_position / total_messages;
        priority += recency_score * self.recency_weight;
        
        // Role: system messages are important, tool results are valuable
        let role_score = match message.role {
            crate::session::MessageRole::System => 1.0,
            crate::session::MessageRole::Tool => 0.8,
            crate::session::MessageRole::Assistant => 0.6,
            crate::session::MessageRole::User => 0.4,
        };
        priority += role_score * self.role_weight;
        
        // Content: longer messages might be more important (but diminishing returns)
        let content_length = message.content.len() as f64;
        let content_score = (content_length / 1000.0).min(1.0); // Cap at 1.0
        priority += content_score * self.content_weight;
        
        priority
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, Message};

    #[test]
    fn test_intelligent_compactor() {
        let mut session = Session::with_name("test".to_string());
        
        // Add various messages
        session.add_message(Message::system("You are a helpful assistant".to_string()));
        session.add_message(Message::user("Hello".to_string()));
        session.add_message(Message::assistant("Hi there!".to_string()));
        session.add_message(Message::user("What's 2+2?".to_string()));
        session.add_message(Message::assistant("2+2 equals 4".to_string()));
        session.add_message(Message::tool("calculation_result: 4".to_string()));
        
        let compactor = IntelligentCompactor::default();
        
        // Compact to a very small token limit to force removal
        let target_tokens = 20; // Very small to test compaction
        compactor.compact(&mut session, target_tokens).unwrap();
        
        // Should keep some messages, prioritizing recent and important ones
        assert!(!session.messages.is_empty());
        assert!(session.total_tokens() <= target_tokens);
    }
}