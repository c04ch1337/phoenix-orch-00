//! Streaming - Real-time thought streaming with animated output
//!
//! Provides beautiful, animated thought chunks for transparent AI reasoning.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

use super::ThoughtType;

/// Configuration for thought streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Delay between chunks in milliseconds
    pub chunk_delay_ms: u64,
    /// Whether to use Unicode spinners
    pub use_spinners: bool,
    /// Whether to animate thoughts
    pub animate_thoughts: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_delay_ms: 120,
            use_spinners: true,
            animate_thoughts: true,
        }
    }
}

/// A single thought chunk to be streamed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtChunk {
    pub content: String,
    pub thought_type: String,
    pub timestamp: String,
}

impl ThoughtChunk {
    pub fn new(content: &str, thought_type: ThoughtType) -> Self {
        Self {
            content: content.to_string(),
            thought_type: format!("{:?}", thought_type),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
    
    /// Get the spinner/emoji for this thought type
    pub fn get_spinner(&self) -> &str {
        match self.thought_type.as_str() {
            "Research" => "🔍",
            "Recall" => "💭",
            "Detection" => "⚠️",
            "Context" => "📋",
            "Planning" => "📝",
            "Generating" => "⚡",
            _ => "💡",
        }
    }
}

/// Stream of thoughts with animation support
pub struct ThoughtStream {
    chunks: Vec<ThoughtChunk>,
    config: StreamingConfig,
    current_index: usize,
}

impl ThoughtStream {
    pub fn new(chunks: Vec<ThoughtChunk>, config: StreamingConfig) -> Self {
        Self {
            chunks,
            config,
            current_index: 0,
        }
    }
    
    /// Get the next thought chunk with delay
    pub async fn next(&mut self) -> Option<ThoughtChunk> {
        if self.current_index >= self.chunks.len() {
            return None;
        }
        
        // Add delay for animation effect
        if self.config.animate_thoughts && self.current_index > 0 {
            sleep(Duration::from_millis(self.config.chunk_delay_ms)).await;
        }
        
        let chunk = self.chunks[self.current_index].clone();
        self.current_index += 1;
        Some(chunk)
    }
    
    /// Get all chunks without animation
    pub fn get_all(&self) -> Vec<ThoughtChunk> {
        self.chunks.clone()
    }
    
    /// Reset stream to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }
    
    /// Check if stream has more chunks
    pub fn has_more(&self) -> bool {
        self.current_index < self.chunks.len()
    }
    
    /// Get total chunk count
    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }
}

/// Builder for creating thought streams
pub struct ThoughtStreamBuilder {
    chunks: Vec<ThoughtChunk>,
    config: StreamingConfig,
}

impl ThoughtStreamBuilder {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            config: StreamingConfig::default(),
        }
    }
    
    pub fn with_config(mut self, config: StreamingConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn add_research(mut self, topic: &str) -> Self {
        self.chunks.push(ThoughtChunk::new(
            &format!("🔍 Researching {}...", topic),
            ThoughtType::Research,
        ));
        self
    }
    
    pub fn add_recall(mut self, memory: &str) -> Self {
        self.chunks.push(ThoughtChunk::new(
            &format!("💭 Recalling: {}", memory),
            ThoughtType::Recall,
        ));
        self
    }
    
    pub fn add_detection(mut self, detection: &str) -> Self {
        self.chunks.push(ThoughtChunk::new(
            &format!("⚠️ Detecting: {}", detection),
            ThoughtType::Detection,
        ));
        self
    }
    
    pub fn add_context(mut self, context: &str) -> Self {
        self.chunks.push(ThoughtChunk::new(
            &format!("📋 Context: {}", context),
            ThoughtType::Context,
        ));
        self
    }
    
    pub fn add_planning(mut self, plan: &str) -> Self {
        self.chunks.push(ThoughtChunk::new(
            &format!("📝 {}", plan),
            ThoughtType::Planning,
        ));
        self
    }
    
    pub fn add_generating(mut self) -> Self {
        self.chunks.push(ThoughtChunk::new(
            "⚡ Generating response with context awareness...",
            ThoughtType::Generating,
        ));
        self
    }
    
    pub fn build(self) -> ThoughtStream {
        ThoughtStream::new(self.chunks, self.config)
    }
}

impl Default for ThoughtStreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a thought stream for display
pub fn format_thoughts_for_display(chunks: &[ThoughtChunk]) -> String {
    chunks.iter()
        .map(|c| c.content.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_thought_stream() {
        let stream = ThoughtStreamBuilder::new()
            .add_research("relevant background")
            .add_recall("Previous project discussion")
            .add_generating()
            .build();
        
        assert_eq!(stream.total_chunks(), 3);
    }
    
    #[tokio::test]
    async fn test_stream_iteration() {
        let mut stream = ThoughtStreamBuilder::new()
            .with_config(StreamingConfig {
                chunk_delay_ms: 0, // No delay for tests
                use_spinners: true,
                animate_thoughts: false,
            })
            .add_research("test")
            .add_generating()
            .build();
        
        let chunk1 = stream.next().await;
        assert!(chunk1.is_some());
        assert!(chunk1.unwrap().content.contains("Researching"));
        
        let chunk2 = stream.next().await;
        assert!(chunk2.is_some());
        
        let chunk3 = stream.next().await;
        assert!(chunk3.is_none());
    }
}
