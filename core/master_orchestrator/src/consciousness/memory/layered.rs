//! Layered Memory System for Consciousness
//!
//! Provides per-layer memory stores that integrate with the existing SemanticMemory infrastructure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Layered Memory System - manages memory per consciousness layer  
#[derive(Debug, Clone)]
pub struct LayeredMemorySystem {
    /// Base path for layer memories
    pub base_path: String,
    /// Memory usage per layer
    pub layer_usage: HashMap<String, MemoryUsage>,
}

/// Memory usage statistics for a layer
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryUsage {
    pub entries_count: u64,
    pub total_bytes: u64,
    pub last_access: Option<String>,
}

impl LayeredMemorySystem {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            layer_usage: HashMap::new(),
        }
    }
    
    /// Get memory path for a specific layer
    pub fn layer_path(&self, layer: &str) -> String {
        format!("{}/{}", self.base_path, layer)
    }
    
    /// Update usage statistics for a layer
    pub fn update_usage(&mut self, layer: &str, entries: u64, bytes: u64) {
        let usage = self.layer_usage.entry(layer.to_string()).or_default();
        usage.entries_count = entries;
        usage.total_bytes = bytes;
        usage.last_access = Some(chrono::Utc::now().to_rfc3339());
    }
    
    /// Get total memory usage across all layers
    pub fn total_usage(&self) -> u64 {
        self.layer_usage.values().map(|u| u.total_bytes).sum()
    }
}
