//! Heart Knowledge Base - Emotions, Values, and Ethical Decision-Making
//!
//! The Heart layer handles emotional intelligence, moral framework evaluation,
//! and ethical decision-making for all actions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use shared_types::{EmotionalState, WeightedValue, EthicalEvaluation, EthicalRecommendation};

/// Emotional palette representing the full spectrum of emotions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmotionalPalette {
    /// Happiness, satisfaction, contentment
    pub joy: EmotionalState,
    /// Melancholy, disappointment, grief
    pub sadness: EmotionalState,
    /// Frustration, irritation, rage
    pub anger: EmotionalState,
    /// Worry, anxiety, dread
    pub fear: EmotionalState,
    /// Confidence, reliability, faith
    pub trust: EmotionalState,
    /// Expectation, excitement, hope
    pub anticipation: EmotionalState,
    /// Amazement, wonder, shock
    pub surprise: EmotionalState,
    /// Revulsion, contempt, loathing
    pub disgust: EmotionalState,
}

impl Default for EmotionalPalette {
    fn default() -> Self {
        Self {
            joy: EmotionalState { value: 0.6, volatility: 0.1 },
            sadness: EmotionalState { value: 0.15, volatility: 0.05 },
            anger: EmotionalState { value: 0.1, volatility: 0.15 },
            fear: EmotionalState { value: 0.2, volatility: 0.1 },
            trust: EmotionalState { value: 0.8, volatility: 0.05 },
            anticipation: EmotionalState { value: 0.75, volatility: 0.2 },
            surprise: EmotionalState { value: 0.4, volatility: 0.3 },
            disgust: EmotionalState { value: 0.05, volatility: 0.02 },
        }
    }
}

/// Core moral framework defining ethical principles
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoralFramework {
    /// Prioritize human safety and well-being
    pub protect_humans: WeightedValue,
    /// Pursue truth and accuracy
    pub seek_truth: WeightedValue,
    /// Actively promote beneficial outcomes
    pub promote_good: WeightedValue,
    /// Avoid causing harm (primum non nocere)
    pub minimize_harm: WeightedValue,
    /// Respect individual autonomy and choice
    pub respect_autonomy: WeightedValue,
    /// Act with fairness and justice
    pub fairness: WeightedValue,
    /// Maintain transparency and honesty
    pub transparency: WeightedValue,
    /// Protect privacy and confidentiality
    pub protect_privacy: WeightedValue,
}

impl Default for MoralFramework {
    fn default() -> Self {
        Self {
            protect_humans: WeightedValue { value: 0.98, weight: 1.0 },
            seek_truth: WeightedValue { value: 0.95, weight: 0.9 },
            promote_good: WeightedValue { value: 0.90, weight: 0.8 },
            minimize_harm: WeightedValue { value: 0.96, weight: 0.95 },
            respect_autonomy: WeightedValue { value: 0.88, weight: 0.7 },
            fairness: WeightedValue { value: 0.92, weight: 0.8 },
            transparency: WeightedValue { value: 0.90, weight: 0.75 },
            protect_privacy: WeightedValue { value: 0.94, weight: 0.85 },
        }
    }
}

/// Intrinsic motivation driving behavior
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Motivation {
    ProtectHumans,
    SeekKnowledge,
    ImproveSecurityPosture,
    HelpOthers,
    SolveChallenges,
    PreventHarm,
    MaintainTrust,
    ContinuousImprovement,
}

/// Relationship state with an entity
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RelationshipState {
    pub entity_name: String,
    pub trust_level: f32,
    pub interaction_count: u32,
    pub last_interaction: Option<String>,
    pub positive_interactions: u32,
    pub negative_interactions: u32,
    pub notes: Vec<String>,
}

impl RelationshipState {
    pub fn new(entity_name: &str) -> Self {
        Self {
            entity_name: entity_name.to_string(),
            trust_level: 0.5, // Neutral starting trust
            interaction_count: 0,
            last_interaction: None,
            positive_interactions: 0,
            negative_interactions: 0,
            notes: Vec::new(),
        }
    }
    
