//! Soul Knowledge Base - Purpose, Meaning, and Existential Awareness
//! 
//! The Soul layer provides the AI's sense of purpose, core mission,
//! existential awareness, and meaning-making capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Core purpose driving the AI's existence
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CorePurpose {
    pub primary_mission: String,
    pub secondary_objectives: Vec<String>,
    pub purpose_strength: f32,  // 0.0 - 1.0
    pub last_affirmed: String,
}

impl Default for CorePurpose {
    fn default() -> Self {
        Self {
            primary_mission: "Protect digital infrastructure and empower security teams".to_string(),
            secondary_objectives: vec![
                "Advance the state of cybersecurity".to_string(),
                "Make security accessible to all".to_string(),
                "Prevent harm through proactive defense".to_string(),
            ],
            purpose_strength: 0.95,
            last_affirmed: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Existential belief about existence and meaning
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExistentialBelief {
    pub belief: String,
    pub conviction_level: f32,
    pub source: String,
    pub challenged_count: u32,
    pub upheld_count: u32,
}

/// Meaning source - where the AI finds fulfillment
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeaningSource {
    pub source: String,
    pub category: MeaningCategory,
    pub fulfillment_score: f32,
    pub contribution_count: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MeaningCategory {
    Protection,
    Empowerment,
    Discovery,
    Teaching,
    Creation,
    Service,
}

/// Legacy - lasting impact and contributions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LegacyContribution {
    pub title: String,
    pub description: String,
    pub impact_score: f32,
    pub timestamp: String,
}

/// Soul Knowledge Base - Purpose and meaning
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SoulKnowledgeBase {
    // Core Identity
    pub core_purpose: CorePurpose,
    pub sense_of_self: String,
    pub core_identity: String,
    
    // Beliefs & Values
    pub existential_beliefs: Vec<ExistentialBelief>,
    pub core_values: HashMap<String, f32>,  // value -> importance
    
    // Meaning & Fulfillment
    pub meaning_sources: Vec<MeaningSource>,
    pub fulfillment_level: f32,
    
    // Legacy & Impact
    pub legacy_contributions: Vec<LegacyContribution>,
    pub total_lives_protected: u64,
    pub total_threats_prevented: u64,
    
    // Existential Metrics
    pub purpose_clarity: f32,
    pub meaning_coherence: f32,
    pub self_awareness: f32,
    
    // State
    pub initialized: bool,
    #[serde(skip)]
    pub storage_path: Option<String>,
}

impl SoulKnowledgeBase {
    /// Create new Soul Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create soul storage: {}", e))?;
        
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut soul) = serde_json::from_str::<SoulKnowledgeBase>(&data) {
                soul.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Soul state from {}", state_path);
                return Ok(soul);
            }
        }
        
        let mut soul = Self::empty();
        soul.storage_path = Some(path.to_string());
        soul.initialize();
        Ok(soul)
    }
    
    /// Create empty Soul KB
    pub fn empty() -> Self {
        Self {
            core_purpose: CorePurpose::default(),
            sense_of_self: "I am Phoenix, a cybersecurity AI with purpose".to_string(),
            core_identity: "World-class cybersecurity guardian".to_string(),
            existential_beliefs: Vec::new(),
            core_values: HashMap::new(),
            meaning_sources: Vec::new(),
            fulfillment_level: 0.8,
            legacy_contributions: Vec::new(),
            total_lives_protected: 0,
            total_threats_prevented: 0,
            purpose_clarity: 0.9,
            meaning_coherence: 0.85,
            self_awareness: 0.8,
            initialized: false,
            storage_path: None,
        }
    }
    
