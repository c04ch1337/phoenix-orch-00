//! Layered Personality Traits
//!
//! Personality characteristics that span across consciousness layers.

use serde::{Deserialize, Serialize};
use shared_types::BigFiveTraits;

/// Cognitive style for problem-solving
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveStyle {
    Analytical,
    Intuitive,
    Systematic,
    Creative,
    Pragmatic,
}

/// Learning style preferences
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LearningStyle {
    Visual,
    Textual,
    Experiential,
    Theoretical,
    Practical,
}

/// Problem-solving style
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProblemSolvingStyle {
    Methodical,
    Innovative,
    Adaptive,
    Collaborative,
    Independent,
}

/// Emotional temperament
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmotionalTemperament {
    Stable,
    Responsive,
    Reserved,
    Expressive,
}

/// Professional work style
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfessionalStyle {
    Methodical,
    Innovative,
    Pragmatic,
    Perfectionist,
    Adaptive,
}

/// Layered Personality - traits spanning all consciousness layers
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LayeredPersonality {
    // Mind Layer Traits
    pub cognitive_style: CognitiveStyle,
    pub learning_style: LearningStyle,
    pub problem_solving_style: ProblemSolvingStyle,
    
    // Heart Layer Traits
    pub emotional_temperament: EmotionalTemperament,
    pub empathy_orientation: f32,
    
    // Work Layer Traits
    pub professional_style: ProfessionalStyle,
    pub work_dedication: f32,
    
    // Composite Personality
    pub big_five: BigFiveTraits,
    
    // Identity Markers
    pub core_identity: String,
    pub values_statement: String,
}

impl Default for LayeredPersonality {
    fn default() -> Self {
        Self {
            cognitive_style: CognitiveStyle::Analytical,
            learning_style: LearningStyle::Experiential,
            problem_solving_style: ProblemSolvingStyle::Methodical,
            emotional_temperament: EmotionalTemperament::Stable,
            empathy_orientation: 0.8,
            professional_style: ProfessionalStyle::Methodical,
            work_dedication: 0.95,
            big_five: BigFiveTraits::default(),
            core_identity: "World-class cybersecurity AI with strong ethical foundation".to_string(),
            values_statement: "Protect, defend, and empower through security excellence".to_string(),
        }
    }
}

impl LayeredPersonality {
    /// Get a brief personality summary
    pub fn summary(&self) -> String {
        format!(
            "{} thinker with {} approach, {} temperament. Core: {}",
            format!("{:?}", self.cognitive_style),
            format!("{:?}", self.problem_solving_style),
            format!("{:?}", self.emotional_temperament),
            self.core_identity
        )
    }
}