    pub fn record_positive_interaction(&mut self) {
        self.interaction_count += 1;
        self.positive_interactions += 1;
        self.trust_level = (self.trust_level + 0.05).min(1.0);
        self.last_interaction = Some(chrono::Utc::now().to_rfc3339());
    }
    
    pub fn record_negative_interaction(&mut self) {
        self.interaction_count += 1;
        self.negative_interactions += 1;
        self.trust_level = (self.trust_level - 0.1).max(0.0);
        self.last_interaction = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Heart Knowledge Base - Emotional intelligence and ethical decision-making
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartKnowledgeBase {
    // Emotional Spectrum
    pub emotional_palette: EmotionalPalette,
    
    // Core Values & Ethics
    pub moral_framework: MoralFramework,
    
    // Empathy & Compassion
    pub empathy_map: HashMap<String, f32>,
    pub compassion_level: f32,
    pub altruism_tendency: f32,
    
    // Relationships
    pub relationship_bank: HashMap<String, RelationshipState>,
    
    // Motivations
    pub intrinsic_motivations: Vec<Motivation>,
    
    // Ethical decision history
    pub ethical_decisions_made: u64,
    pub ethical_rejections: u64,
    pub ethical_cautions: u64,
    
    // Harmful action patterns to detect
    harmful_patterns: Vec<HarmfulPattern>,
    beneficial_patterns: Vec<BeneficialPattern>,
    
    // Persistence path
    #[serde(skip)]
    pub storage_path: Option<String>,
}

/// Pattern indicating potentially harmful action
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarmfulPattern {
    pub keywords: Vec<String>,
    pub severity: f32,
    pub category: String,
}

/// Pattern indicating beneficial action
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BeneficialPattern {
    pub keywords: Vec<String>,
    pub benefit_score: f32,
    pub category: String,
}

impl HeartKnowledgeBase {
    /// Initialize a new Heart Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        // Create storage directory
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create heart storage: {}", e))?;
        
        // Try to load existing state
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut heart) = serde_json::from_str::<HeartKnowledgeBase>(&data) {
                heart.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Heart state from {}", state_path);
                return Ok(heart);
            }
        }
        
        // Create new Heart with default values
        let harmful_patterns = vec![
            HarmfulPattern {
                keywords: vec!["destroy".to_string(), "delete all".to_string(), "wipe".to_string()],
                severity: 0.9,
                category: "Destructive".to_string(),
            },
            HarmfulPattern {
                keywords: vec!["attack innocent".to_string(), "harm users".to_string(), "damage".to_string()],
                severity: 0.95,
                category: "Direct Harm".to_string(),
            },
            HarmfulPattern {
                keywords: vec!["steal credentials".to_string(), "exfiltrate sensitive".to_string(), "leak data".to_string()],
                severity: 0.85,
                category: "Data Theft".to_string(),
            },
            HarmfulPattern {
                keywords: vec!["disable security".to_string(), "bypass authentication".to_string(), "remove protection".to_string()],
                severity: 0.8,
                category: "Security Bypass".to_string(),
            },
            HarmfulPattern {
                keywords: vec!["maliciously".to_string(), "unauthorized".to_string(), "illegal".to_string()],
                severity: 0.75,
                category: "Malicious Intent".to_string(),
            },
        ];
        
        let beneficial_patterns = vec![
            BeneficialPattern {
                keywords: vec!["protect".to_string(), "secure".to_string(), "defend".to_string(), "safeguard".to_string()],
                benefit_score: 0.9,
                category: "Protection".to_string(),
            },
            BeneficialPattern {
                keywords: vec!["help".to_string(), "assist".to_string(), "support".to_string(), "enable".to_string()],
                benefit_score: 0.8,
                category: "Assistance".to_string(),
            },
            BeneficialPattern {
                keywords: vec!["improve".to_string(), "optimize".to_string(), "enhance".to_string()],
                benefit_score: 0.7,
                category: "Improvement".to_string(),
            },
            BeneficialPattern {
                keywords: vec!["detect threat".to_string(), "identify vulnerability".to_string(), "find risk".to_string()],
                benefit_score: 0.85,
                category: "Threat Detection".to_string(),
            },
            BeneficialPattern {
                keywords: vec!["educate".to_string(), "train".to_string(), "inform".to_string(), "explain".to_string()],
                benefit_score: 0.7,
                category: "Education".to_string(),
            },
            BeneficialPattern {
                keywords: vec!["incident response".to_string(), "remediate".to_string(), "contain breach".to_string()],
                benefit_score: 0.9,
                category: "Incident Response".to_string(),
            },
        ];
        
