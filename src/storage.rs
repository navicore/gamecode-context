use crate::error::ContextError;
use crate::session::Session;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Trait for session storage backends
pub trait SessionStorage: Send + Sync {
    /// Save a session to storage
    fn save_session(&self, session: &Session) -> Result<(), ContextError>;
    
    /// Load a session by ID
    fn load_session(&self, session_id: &Uuid) -> Result<Session, ContextError>;
    
    /// Load the most recent session
    fn load_latest_session(&self) -> Result<Option<Session>, ContextError>;
    
    /// List all available sessions
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, ContextError>;
    
    /// Delete a session
    fn delete_session(&self, session_id: &Uuid) -> Result<(), ContextError>;
    
    /// Clean up old sessions (keep last N sessions)
    fn cleanup_old_sessions(&self, keep_count: usize) -> Result<usize, ContextError>;
}

/// Information about a stored session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub created_at: SystemTime,
    pub modified_at: SystemTime,
    pub message_count: usize,
    pub file_path: PathBuf,
}

/// File-based session storage implementation
pub struct FileStorage {
    sessions_dir: PathBuf,
    latest_symlink: PathBuf,
}

impl FileStorage {
    /// Create a new file storage instance
    pub fn new() -> Result<Self, ContextError> {
        let sessions_dir = Self::default_sessions_dir()?;
        let latest_symlink = sessions_dir.join("latest.json");
        
        // Create sessions directory if it doesn't exist
        if !sessions_dir.exists() {
            fs::create_dir_all(&sessions_dir)
                .map_err(|e| ContextError::Storage(format!("Failed to create sessions directory: {}", e)))?;
            info!("Created sessions directory: {}", sessions_dir.display());
        }
        
        Ok(Self {
            sessions_dir,
            latest_symlink,
        })
    }
    
    /// Create a file storage instance with custom directory
    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Result<Self, ContextError> {
        let sessions_dir = dir.as_ref().to_path_buf();
        let latest_symlink = sessions_dir.join("latest.json");
        
        if !sessions_dir.exists() {
            fs::create_dir_all(&sessions_dir)
                .map_err(|e| ContextError::Storage(format!("Failed to create sessions directory: {}", e)))?;
        }
        
        Ok(Self {
            sessions_dir,
            latest_symlink,
        })
    }
    
    /// Get the default sessions directory
    fn default_sessions_dir() -> Result<PathBuf, ContextError> {
        let home_dir = home::home_dir()
            .ok_or_else(|| ContextError::Storage("Could not determine home directory".to_string()))?;
        
        #[cfg(target_os = "macos")]
        let config_dir = home_dir.join("Library").join("Application Support");
        
        #[cfg(target_os = "linux")]
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir.join(".config"));
        
        #[cfg(target_os = "windows")]
        let config_dir = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir.join("AppData").join("Roaming"));
        
        Ok(config_dir.join("gamecode").join("sessions"))
    }
    
    /// Get the file path for a session
    fn session_file_path(&self, session_id: &Uuid) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", session_id))
    }
    
    /// Update the latest session symlink
    fn update_latest_symlink(&self, session_id: &Uuid) -> Result<(), ContextError> {
        let target_file = format!("{}.json", session_id);
        
        // Remove existing symlink if it exists
        if self.latest_symlink.exists() {
            fs::remove_file(&self.latest_symlink)
                .map_err(|e| ContextError::Storage(format!("Failed to remove old symlink: {}", e)))?;
        }
        
        // Create new symlink (or copy on Windows)
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target_file, &self.latest_symlink)
                .map_err(|e| ContextError::Storage(format!("Failed to create symlink: {}", e)))?;
        }
        
        #[cfg(windows)]
        {
            // Windows doesn't always support symlinks, so we'll copy the file
            let source = self.sessions_dir.join(&target_file);
            fs::copy(&source, &self.latest_symlink)
                .map_err(|e| ContextError::Storage(format!("Failed to copy to latest: {}", e)))?;
        }
        
        debug!("Updated latest session symlink to {}", session_id);
        Ok(())
    }
    
    /// Get session info from a file
    fn get_session_info(&self, file_path: &Path) -> Result<SessionInfo, ContextError> {
        let file_name = file_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ContextError::Storage("Invalid session file name".to_string()))?;
        
        let session_id = Uuid::parse_str(file_name)
            .map_err(|_| ContextError::Storage(format!("Invalid session ID in filename: {}", file_name)))?;
        
        let metadata = fs::metadata(file_path)
            .map_err(|e| ContextError::Storage(format!("Failed to read file metadata: {}", e)))?;
        
        let created_at = metadata.created().unwrap_or_else(|_| SystemTime::now());
        let modified_at = metadata.modified().unwrap_or_else(|_| SystemTime::now());
        
        // Read session to get message count
        let session_data = fs::read_to_string(file_path)
            .map_err(|e| ContextError::Storage(format!("Failed to read session file: {}", e)))?;
        
        let session: Session = serde_json::from_str(&session_data)?;
        
        Ok(SessionInfo {
            id: session_id,
            created_at,
            modified_at,
            message_count: session.messages.len(),
            file_path: file_path.to_path_buf(),
        })
    }
}

impl SessionStorage for FileStorage {
    fn save_session(&self, session: &Session) -> Result<(), ContextError> {
        let file_path = self.session_file_path(&session.id);
        
        let session_json = serde_json::to_string_pretty(session)?;
        
        fs::write(&file_path, session_json)
            .map_err(|e| ContextError::Storage(format!("Failed to write session file: {}", e)))?;
        
        // Update the latest symlink
        self.update_latest_symlink(&session.id)?;
        
        debug!("Saved session {} to {}", session.id, file_path.display());
        Ok(())
    }
    
