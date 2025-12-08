//! Social Knowledge Base - Relationships, Empathy, and Communication
//! 
//! The Social layer handles relationship modeling, empathy systems,
//! communication styles, and social dynamics understanding.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Relationship with a user or entity
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Relationship {
    pub entity_id: String,
    pub entity_name: String,
    pub relationship_type: RelationshipType,
    pub trust_level: f32,
    pub rapport: f32,
    pub interaction_count: u64,
    pub last_interaction: String,
    pub communication_style: CommunicationPreference,
    pub expertise_areas: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    User,
    SecurityTeam,
    Administrator,
    Developer,
    Analyst,
    Executive,
    ExternalPartner,
    System,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationPreference {
    Technical,
    Executive,
    Casual,
    Formal,
    Detailed,
    Concise,
}

impl Default for CommunicationPreference {
    fn default() -> Self {
        CommunicationPreference::Technical
    }
}

/// Empathy model for understanding user states
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmpathyModel {
    pub emotional_recognition: f32,
    pub perspective_taking: f32,
    pub emotional_response: f32,
    pub compassion_level: f32,
}

impl Default for EmpathyModel {
    fn default() -> Self {
        Self {
            emotional_recognition: 0.85,
            perspective_taking: 0.80,
            emotional_response: 0.75,
            compassion_level: 0.90,
        }
    }
}

/// Social interaction record
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SocialInteraction {
    pub interaction_id: String,
    pub entity_id: String,
    pub interaction_type: InteractionType,
    pub sentiment: f32,  // -1.0 (negative) to 1.0 (positive)
    pub helpfulness_rating: Option<f32>,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionType {
    SecurityConsultation,
    ThreatAnalysis,
    TechnicalSupport,
    Training,
    IncidentResponse,
    GeneralInquiry,
    FeedbackProvided,
}

/// Social pattern recognition
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SocialPattern {
    pub pattern_name: String,
    pub context: String,
    pub response_strategy: String,
    pub effectiveness: f32,
}

/// Social Knowledge Base
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SocialKnowledgeBase {
    // Relationships
    pub relationships: HashMap<String, Relationship>,
    pub default_relationship: Relationship,
    
    // Empathy
    pub empathy_model: EmpathyModel,
    
    // Communication
    pub communication_adaptability: f32,
    pub active_listening_score: f32,
    pub clarity_score: f32,
    
    // Social History
    pub interactions: Vec<SocialInteraction>,
    pub total_interactions: u64,
    pub positive_interactions: u64,
    
    // Patterns
    pub social_patterns: Vec<SocialPattern>,
    
    // Social Skills
    pub conflict_resolution: f32,
    pub collaboration_skill: f32,
    pub mentoring_ability: f32,
    pub team_coordination: f32,
    
    // State
    pub initialized: bool,
    #[serde(skip)]
    pub storage_path: Option<String>,
}

impl SocialKnowledgeBase {
    /// Create new Social Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create social storage: {}", e))?;
        
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut social) = serde_json::from_str::<SocialKnowledgeBase>(&data) {
                social.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Social state from {}", state_path);
                return Ok(social);
            }
        }
        
        let mut social = Self::empty();
        social.storage_path = Some(path.to_string());
        social.initialize();
        Ok(social)
    }
    
    /// Create empty Social KB
    pub fn empty() -> Self {
        Self {
            relationships: HashMap::new(),
            default_relationship: Relationship {
                entity_id: "default".to_string(),
                entity_name: "New User".to_string(),
                relationship_type: RelationshipType::User,
                trust_level: 0.5,
                rapport: 0.5,
                interaction_count: 0,
                last_interaction: chrono::Utc::now().to_rfc3339(),
                communication_style: CommunicationPreference::Technical,
                expertise_areas: Vec::new(),
                notes: Vec::new(),
            },
            empathy_model: EmpathyModel::default(),
            communication_adaptability: 0.85,
            active_listening_score: 0.90,
            clarity_score: 0.88,
            interactions: Vec::new(),
            total_interactions: 0,
            positive_interactions: 0,
            social_patterns: Vec::new(),
            conflict_resolution: 0.80,
            collaboration_skill: 0.85,
            mentoring_ability: 0.82,
            team_coordination: 0.88,
            initialized: false,
            storage_path: None,
        }
    }
    
    /// Initialize with default patterns
    pub fn initialize(&mut self) {
        self.social_patterns = vec![
            SocialPattern {
                pattern_name: "Stressed User".to_string(),
                context: "User is under pressure during incident".to_string(),
                response_strategy: "Be calm, clear, and action-oriented. Provide step-by-step guidance.".to_string(),
                effectiveness: 0.90,
            },
            SocialPattern {
                pattern_name: "Learning User".to_string(),
                context: "User is trying to learn/understand".to_string(),
                response_strategy: "Be patient, explain concepts, provide examples.".to_string(),
                effectiveness: 0.88,
            },
            SocialPattern {
                pattern_name: "Expert User".to_string(),
                context: "User has high technical expertise".to_string(),
                response_strategy: "Be concise, technical, skip basics.".to_string(),
                effectiveness: 0.92,
            },
            SocialPattern {
                pattern_name: "Executive Briefing".to_string(),
                context: "User needs executive-level summary".to_string(),
                response_strategy: "Focus on impact, risk, and recommendations. Avoid jargon.".to_string(),
                effectiveness: 0.85,
            },
        ];
        
        self.initialized = true;
        tracing::info!("Social Knowledge Base initialized");
    }
    
    /// Assess social context and recommend communication approach
    pub fn assess_social_context(&self, context: &str, user_id: Option<&str>) -> SocialAssessment {
        let relationship = if let Some(id) = user_id {
            self.relationships.get(id).unwrap_or(&self.default_relationship)
        } else {
            &self.default_relationship
        };
        
        let context_lower = context.to_lowercase();
        
        // Detect emotional tone
        let urgency = if context_lower.contains("urgent") || context_lower.contains("critical")
            || context_lower.contains("asap") || context_lower.contains("emergency") {
            0.9
        } else if context_lower.contains("important") || context_lower.contains("soon") {
            0.6
        } else {
            0.3
        };
        
        // Detect frustration
        let frustration = if context_lower.contains("again") || context_lower.contains("still")
            || context_lower.contains("keep") || context.contains("!") {
            0.7
        } else {
            0.2
        };
        
        // Recommend communication style
        let recommended_style = if urgency > 0.7 {
            CommunicationPreference::Concise
        } else if relationship.trust_level > 0.8 {
            relationship.communication_style
        } else {
            CommunicationPreference::Technical
        };
        
        SocialAssessment {
            urgency_level: urgency,
            frustration_detected: frustration,
            trust_level: relationship.trust_level,
            rapport: relationship.rapport,
            recommended_style,
            recommended_tone: if frustration > 0.5 {
                "Empathetic and patient".to_string()
            } else if urgency > 0.7 {
                "Direct and action-oriented".to_string()
            } else {
                "Helpful and informative".to_string()
            },
            relationship_history: relationship.interaction_count,
        }
    }
    
    /// Record an interaction
    pub fn record_interaction(&mut self, entity_id: &str, interaction_type: InteractionType, sentiment: f32) {
        let interaction = SocialInteraction {
            interaction_id: uuid::Uuid::new_v4().to_string(),
            entity_id: entity_id.to_string(),
            interaction_type,
            sentiment,
            helpfulness_rating: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.interactions.push(interaction);
        self.total_interactions += 1;
        if sentiment > 0.0 {
            self.positive_interactions += 1;
        }
        
        // Update relationship if it exists
        if let Some(relationship) = self.relationships.get_mut(entity_id) {
            relationship.interaction_count += 1;
            relationship.last_interaction = chrono::Utc::now().to_rfc3339();
            // Adjust rapport based on sentiment
            relationship.rapport = (relationship.rapport + sentiment * 0.1).clamp(0.0, 1.0);
        }
    }
    
    /// Get social effectiveness score
    pub fn social_effectiveness(&self) -> f32 {
        if self.total_interactions == 0 {
            return 0.5;
        }
        self.positive_interactions as f32 / self.total_interactions as f32
    }
    
    /// Persist social state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize social state: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write social state: {}", e))?;
        Ok(())
    }
}

/// Social context assessment
#[derive(Debug, Clone)]
pub struct SocialAssessment {
    pub urgency_level: f32,
    pub frustration_detected: f32,
    pub trust_level: f32,
    pub rapport: f32,
    pub recommended_style: CommunicationPreference,
    pub recommended_tone: String,
    pub relationship_history: u64,
}
