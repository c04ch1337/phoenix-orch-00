//! Context Management - Hierarchical memory for conversation context
//!
//! Provides three-tier context:
//! - Short-term: Last N raw messages
//! - Episodic: Summarized conversation chunks
//! - Long-term: Semantic memory store

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub turn_number: u64,
}

/// Short-term buffer - maintains last N messages in raw form
#[derive(Debug, Clone)]
pub struct ShortTermBuffer {
    messages: Vec<Message>,
    max_size: usize,
    current_turn: u64,
}

impl ShortTermBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: Vec::with_capacity(max_size * 2),
            max_size,
            current_turn: 0,
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        let msg = Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            turn_number: self.current_turn,
        };
        
        self.messages.push(msg);
        
        // Keep only last max_size * 2 messages (user + assistant pairs)
        while self.messages.len() > self.max_size * 2 {
            self.messages.remove(0);
        }
        
        if role == "assistant" {
            self.current_turn += 1;
        }
    }
    
    pub fn get_recent_messages(&self, count: usize) -> Vec<Message> {
        let start = if self.messages.len() > count * 2 {
            self.messages.len() - count * 2
        } else {
            0
        };
        self.messages[start..].to_vec()
    }
    
    pub fn get_all_messages(&self) -> Vec<Message> {
        self.messages.clone()
    }
    
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
    
    pub fn total_chars(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }
    
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// An episodic summary - compressed representation of a conversation chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub summary: String,
    pub turn_number: u64,
    pub message_count: usize,
    pub timestamp: String,
    pub key_topics: Vec<String>,
}

/// Episodic memory - stores summaries of conversation chunks
#[derive(Debug, Clone)]
pub struct EpisodicMemory {
    episodes: Vec<Episode>,
}

impl EpisodicMemory {
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
        }
    }
    
    pub fn add_episode(&mut self, summary: &str, turn_number: u64) {
        let episode = Episode {
            id: uuid::Uuid::new_v4().to_string(),
            summary: summary.to_string(),
            turn_number,
            message_count: 12, // Approximate
            timestamp: chrono::Utc::now().to_rfc3339(),
            key_topics: Self::extract_topics(summary),
        };
        
        self.episodes.push(episode);
        tracing::debug!("Added episode at turn {}: {}", turn_number, summary);
    }
    
    fn extract_topics(summary: &str) -> Vec<String> {
        // Simple keyword extraction
        let keywords = ["Rust", "Python", "project", "API", "security", "build", "deploy"];
        keywords.iter()
            .filter(|k| summary.to_lowercase().contains(&k.to_lowercase()))
            .map(|k| k.to_string())
            .collect()
    }
    
    pub fn episode_count(&self) -> usize {
        self.episodes.len()
    }
    
    pub fn total_chars(&self) -> usize {
        self.episodes.iter().map(|e| e.summary.len()).sum()
    }
    
    pub fn get_old_episodes(&self, keep_recent: usize) -> Vec<Episode> {
        if self.episodes.len() <= keep_recent {
            return Vec::new();
        }
        self.episodes[..self.episodes.len() - keep_recent].to_vec()
    }
    
    pub fn search_relevant(&self, query: &str, top_k: usize) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        
        let mut scored: Vec<(usize, &Episode)> = self.episodes.iter()
            .map(|ep| {
                let summary_lower = ep.summary.to_lowercase();
                let score = query_words.iter()
                    .filter(|w| summary_lower.contains(*w))
                    .count();
                (score, ep)
            })
            .filter(|(score, _)| *score > 0)
            .collect();
        
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        
        scored.into_iter()
            .take(top_k)
            .map(|(_, ep)| ep.summary.clone())
            .collect()
    }
}

impl Default for EpisodicMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// A memory item in long-term storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub turn_created: u64,
    pub timestamp: String,
    pub embedding: Option<Vec<f32>>,
    pub keywords: Vec<String>,
}