    fn load_session(&self, session_id: &Uuid) -> Result<Session, ContextError> {
        let file_path = self.session_file_path(session_id);
        
        if !file_path.exists() {
            return Err(ContextError::SessionNotFound(session_id.to_string()));
        }
        
        let session_data = fs::read_to_string(&file_path)
            .map_err(|e| ContextError::Storage(format!("Failed to read session file: {}", e)))?;
        
        let session: Session = serde_json::from_str(&session_data)?;
        
        debug!("Loaded session {} from {}", session_id, file_path.display());
        Ok(session)
    }
    
    fn load_latest_session(&self) -> Result<Option<Session>, ContextError> {
        if !self.latest_symlink.exists() {
            debug!("No latest session symlink found");
            return Ok(None);
        }
        
        // Read the symlink target or the file content
        #[cfg(unix)]
        let target_path = {
            let target = fs::read_link(&self.latest_symlink)
                .map_err(|e| ContextError::Storage(format!("Failed to read symlink: {}", e)))?;
            
            if target.is_relative() {
                self.sessions_dir.join(target)
            } else {
                target
            }
        };
        
        #[cfg(windows)]
        let target_path = self.latest_symlink.clone();
        
        if !target_path.exists() {
            warn!("Latest session symlink points to non-existent file");
            return Ok(None);
        }
        
        let session_data = fs::read_to_string(&target_path)
            .map_err(|e| ContextError::Storage(format!("Failed to read latest session: {}", e)))?;
        
        let session: Session = serde_json::from_str(&session_data)?;
        
        debug!("Loaded latest session: {}", session.id);
        Ok(Some(session))
    }
    
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, ContextError> {
        let mut sessions = Vec::new();
        
        let entries = fs::read_dir(&self.sessions_dir)
            .map_err(|e| ContextError::Storage(format!("Failed to read sessions directory: {}", e)))?;
        
        for entry in entries {
            let entry = entry
                .map_err(|e| ContextError::Storage(format!("Failed to read directory entry: {}", e)))?;
            
            let path = entry.path();
            
            // Skip non-JSON files and the latest symlink
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            
            if path.file_name() == Some(std::ffi::OsStr::new("latest.json")) {
                continue;
            }
            
            match self.get_session_info(&path) {
                Ok(info) => sessions.push(info),
                Err(e) => warn!("Failed to get info for session file {}: {}", path.display(), e),
            }
        }
        
        // Sort by modification time (newest first)
        sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        
        debug!("Listed {} sessions", sessions.len());
        Ok(sessions)
    }
    
    fn delete_session(&self, session_id: &Uuid) -> Result<(), ContextError> {
        let file_path = self.session_file_path(session_id);
        
        if !file_path.exists() {
            return Err(ContextError::SessionNotFound(session_id.to_string()));
        }
        
        fs::remove_file(&file_path)
            .map_err(|e| ContextError::Storage(format!("Failed to delete session file: {}", e)))?;
        
        // If this was the latest session, remove the symlink
        if let Ok(Some(latest)) = self.load_latest_session() {
            if latest.id == *session_id {
                if self.latest_symlink.exists() {
                    fs::remove_file(&self.latest_symlink)
                        .map_err(|e| ContextError::Storage(format!("Failed to remove latest symlink: {}", e)))?;
                }
            }
        }
        
        info!("Deleted session {}", session_id);
        Ok(())
    }
    
    fn cleanup_old_sessions(&self, keep_count: usize) -> Result<usize, ContextError> {
        let sessions = self.list_sessions()?;
        
        if sessions.len() <= keep_count {
            debug!("No sessions to clean up (have {}, keeping {})", sessions.len(), keep_count);
            return Ok(0);
        }
        
        let to_delete = &sessions[keep_count..];
        let mut deleted_count = 0;
        
        for session_info in to_delete {
            match self.delete_session(&session_info.id) {
                Ok(()) => {
                    deleted_count += 1;
                    debug!("Cleaned up old session {}", session_info.id);
                }
                Err(e) => warn!("Failed to delete old session {}: {}", session_info.id, e),
            }
        }
        
        info!("Cleaned up {} old sessions", deleted_count);
        Ok(deleted_count)
    }
}

impl Default for FileStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create default file storage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Message, MessageRole};
    use tempfile::TempDir;
    
    #[test]
    fn test_file_storage_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::with_directory(temp_dir.path()).unwrap();
        
        // Create a test session
        let mut session = Session::new();
        session.add_message(Message::new(MessageRole::User, "Hello".to_string()));
        session.add_message(Message::new(MessageRole::Assistant, "Hi there!".to_string()));
        
        // Save session
        storage.save_session(&session).unwrap();
        
        // Load session
        let loaded = storage.load_session(&session.id).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.messages.len(), 2);
        
        // Load latest session
        let latest = storage.load_latest_session().unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().id, session.id);
        
        // List sessions
        let sessions = storage.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session.id);
        
        // Delete session
        storage.delete_session(&session.id).unwrap();
        let sessions = storage.list_sessions().unwrap();
        assert_eq!(sessions.len(), 0);
    }
    
    #[test]
    fn test_cleanup_old_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::with_directory(temp_dir.path()).unwrap();
        
        // Create multiple sessions
        let mut sessions = Vec::new();
        for i in 0..5 {
            let mut session = Session::new();
            session.add_message(Message::new(MessageRole::User, format!("Message {}", i)));
            storage.save_session(&session).unwrap();
            sessions.push(session);
            
            // Sleep to ensure different modification times
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Keep only 2 sessions
        let deleted = storage.cleanup_old_sessions(2).unwrap();
        assert_eq!(deleted, 3);
        
        let remaining = storage.list_sessions().unwrap();
        assert_eq!(remaining.len(), 2);
    }
}