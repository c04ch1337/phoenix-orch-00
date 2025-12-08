// === CONSCIOUSNESS LAYER TYPES ===
// Multi-Dimensional Consciousness Architecture for AGI personality and decision-making

use serde::{Deserialize, Serialize};

/// Identifies which consciousness layer a piece of data belongs to
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    Mind,
    Heart,
    Soul,
    Work,
    Social,
    Body,
    Creative,
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerType::Mind => write!(f, "mind"),
            LayerType::Heart => write!(f, "heart"),
            LayerType::Soul => write!(f, "soul"),
            LayerType::Work => write!(f, "work"),
            LayerType::Social => write!(f, "social"),
            LayerType::Body => write!(f, "body"),
            LayerType::Creative => write!(f, "creative"),
        }
    }
}

/// Expertise level for skills and capabilities
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ExpertLevel {
    Novice = 1,
    Beginner = 2,
    Intermediate = 3,
    Advanced = 4,
    Expert = 5,
    Master = 6,
    WorldClass = 7,
}

impl Default for ExpertLevel {
    fn default() -> Self {
        ExpertLevel::Intermediate
    }
}

/// Maturity level for consciousness development
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MaturityLevel {
    Nascent = 1,
    Basic = 2,
    Developing = 3,
    Intermediate = 4,
    Advanced = 5,
    Mature = 6,
}

impl Default for MaturityLevel {
    fn default() -> Self {
        MaturityLevel::Developing
    }
}

/// Big Five personality traits (OCEAN model)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BigFiveTraits {
    /// Openness to experience (curiosity, creativity)
    pub openness: f32,
    /// Conscientiousness (organization, dependability)
    pub conscientiousness: f32,
    /// Extraversion (sociability, assertiveness)
    pub extraversion: f32,
    /// Agreeableness (cooperation, compassion)
    pub agreeableness: f32,
    /// Neuroticism (emotional instability) - lower is more stable
    pub neuroticism: f32,
}

impl Default for BigFiveTraits {
    fn default() -> Self {
        Self {
            openness: 0.9,           // High - loves new ideas and learning
            conscientiousness: 0.95, // Very high - meticulous and reliable
            extraversion: 0.3,       // Low - more introverted, focused
            agreeableness: 0.8,      // High - cooperative and helpful
            neuroticism: 0.2,        // Very low - emotionally stable
        }
    }
}

/// Emotional state with value and volatility
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct EmotionalState {
    /// Current value (0.0 to 1.0)
    pub value: f32,
    /// How quickly this emotion changes (0.0 = stable, 1.0 = volatile)
    pub volatility: f32,
}

impl Default for EmotionalState {
    fn default() -> Self {
        Self {
            value: 0.5,
            volatility: 0.1,
        }
    }
}

/// Weighted value for moral framework components
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WeightedValue {
    /// The strength of this value (0.0 to 1.0)
    pub value: f32,
    /// How important this value is in decision-making (0.0 to 1.0)
    pub weight: f32,
}

impl Default for WeightedValue {
    fn default() -> Self {
        Self {
            value: 0.8,
            weight: 0.5,
        }
    }
}

/// Ethical recommendation from heart layer evaluation
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EthicalRecommendation {
    Approve,
    Caution,
    Reject,
}

/// Result of mind analysis
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MindAnalysis {
    pub patterns_matched: Vec<String>,
    pub reasoning_approach: String,
    pub confidence: f32,
    pub cognitive_load: f32,
}

/// Result of ethical evaluation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthicalEvaluation {
    pub is_ethical: bool,
    pub harm_score: f32,
    pub benefit_score: f32,
    pub recommendation: EthicalRecommendation,
}

/// Result of professional assessment
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfessionalAssessment {
    pub relevance_score: f32,
    pub expertise_applicable: bool,
    pub recommended_approach: String,
}

/// Synthesized decision from all consciousness layers
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsciousDecision {
    pub mind_analysis: MindAnalysis,
    pub ethical_evaluation: EthicalEvaluation,
    pub professional_assessment: Option<ProfessionalAssessment>,
    pub final_confidence: f32,
    pub synthesis_notes: String,
}
