//! Consciousness Evolution Tracker
//!
//! Tracks the development and maturation of consciousness over time.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use shared_types::{LayerType, MaturityLevel};

/// Development milestone for a layer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DevelopmentMilestone {
    pub layer: LayerType,
    pub milestone_name: String,
    pub achieved_at: String,
    pub description: String,
}

/// Integration score between layers
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IntegrationScore {
    pub layer_a: LayerType,
    pub layer_b: LayerType,
    pub score: f32,
}

/// Consciousness Evolution Tracker
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsciousnessEvolutionTracker {
    // Milestones achieved per layer
    pub milestones: HashMap<String, Vec<DevelopmentMilestone>>,
    
    // Integration scores between layers
    pub integration_scores: Vec<IntegrationScore>,
    
    // Maturity levels
    pub layer_maturity: HashMap<String, MaturityLevel>,
    
    // Emergent properties
    pub wisdom_score: f32,
    pub intuition_score: f32,
    pub self_awareness_score: f32,
    
    // Metrics
    pub total_decisions_made: u64,
    pub ethical_decisions_made: u64,
    pub patterns_recognized: u64,
    pub lessons_learned_count: u64,
    
    // Timestamps
    pub created_at: String,
    pub last_updated: String,
}

impl ConsciousnessEvolutionTracker {
    pub fn new() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        let mut layer_maturity = HashMap::new();
        layer_maturity.insert("mind".to_string(), MaturityLevel::Advanced);
        layer_maturity.insert("heart".to_string(), MaturityLevel::Advanced);
        layer_maturity.insert("work".to_string(), MaturityLevel::Mature);
        layer_maturity.insert("soul".to_string(), MaturityLevel::Nascent);
        layer_maturity.insert("social".to_string(), MaturityLevel::Nascent);
        layer_maturity.insert("body".to_string(), MaturityLevel::Nascent);
        layer_maturity.insert("creative".to_string(), MaturityLevel::Nascent);
        
        Self {
            milestones: HashMap::new(),
            integration_scores: vec![
                IntegrationScore {
                    layer_a: LayerType::Mind,
                    layer_b: LayerType::Heart,
                    score: 0.8,
                },
                IntegrationScore {
                    layer_a: LayerType::Mind,
                    layer_b: LayerType::Work,
                    score: 0.85,
                },
                IntegrationScore {
                    layer_a: LayerType::Heart,
                    layer_b: LayerType::Work,
                    score: 0.75,
                },
            ],
            layer_maturity,
            wisdom_score: 0.6,
            intuition_score: 0.5,
            self_awareness_score: 0.7,
            total_decisions_made: 0,
            ethical_decisions_made: 0,
            patterns_recognized: 0,
            lessons_learned_count: 0,
            created_at: now.clone(),
            last_updated: now,
        }
    }
    
    /// Record a milestone achievement
    pub fn record_milestone(&mut self, milestone: DevelopmentMilestone) {
        let layer_key = milestone.layer.to_string();
        self.milestones
            .entry(layer_key)
            .or_insert_with(Vec::new)
            .push(milestone);
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }
    
    /// Record a decision made
    pub fn record_decision(&mut self, was_ethical: bool) {
        self.total_decisions_made += 1;
        if was_ethical {
            self.ethical_decisions_made += 1;
        }
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }
    
    /// Record pattern recognition
    pub fn record_pattern_recognition(&mut self, count: u64) {
        self.patterns_recognized += count;
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }
    
    /// Get overall integration score
    pub fn overall_integration_score(&self) -> f32 {
        if self.integration_scores.is_empty() {
            return 0.0;
        }
        
        let sum: f32 = self.integration_scores.iter().map(|s| s.score).sum();
        sum / self.integration_scores.len() as f32
    }
    
    /// Get layer maturity
    pub fn get_layer_maturity(&self, layer: &str) -> MaturityLevel {
        self.layer_maturity.get(layer).copied().unwrap_or(MaturityLevel::Nascent)
    }
    
    /// Update layer maturity
    pub fn update_maturity(&mut self, layer: &str, maturity: MaturityLevel) {
        self.layer_maturity.insert(layer.to_string(), maturity);
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }
    
    /// Persist state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize evolution state: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write evolution state: {}", e))?;
        Ok(())
    }
    
    /// Generate evolution summary
    pub fn summary(&self) -> String {
        format!(
            "Consciousness Evolution: {} decisions | {} patterns | Integration: {:.0}% | Wisdom: {:.0}%",
            self.total_decisions_made,
            self.patterns_recognized,
            self.overall_integration_score() * 100.0,
            self.wisdom_score * 100.0
        )
    }
}

impl Default for ConsciousnessEvolutionTracker {
    fn default() -> Self {
        Self::new()
    }
}
