//! Reasoning Streamer - Context-Aware Streaming Reasoning Engine
//!
//! A high-performance, fully async, context-aware reasoning engine that provides
//! real-time thought streaming with hierarchical context management.

pub mod context;
pub mod streaming;
pub mod profile;

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use context::{ShortTermBuffer, EpisodicMemory, LongTermMemory, ContextManager};
pub use streaming::{ThoughtStream, ThoughtChunk, StreamingConfig};
pub use profile::{UserProfile, EntityExtractor, PreferenceTracker};

/// Configuration for the reasoning streamer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Maximum messages in short-term buffer
    pub short_term_size: usize,
    /// Turns between episodic summaries
    pub episodic_interval: usize,
    /// Maximum token budget before compression
    pub max_tokens: usize,
    /// Token threshold to trigger compression (0.0-1.0)
    pub compression_threshold: f32,
    /// Delay between streaming chunks in milliseconds
    pub chunk_delay_ms: u64,
    /// Number of relevant memories to retrieve
    pub retrieval_top_k: usize,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            short_term_size: 8,
            episodic_interval: 12,
            max_tokens: 4096,
            compression_threshold: 0.75,
            chunk_delay_ms: 120,
            retrieval_top_k: 5,
        }
    }
}

/// Main context-aware reasoning streamer
pub struct ContextAwareReasoningStreamer {
    /// Short-term buffer - last N messages
    pub short_term: Arc<RwLock<ShortTermBuffer>>,
    
    /// Episodic memory - summaries of conversation chunks
    pub episodic: Arc<RwLock<EpisodicMemory>>,
    
    /// Long-term memory - semantic store of facts and entities
    pub long_term: Arc<RwLock<LongTermMemory>>,
    
    /// User profile - preferences and entities
    pub user_profile: Arc<RwLock<UserProfile>>,
    
    /// Streaming configuration
    pub streaming_config: StreamingConfig,
    
    /// Overall configuration
    pub config: ReasoningConfig,
    
    /// Total turns in conversation
    pub turn_count: Arc<RwLock<u64>>,
    
    /// Current token usage estimate
    pub token_usage: Arc<RwLock<usize>>,
}

impl ContextAwareReasoningStreamer {
    /// Create a new reasoning streamer with default config
    pub fn new() -> Self {
        Self::with_config(ReasoningConfig::default())
    }
    