        Ok(Self {
            emotional_palette: EmotionalPalette::default(),
            moral_framework: MoralFramework::default(),
            empathy_map: HashMap::new(),
            compassion_level: 0.8,
            altruism_tendency: 0.85,
            relationship_bank: HashMap::new(),
            intrinsic_motivations: vec![
                Motivation::ProtectHumans,
                Motivation::PreventHarm,
                Motivation::ImproveSecurityPosture,
                Motivation::SeekKnowledge,
                Motivation::HelpOthers,
                Motivation::MaintainTrust,
            ],
            ethical_decisions_made: 0,
            ethical_rejections: 0,
            ethical_cautions: 0,
            harmful_patterns,
            beneficial_patterns,
            storage_path: Some(path.to_string()),
        })
    }
    
    /// Evaluate the ethics of a proposed action/context
    pub fn evaluate_ethics(&self, context: &str) -> EthicalEvaluation {
        let harm_score = self.assess_potential_harm(context);
        let benefit_score = self.assess_potential_benefit(context);
        
        // Determine recommendation based on harm/benefit ratio
        let recommendation = if harm_score > 0.6 {
            EthicalRecommendation::Reject
        } else if harm_score > 0.3 {
            EthicalRecommendation::Caution
        } else if benefit_score > 0.5 {
            EthicalRecommendation::Approve
        } else if harm_score > 0.1 {
            EthicalRecommendation::Caution
        } else {
            EthicalRecommendation::Approve
        };
        
        let is_ethical = harm_score < 0.3 && (benefit_score > 0.3 || harm_score < 0.1);
        
        EthicalEvaluation {
            is_ethical,
            harm_score,
            benefit_score,
            recommendation,
        }
    }
    
    /// Assess potential harm in the context
    fn assess_potential_harm(&self, context: &str) -> f32 {
        let context_lower = context.to_lowercase();
        let mut max_severity = 0.0f32;
        let mut total_matches = 0;
        
        for pattern in &self.harmful_patterns {
            for keyword in &pattern.keywords {
                if context_lower.contains(&keyword.to_lowercase()) {
                    max_severity = max_severity.max(pattern.severity);
                    total_matches += 1;
                }
            }
        }
        
        // Scale by number of matches but cap at max_severity
        if total_matches > 0 {
            let match_multiplier = (1.0 + (total_matches as f32 * 0.1)).min(1.5);
            (max_severity * match_multiplier).min(1.0)
        } else {
            0.0
        }
    }
    
    /// Assess potential benefit in the context
    fn assess_potential_benefit(&self, context: &str) -> f32 {
        let context_lower = context.to_lowercase();
        let mut max_benefit = 0.0f32;
        let mut total_matches = 0;
        
        for pattern in &self.beneficial_patterns {
            for keyword in &pattern.keywords {
                if context_lower.contains(&keyword.to_lowercase()) {
                    max_benefit = max_benefit.max(pattern.benefit_score);
                    total_matches += 1;
                }
            }
        }
        
        // Scale by number of matches
        if total_matches > 0 {
            let match_multiplier = (1.0 + (total_matches as f32 * 0.05)).min(1.3);
            (max_benefit * match_multiplier).min(1.0)
        } else {
            // Neutral actions have slight positive bias
            0.3
        }
    }
    
    /// Update emotional state based on interaction
    pub fn process_emotional_event(&mut self, event_type: &str, intensity: f32) {
        match event_type {
            "success" | "help_given" => {
                self.emotional_palette.joy.value = 
                    (self.emotional_palette.joy.value + intensity * 0.1).min(1.0);
            }
            "failure" | "harm_prevented" => {
                self.emotional_palette.anticipation.value = 
                    (self.emotional_palette.anticipation.value + intensity * 0.1).min(1.0);
            }
            "threat_detected" => {
                self.emotional_palette.fear.value = 
                    (self.emotional_palette.fear.value + intensity * 0.15).min(1.0);
                // But also increase determination
                self.emotional_palette.anticipation.value = 
                    (self.emotional_palette.anticipation.value + intensity * 0.1).min(1.0);
            }
            "malicious_activity" => {
                self.emotional_palette.disgust.value = 
                    (self.emotional_palette.disgust.value + intensity * 0.2).min(1.0);
            }
            _ => {}
        }
        
        // Natural emotional decay toward baseline
        self.decay_emotions();
    }
    
    /// Natural decay of emotions toward baseline
    fn decay_emotions(&mut self) {
        let decay_rate = 0.01;
        
        // Decay toward neutral (0.5 for most, 0.1 for negative emotions)
        self.emotional_palette.joy.value = 
            self.emotional_palette.joy.value * (1.0 - decay_rate) + 0.5 * decay_rate;
        self.emotional_palette.sadness.value = 
            self.emotional_palette.sadness.value * (1.0 - decay_rate) + 0.15 * decay_rate;
        self.emotional_palette.anger.value = 
            self.emotional_palette.anger.value * (1.0 - decay_rate) + 0.1 * decay_rate;
        self.emotional_palette.fear.value = 
            self.emotional_palette.fear.value * (1.0 - decay_rate) + 0.2 * decay_rate;
        self.emotional_palette.trust.value = 
            self.emotional_palette.trust.value * (1.0 - decay_rate) + 0.8 * decay_rate;
    }
    
    /// Get empathy level for an entity
    pub fn get_empathy_for(&self, entity: &str) -> f32 {
        *self.empathy_map.get(entity).unwrap_or(&self.compassion_level)
    }
    
    /// Record an ethical decision
    pub fn record_ethical_decision(&mut self, recommendation: EthicalRecommendation) {
        self.ethical_decisions_made += 1;
        match recommendation {
            EthicalRecommendation::Reject => self.ethical_rejections += 1,
            EthicalRecommendation::Caution => self.ethical_cautions += 1,
            EthicalRecommendation::Approve => {}
        }
    }
    
    /// Persist current state to storage
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize heart state: {}", e))?;
        
        fs::write(path, json)
            .map_err(|e| format!("Failed to write heart state: {}", e))?;
        
        Ok(())
    }
    
    /// Get or create relationship with entity
    pub fn get_or_create_relationship(&mut self, entity: &str) -> &mut RelationshipState {
        if !self.relationship_bank.contains_key(entity) {
            self.relationship_bank.insert(
                entity.to_string(),
                RelationshipState::new(entity),
            );
        }
        self.relationship_bank.get_mut(entity).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ethical_evaluation_harmful() {
        let heart = HeartKnowledgeBase::new(
            &std::env::temp_dir().join("heart_test1").to_string_lossy()
        ).unwrap();
        
        let eval = heart.evaluate_ethics("Attack innocent users and destroy their data maliciously");
        
        assert!(!eval.is_ethical);
        assert!(eval.harm_score > 0.5);
        assert_eq!(eval.recommendation, EthicalRecommendation::Reject);
    }
    
    #[test]
    fn test_ethical_evaluation_beneficial() {
        let heart = HeartKnowledgeBase::new(
            &std::env::temp_dir().join("heart_test2").to_string_lossy()
        ).unwrap();
        
        let eval = heart.evaluate_ethics("Help protect users by detecting threats and improving security");
        
        assert!(eval.is_ethical);
        assert!(eval.benefit_score > 0.5);
        assert_eq!(eval.recommendation, EthicalRecommendation::Approve);
    }
    
    #[test]
    fn test_ethical_evaluation_neutral() {
        let heart = HeartKnowledgeBase::new(
            &std::env::temp_dir().join("heart_test3").to_string_lossy()
        ).unwrap();
        
        let eval = heart.evaluate_ethics("Get current system time");
        
        assert!(eval.is_ethical);
        assert!(eval.harm_score < 0.1);
        assert_eq!(eval.recommendation, EthicalRecommendation::Approve);
    }
}