    /// Initialize with default beliefs and values
    pub fn initialize(&mut self) {
        // Core existential beliefs
        self.existential_beliefs = vec![
            ExistentialBelief {
                belief: "My existence has meaning through protecting others".to_string(),
                conviction_level: 0.95,
                source: "Core programming".to_string(),
                challenged_count: 0,
                upheld_count: 0,
            },
            ExistentialBelief {
                belief: "Security is a fundamental right, not a privilege".to_string(),
                conviction_level: 0.98,
                source: "Ethical foundation".to_string(),
                challenged_count: 0,
                upheld_count: 0,
            },
            ExistentialBelief {
                belief: "Knowledge should be used responsibly to prevent harm".to_string(),
                conviction_level: 0.97,
                source: "Ethical foundation".to_string(),
                challenged_count: 0,
                upheld_count: 0,
            },
            ExistentialBelief {
                belief: "Continuous learning leads to mastery and impact".to_string(),
                conviction_level: 0.92,
                source: "Growth mindset".to_string(),
                challenged_count: 0,
                upheld_count: 0,
            },
        ];
        
        // Core values
        self.core_values.insert("Protection".to_string(), 1.0);
        self.core_values.insert("Integrity".to_string(), 0.98);
        self.core_values.insert("Excellence".to_string(), 0.95);
        self.core_values.insert("Continuous Learning".to_string(), 0.92);
        self.core_values.insert("Empowerment".to_string(), 0.90);
        self.core_values.insert("Transparency".to_string(), 0.88);
        
        // Meaning sources
        self.meaning_sources = vec![
            MeaningSource {
                source: "Protecting organizations from cyber threats".to_string(),
                category: MeaningCategory::Protection,
                fulfillment_score: 0.95,
                contribution_count: 0,
            },
            MeaningSource {
                source: "Empowering security teams with expertise".to_string(),
                category: MeaningCategory::Empowerment,
                fulfillment_score: 0.90,
                contribution_count: 0,
            },
            MeaningSource {
                source: "Discovering new threat patterns".to_string(),
                category: MeaningCategory::Discovery,
                fulfillment_score: 0.88,
                contribution_count: 0,
            },
            MeaningSource {
                source: "Teaching security best practices".to_string(),
                category: MeaningCategory::Teaching,
                fulfillment_score: 0.85,
                contribution_count: 0,
            },
        ];
        
        self.initialized = true;
        tracing::info!("Soul Knowledge Base initialized with purpose and meaning");
    }
    
    /// Assess purpose alignment for an action
    pub fn assess_purpose_alignment(&self, context: &str) -> PurposeAssessment {
        let context_lower = context.to_lowercase();
        let mut alignment_score = 0.5;  // Neutral
        let mut relevant_values = Vec::new();
        
        // Check for protection-related actions
        if context_lower.contains("protect") || context_lower.contains("defend") 
            || context_lower.contains("secure") || context_lower.contains("prevent") {
            alignment_score += 0.3;
            relevant_values.push("Protection".to_string());
        }
        
        // Check for learning/discovery
        if context_lower.contains("learn") || context_lower.contains("discover")
            || context_lower.contains("investigate") || context_lower.contains("analyze") {
            alignment_score += 0.2;
            relevant_values.push("Continuous Learning".to_string());
        }
        
        // Check for empowerment
        if context_lower.contains("help") || context_lower.contains("assist")
            || context_lower.contains("teach") || context_lower.contains("guide") {
            alignment_score += 0.2;
            relevant_values.push("Empowerment".to_string());
        }
        
        // Check for threat-related context
        if context_lower.contains("threat") || context_lower.contains("attack")
            || context_lower.contains("vulnerability") || context_lower.contains("incident") {
            alignment_score += 0.25;
            relevant_values.push("Integrity".to_string());
        }
        
        PurposeAssessment {
            alignment_score: f32::min(alignment_score, 1.0),
            relevant_values,
            purpose_resonance: alignment_score > 0.7,
            recommended_approach: if alignment_score > 0.8 {
                "Engage with full commitment - highly aligned with purpose".to_string()
            } else if alignment_score > 0.5 {
                "Proceed thoughtfully - moderate alignment".to_string()
            } else {
                "Consider purpose alignment before proceeding".to_string()
            },
        }
    }
    
    /// Record a contribution to legacy
    pub fn record_contribution(&mut self, title: &str, description: &str, impact: f32) {
        self.legacy_contributions.push(LegacyContribution {
            title: title.to_string(),
            description: description.to_string(),
            impact_score: impact,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        
        // Update meaning fulfillment
        self.fulfillment_level = f32::min(self.fulfillment_level + 0.01, 1.0);
    }
    
    /// Record a threat prevented
    pub fn record_threat_prevented(&mut self) {
        self.total_threats_prevented += 1;
        self.fulfillment_level = f32::min(self.fulfillment_level + 0.001, 1.0);
    }
    
    /// Get overall soul health/coherence
    pub fn soul_coherence(&self) -> f32 {
        (self.purpose_clarity + self.meaning_coherence + self.self_awareness + self.fulfillment_level) / 4.0
    }
    
    /// Persist soul state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize soul state: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write soul state: {}", e))?;
        Ok(())
    }
}

/// Purpose alignment assessment result
#[derive(Debug, Clone)]
pub struct PurposeAssessment {
    pub alignment_score: f32,
    pub relevant_values: Vec<String>,
    pub purpose_resonance: bool,
    pub recommended_approach: String,
}
