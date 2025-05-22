//! Message format abstraction for different LLM APIs

use crate::session::{Session, Message};
use crate::error::Result;

/// Trait for converting between session format and LLM-specific message formats
pub trait MessageFormat<T> {
    /// Convert session messages to LLM-specific format
    fn from_session(&self, session: &Session) -> Result<Vec<T>>;
    
    /// Convert LLM-specific messages back to session format
    fn to_session(&self, messages: &[T], session_name: String) -> Result<Session>;
    
    /// Estimate token count for a single message
    fn estimate_tokens(&self, message: &T) -> usize;
    
    /// Get the maximum context window size for this format
    fn max_context_tokens(&self) -> usize;
}

/// AWS Bedrock message format
#[derive(Debug, Clone)]
pub struct BedrockFormat {
    pub max_tokens: usize,
}

impl Default for BedrockFormat {
    fn default() -> Self {
        Self {
            max_tokens: 8000, // Conservative default
        }
    }
}

impl BedrockFormat {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }
}

/// Simplified Bedrock message representation for the format trait
#[derive(Debug, Clone)]
pub struct BedrockMessage {
    pub role: String,
    pub content: String,
}

impl MessageFormat<BedrockMessage> for BedrockFormat {
    fn from_session(&self, session: &Session) -> Result<Vec<BedrockMessage>> {
        let mut bedrock_messages = Vec::new();
        
        for message in &session.messages {
            let role = match message.role {
                crate::session::MessageRole::System => "system",
                crate::session::MessageRole::User => "user", 
                crate::session::MessageRole::Assistant => "assistant",
                crate::session::MessageRole::Tool => "user", // Tool results as user messages
            };
            
            bedrock_messages.push(BedrockMessage {
                role: role.to_string(),
                content: message.content.clone(),
            });
        }
        
        Ok(bedrock_messages)
    }
    
    fn to_session(&self, messages: &[BedrockMessage], session_name: String) -> Result<Session> {
        let mut session = Session::with_name(session_name);
        
        for bedrock_msg in messages {
            let role = match bedrock_msg.role.as_str() {
                "system" => crate::session::MessageRole::System,
                "user" => crate::session::MessageRole::User,
                "assistant" => crate::session::MessageRole::Assistant,
                _ => crate::session::MessageRole::User, // Default fallback
            };
            
            session.add_message(Message::new(role, bedrock_msg.content.clone()));
        }
        
        Ok(session)
    }
    
    fn estimate_tokens(&self, message: &BedrockMessage) -> usize {
        // Simple estimation: ~4 characters per token
        (message.content.len() + 3) / 4
    }
    
    fn max_context_tokens(&self) -> usize {
        self.max_tokens
    }
}

/// OpenAI message format
#[derive(Debug, Clone)]
pub struct OpenAIFormat {
    pub max_tokens: usize,
}

impl Default for OpenAIFormat {
    fn default() -> Self {
        Self {
            max_tokens: 4000, // GPT-3.5 default
        }
    }
}

impl OpenAIFormat {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }
    
    pub fn gpt4() -> Self {
        Self { max_tokens: 8000 }
    }
    
    pub fn gpt4_turbo() -> Self {
        Self { max_tokens: 128000 }
    }
}

/// Simplified OpenAI message representation
#[derive(Debug, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

impl MessageFormat<OpenAIMessage> for OpenAIFormat {
    fn from_session(&self, session: &Session) -> Result<Vec<OpenAIMessage>> {
        let mut openai_messages = Vec::new();
        
        for message in &session.messages {
            let role = match message.role {
                crate::session::MessageRole::System => "system",
                crate::session::MessageRole::User => "user",
                crate::session::MessageRole::Assistant => "assistant", 
                crate::session::MessageRole::Tool => "function", // OpenAI has function role
            };
            
            openai_messages.push(OpenAIMessage {
                role: role.to_string(),
                content: message.content.clone(),
            });
        }
        
        Ok(openai_messages)
    }
    
    fn to_session(&self, messages: &[OpenAIMessage], session_name: String) -> Result<Session> {
        let mut session = Session::with_name(session_name);
        
        for openai_msg in messages {
            let role = match openai_msg.role.as_str() {
                "system" => crate::session::MessageRole::System,
                "user" => crate::session::MessageRole::User,
                "assistant" => crate::session::MessageRole::Assistant,
                "function" => crate::session::MessageRole::Tool,
                _ => crate::session::MessageRole::User, // Default fallback
            };
            
            session.add_message(Message::new(role, openai_msg.content.clone()));
        }
        
        Ok(session)
    }
    
    fn estimate_tokens(&self, message: &OpenAIMessage) -> usize {
        // OpenAI's tokenization is roughly 4 characters per token
        (message.content.len() + 3) / 4
    }
    
    fn max_context_tokens(&self) -> usize {
        self.max_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, Message, MessageRole};

    #[test]
    fn test_bedrock_format_conversion() {
        let mut session = Session::with_name("test".to_string());
        session.add_message(Message::new(MessageRole::System, "You are helpful".to_string()));
        session.add_message(Message::new(MessageRole::User, "Hello".to_string()));
        session.add_message(Message::new(MessageRole::Assistant, "Hi!".to_string()));

        let format = BedrockFormat::default();
        let bedrock_messages = format.from_session(&session).unwrap();
        
        assert_eq!(bedrock_messages.len(), 3);
        assert_eq!(bedrock_messages[0].role, "system");
        assert_eq!(bedrock_messages[1].role, "user");
        assert_eq!(bedrock_messages[2].role, "assistant");
        
        // Test round-trip conversion
        let converted_session = format.to_session(&bedrock_messages, "converted".to_string()).unwrap();
        assert_eq!(converted_session.messages.len(), 3);
    }

    #[test]
    fn test_openai_format_conversion() {
        let mut session = Session::with_name("test".to_string());
        session.add_message(Message::new(MessageRole::System, "You are helpful".to_string()));
        session.add_message(Message::new(MessageRole::User, "Hello".to_string()));
        session.add_message(Message::new(MessageRole::Tool, "result".to_string()));

        let format = OpenAIFormat::default();
        let openai_messages = format.from_session(&session).unwrap();
        
        assert_eq!(openai_messages.len(), 3);
        assert_eq!(openai_messages[0].role, "system");
        assert_eq!(openai_messages[1].role, "user");
        assert_eq!(openai_messages[2].role, "function");
    }
}