/// Long-term memory - semantic store of facts and entities
#[derive(Debug, Clone)]
pub struct LongTermMemory {
    memories: Vec<MemoryItem>,
    keyword_index: HashMap<String, Vec<usize>>,
}

impl LongTermMemory {
    pub fn new() -> Self {
        Self {
            memories: Vec::new(),
            keyword_index: HashMap::new(),
        }
    }
    
    pub fn add_memory(&mut self, content: &str, memory_type: &str, turn_created: u64) {
        let keywords = Self::extract_keywords(content);
        let idx = self.memories.len();
        
        let item = MemoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_string(),
            memory_type: memory_type.to_string(),
            turn_created,
            timestamp: chrono::Utc::now().to_rfc3339(),
            embedding: None, // Would be computed via embedding model
            keywords: keywords.clone(),
        };
        
        self.memories.push(item);
        
        // Index by keywords
        for keyword in keywords {
            self.keyword_index
                .entry(keyword.to_lowercase())
                .or_default()
                .push(idx);
        }
    }
    
    fn extract_keywords(content: &str) -> Vec<String> {
        let stopwords = ["the", "a", "an", "is", "are", "was", "were", "in", "on", "at", "to", "for"];
        
        content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .filter(|w| !stopwords.contains(&w.to_lowercase().as_str()))
            .take(10)
            .map(|w| w.to_string())
            .collect()
    }
    
    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }
    
    pub fn search_by_keywords(&self, query: &str, top_k: usize) -> Vec<String> {
        let query_words: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .collect();
        
        // Count matches per memory
        let mut matches: HashMap<usize, usize> = HashMap::new();
        
        for word in &query_words {
            if let Some(indices) = self.keyword_index.get(word) {
                for &idx in indices {
                    *matches.entry(idx).or_default() += 1;
                }
            }
        }
        
        // Sort by match count
        let mut sorted: Vec<(usize, usize)> = matches.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        
        sorted.into_iter()
            .take(top_k)
            .filter_map(|(idx, _)| self.memories.get(idx))
            .map(|m| m.content.clone())
            .collect()
    }
    
    /// Cosine similarity between two vectors
    #[allow(dead_code)]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        
        dot / (norm_a * norm_b)
    }
}

impl Default for LongTermMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Context manager combining all memory layers
pub struct ContextManager {
    pub short_term: ShortTermBuffer,
    pub episodic: EpisodicMemory,
    pub long_term: LongTermMemory,
}

impl ContextManager {
    pub fn new(short_term_size: usize) -> Self {
        Self {
            short_term: ShortTermBuffer::new(short_term_size),
            episodic: EpisodicMemory::new(),
            long_term: LongTermMemory::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_short_term_buffer() {
        let mut buffer = ShortTermBuffer::new(4);
        
        buffer.add_message("user", "Hello");
        buffer.add_message("assistant", "Hi there");
        buffer.add_message("user", "How are you?");
        buffer.add_message("assistant", "I'm good!");
        
        assert_eq!(buffer.message_count(), 4);
        
        let recent = buffer.get_recent_messages(2);
        assert_eq!(recent.len(), 4); // 2 turns = 4 messages
    }
    
    #[test]
    fn test_episodic_search() {
        let mut memory = EpisodicMemory::new();
        
        memory.add_episode("User discussed building a Rust API", 1);
        memory.add_episode("User asked about Python deployment", 2);
        memory.add_episode("User wanted security features", 3);
        
        let results = memory.search_relevant("Rust security", 5);
        assert!(!results.is_empty());
    }
    
    #[test]
    fn test_long_term_keyword_search() {
        let mut memory = LongTermMemory::new();
        
        memory.add_memory("The user is building NexusFlow project in Rust", "fact", 1);
        memory.add_memory("User prefers functional programming style", "preference", 2);
        
        let results = memory.search_by_keywords("NexusFlow Rust", 5);
        assert!(!results.is_empty());
        assert!(results[0].contains("NexusFlow"));
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((LongTermMemory::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        
        let c = vec![0.0, 1.0, 0.0];
        assert!((LongTermMemory::cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }
}