    /// Create a new reasoning streamer with custom config
    pub fn with_config(config: ReasoningConfig) -> Self {
        Self {
            short_term: Arc::new(RwLock::new(ShortTermBuffer::new(config.short_term_size))),
            episodic: Arc::new(RwLock::new(EpisodicMemory::new())),
            long_term: Arc::new(RwLock::new(LongTermMemory::new())),
            user_profile: Arc::new(RwLock::new(UserProfile::new())),
            streaming_config: StreamingConfig {
                chunk_delay_ms: config.chunk_delay_ms,
                use_spinners: true,
                animate_thoughts: true,
            },
            config,
            turn_count: Arc::new(RwLock::new(0)),
            token_usage: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Process a user message and prepare context for reasoning
    pub async fn process_message(&self, user_message: &str, assistant_response: Option<&str>) {
        // Increment turn count
        {
            let mut count = self.turn_count.write().await;
            *count += 1;
        }
        
        // Add to short-term buffer
        {
            let mut buffer = self.short_term.write().await;
            buffer.add_message("user", user_message);
            if let Some(response) = assistant_response {
                buffer.add_message("assistant", response);
            }
        }
        
        // Extract entities and preferences
        {
            let mut profile = self.user_profile.write().await;
            profile.extract_from_message(user_message);
        }
        
        // Check if we need to create an episodic summary
        let turn_count = *self.turn_count.read().await;
        if turn_count % self.config.episodic_interval as u64 == 0 && turn_count > 0 {
            self.create_episodic_summary().await;
        }
        
        // Check if we need to compress
        let current_usage = *self.token_usage.read().await;
        let threshold = (self.config.max_tokens as f32 * self.config.compression_threshold) as usize;
        if current_usage > threshold {
            self.trigger_compression().await;
        }
    }
    
    /// Create an episodic summary from recent messages
    async fn create_episodic_summary(&self) {
        let buffer = self.short_term.read().await;
        let messages = buffer.get_all_messages();
        
        if messages.is_empty() {
            return;
        }
        
        // Create a simple summary (in production, this would call the LLM)
        let summary = format!(
            "Episode {} ({} messages): {}",
            self.episodic.read().await.episode_count() + 1,
            messages.len(),
            messages.iter().take(2).map(|m| m.content.chars().take(50).collect::<String>()).collect::<Vec<_>>().join(" | ")
        );
        
        let turn_count = *self.turn_count.read().await;
        
        {
            let mut episodic = self.episodic.write().await;
            episodic.add_episode(&summary, turn_count);
        }
        
        tracing::debug!("Created episodic summary at turn {}", turn_count);
    }
    
    /// Trigger context compression
    async fn trigger_compression(&self) {
        tracing::info!("Triggering context compression due to token budget");
        
        // Move old episodic memories to long-term
        let episodes_to_move = {
            let episodic = self.episodic.read().await;
            episodic.get_old_episodes(3) // Keep last 3, move rest
        };
        
        if !episodes_to_move.is_empty() {
            let mut long_term = self.long_term.write().await;
            for episode in episodes_to_move {
                long_term.add_memory(&episode.summary, "episodic_summary", episode.turn_number);
            }
        }
        
        // Update token usage estimate
        let new_usage = self.estimate_token_usage().await;
        *self.token_usage.write().await = new_usage;
    }
    
    /// Estimate current token usage
    async fn estimate_token_usage(&self) -> usize {
        let short_term = self.short_term.read().await;
        let episodic = self.episodic.read().await;
        let profile = self.user_profile.read().await;
        
        // Rough estimate: 4 chars per token
        let short_term_tokens = short_term.total_chars() / 4;
        let episodic_tokens = episodic.total_chars() / 4;
        let profile_tokens = profile.to_context_string().len() / 4;
        
        short_term_tokens + episodic_tokens + profile_tokens
    }
    
    /// Get relevant context for a query
    pub async fn get_relevant_context(&self, query: &str) -> RelevantContext {
        let short_term = self.short_term.read().await;
        let episodic = self.episodic.read().await;
        let long_term = self.long_term.read().await;
        let profile = self.user_profile.read().await;
        
        // Get recent messages
        let recent_messages = short_term.get_recent_messages(self.config.short_term_size);
        
        // Get relevant episodic memories (simple keyword matching for now)
        let relevant_episodes = episodic.search_relevant(query, 3);
        
        // Get relevant long-term memories
        let relevant_long_term = long_term.search_by_keywords(query, self.config.retrieval_top_k);
        
        // Get user profile context
        let profile_context = profile.to_context_string();
        
        // Check for contradictions
        let contradictions = profile.detect_contradictions(query);
        
        RelevantContext {
            recent_messages,
            relevant_episodes,
            relevant_long_term,
            profile_context,
            contradictions,
        }
    }
    
    /// Build streaming thoughts for a query
    pub async fn build_thought_stream(&self, query: &str) -> Vec<ThoughtChunk> {
        let context = self.get_relevant_context(query).await;
        let profile = self.user_profile.read().await;
        
        let mut thoughts = Vec::new();
        
        // Phase 1: Research
        thoughts.push(ThoughtChunk::new(
            "🔍 Researching relevant background...",
            ThoughtType::Research,
        ));
        
        // Phase 2: Recall relevant memories
        if !context.relevant_episodes.is_empty() || !context.relevant_long_term.is_empty() {
            for episode in context.relevant_episodes.iter().take(2) {
                thoughts.push(ThoughtChunk::new(
                    &format!("💭 Recalling: {}", episode.chars().take(80).collect::<String>()),
                    ThoughtType::Recall,
                ));
            }
        }
        
        // Phase 3: Detect preference changes
        if !context.contradictions.is_empty() {
            for contradiction in &context.contradictions {
                thoughts.push(ThoughtChunk::new(
                    &format!("⚠️ Detecting shift: {}", contradiction),
                    ThoughtType::Detection,
                ));
            }
        }
        
        // Phase 4: User profile relevance
        if !profile.projects.is_empty() {
            let project = profile.projects.last().unwrap();
            thoughts.push(ThoughtChunk::new(
                &format!("📋 Project context: {} ({})", project.name, project.language),
                ThoughtType::Context,
            ));
        }
        
        // Phase 5: Planning
        thoughts.push(ThoughtChunk::new(
            "📝 Breaking down implementation into phases...",
            ThoughtType::Planning,
        ));
        
        // Phase 6: Generating
        thoughts.push(ThoughtChunk::new(
            "⚡ Generating response with context awareness...",
            ThoughtType::Generating,
        ));
        
        thoughts
    }
    
    /// Get conversation statistics
    pub async fn get_stats(&self) -> ConversationStats {
        let turn_count = *self.turn_count.read().await;
        let token_usage = self.estimate_token_usage().await;
        let short_term = self.short_term.read().await;
        let episodic = self.episodic.read().await;
        let long_term = self.long_term.read().await;
        let profile = self.user_profile.read().await;
        
        ConversationStats {
            turn_count,
            token_usage,
            short_term_messages: short_term.message_count(),
            episodic_summaries: episodic.episode_count(),
            long_term_memories: long_term.memory_count(),
            tracked_entities: profile.entity_count(),
            tracked_preferences: profile.preference_count(),
            tracked_projects: profile.projects.len(),
        }
    }
}

impl Default for ContextAwareReasoningStreamer {
    fn default() -> Self {
        Self::new()
    }
}

/// Relevant context retrieved for a query
#[derive(Debug, Clone)]
pub struct RelevantContext {
    pub recent_messages: Vec<context::Message>,
    pub relevant_episodes: Vec<String>,
    pub relevant_long_term: Vec<String>,
    pub profile_context: String,
    pub contradictions: Vec<String>,
}

/// Type of thought being streamed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThoughtType {
    Research,
    Recall,
    Detection,
    Context,
    Planning,
    Generating,
}

/// Conversation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationStats {
    pub turn_count: u64,
    pub token_usage: usize,
    pub short_term_messages: usize,
    pub episodic_summaries: usize,
    pub long_term_memories: usize,
    pub tracked_entities: usize,
    pub tracked_preferences: usize,
    pub tracked_projects: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_message_processing() {
        let streamer = ContextAwareReasoningStreamer::new();
        
        streamer.process_message("Hello, I'm working on a project", Some("Great! Tell me more.")).await;
        
        let stats = streamer.get_stats().await;
        assert_eq!(stats.turn_count, 1);
        assert!(stats.short_term_messages > 0);
    }
    
    #[tokio::test]
    async fn test_entity_extraction() {
        let streamer = ContextAwareReasoningStreamer::new();
        
        streamer.process_message("I'm building NexusFlow in Rust", None).await;
        
        let profile = streamer.user_profile.read().await;
        assert!(!profile.projects.is_empty());
        assert_eq!(profile.projects[0].name, "NexusFlow");
    }
}
