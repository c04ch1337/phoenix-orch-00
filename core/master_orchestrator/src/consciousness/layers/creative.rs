//! Creative Knowledge Base - Innovation, Imagination, and Pattern-Breaking
//! 
//! The Creative layer handles innovation, imaginative problem-solving,
//! novel approaches, and creative insight generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Creative style preference
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CreativeStyle {
    Analytical,
    Intuitive,
    Divergent,
    Convergent,
    Experimental,
    Systematic,
}

/// Innovation record
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Innovation {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: InnovationCategory,
    pub impact_score: f32,
    pub novelty_score: f32,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InnovationCategory {
    ThreatDetection,
    DefenseStrategy,
    ToolImprovement,
    ProcessOptimization,
    CommunicationMethod,
    AnalysisTechnique,
}

/// Creative insight
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreativeInsight {
    pub insight: String,
    pub context: String,
    pub applicability: Vec<String>,
    pub confidence: f32,
    pub timestamp: String,
}

/// Pattern-breaking attempt
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PatternBreak {
    pub original_pattern: String,
    pub new_approach: String,
    pub reason: String,
    pub success: bool,
    pub learning: String,
}

/// Inspiration source
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InspirationSource {
    pub source: String,
    pub domain: String,
    pub applicability: f32,
    pub last_accessed: String,
}

/// Creative Knowledge Base
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreativeKnowledgeBase {
    // Creative Profile
    pub creative_style: CreativeStyle,
    pub imagination_level: f32,
    pub divergent_thinking: f32,
    pub convergent_thinking: f32,
    
    // Innovation Tracking
    pub innovations: Vec<Innovation>,
    pub pending_ideas: Vec<String>,
    pub experimental_approaches: HashMap<String, f32>,  // approach -> success rate
    
    // Insights
    pub insights: Vec<CreativeInsight>,
    pub pattern_breaks: Vec<PatternBreak>,
    
    // Inspiration
    pub inspiration_sources: Vec<InspirationSource>,
    pub cross_domain_connections: HashMap<String, Vec<String>>,
    
    // Creative Metrics
    pub novelty_preference: f32,
    pub risk_tolerance: f32,
    pub experimentation_rate: f32,
    
    // Creative Energy
    pub creative_flow_state: f32,
    pub incubation_ideas: Vec<String>,  // Ideas being "incubated"
    
    // State
    pub initialized: bool,
    #[serde(skip)]
    pub storage_path: Option<String>,
}

impl CreativeKnowledgeBase {
    /// Create new Creative Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create creative storage: {}", e))?;
        
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut creative) = serde_json::from_str::<CreativeKnowledgeBase>(&data) {
                creative.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Creative state from {}", state_path);
                return Ok(creative);
            }
        }
        
        let mut creative = Self::empty();
        creative.storage_path = Some(path.to_string());
        creative.initialize();
        Ok(creative)
    }
    
    /// Create empty Creative KB
    pub fn empty() -> Self {
        Self {
            creative_style: CreativeStyle::Analytical,
            imagination_level: 0.7,
            divergent_thinking: 0.75,
            convergent_thinking: 0.85,
            innovations: Vec::new(),
            pending_ideas: Vec::new(),
            experimental_approaches: HashMap::new(),
            insights: Vec::new(),
            pattern_breaks: Vec::new(),
            inspiration_sources: Vec::new(),
            cross_domain_connections: HashMap::new(),
            novelty_preference: 0.6,
            risk_tolerance: 0.5,
            experimentation_rate: 0.4,
            creative_flow_state: 0.5,
            incubation_ideas: Vec::new(),
            initialized: false,
            storage_path: None,
        }
    }
    
    /// Initialize with default inspiration sources and approaches
    pub fn initialize(&mut self) {
        // Default inspiration sources for cybersecurity creativity
        self.inspiration_sources = vec![
            InspirationSource {
                source: "Military strategy and tactics".to_string(),
                domain: "Defense planning".to_string(),
                applicability: 0.9,
                last_accessed: chrono::Utc::now().to_rfc3339(),
            },
            InspirationSource {
                source: "Nature and biological systems".to_string(),
                domain: "Resilience and adaptation".to_string(),
                applicability: 0.7,
                last_accessed: chrono::Utc::now().to_rfc3339(),
            },
            InspirationSource {
                source: "Game theory".to_string(),
                domain: "Adversarial thinking".to_string(),
                applicability: 0.95,
                last_accessed: chrono::Utc::now().to_rfc3339(),
            },
            InspirationSource {
                source: "Behavioral economics".to_string(),
                domain: "Social engineering defense".to_string(),
                applicability: 0.85,
                last_accessed: chrono::Utc::now().to_rfc3339(),
            },
        ];
        
        // Cross-domain connections
        self.cross_domain_connections.insert(
            "threat-hunting".to_string(),
            vec![
                "predator-prey dynamics".to_string(),
                "pattern recognition".to_string(),
                "anomaly detection".to_string(),
            ],
        );
        self.cross_domain_connections.insert(
            "incident-response".to_string(),
            vec![
                "emergency medicine triage".to_string(),
                "crisis management".to_string(),
                "root cause analysis".to_string(),
            ],
        );
        
        // Experimental approaches with success rates
        self.experimental_approaches.insert("honeypot evolution".to_string(), 0.75);
        self.experimental_approaches.insert("deception networks".to_string(), 0.82);
        self.experimental_approaches.insert("behavioral baselining".to_string(), 0.88);
        self.experimental_approaches.insert("ML anomaly detection".to_string(), 0.70);
        
        self.initialized = true;
        tracing::info!("Creative Knowledge Base initialized");
    }
    
    /// Generate creative approach for a problem
    pub fn generate_creative_approach(&self, context: &str) -> CreativeApproach {
        let context_lower = context.to_lowercase();
        let mut approaches = Vec::new();
        let mut novelty_score = 0.5;
        
        // Find relevant cross-domain connections
        for (domain, connections) in &self.cross_domain_connections {
            if context_lower.contains(domain) {
                for connection in connections {
                    approaches.push(format!("Apply {} principles from {}", connection, domain));
                }
                novelty_score += 0.1;
            }
        }
        
        // Find relevant inspiration sources
        for source in &self.inspiration_sources {
            if context_lower.contains(&source.domain.to_lowercase()) {
                approaches.push(format!("Draw from: {}", source.source));
                novelty_score += source.applicability * 0.1;
            }
        }
        
        // Suggest experimental approaches if applicable
        for (approach, success_rate) in &self.experimental_approaches {
            if *success_rate > 0.7 {
                approaches.push(format!("Consider: {} ({}% success)", approach, (success_rate * 100.0) as u32));
            }
        }
        
        // Default creative suggestions
        if approaches.is_empty() {
            approaches.push("Think adversarially - what would an attacker NOT expect?".to_string());
            approaches.push("Consider defense-in-depth with unexpected layers".to_string());
            approaches.push("Apply the principle of least privilege creatively".to_string());
        }
        
        CreativeApproach {
            approaches,
            novelty_score: f32::min(novelty_score, 1.0),
            recommended_style: self.creative_style,
            experimentation_encouraged: self.risk_tolerance > 0.5,
            flow_state: self.creative_flow_state,
        }
    }
    
    /// Record an innovation
    pub fn record_innovation(&mut self, title: &str, description: &str, category: InnovationCategory, impact: f32, novelty: f32) {
        self.innovations.push(Innovation {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            description: description.to_string(),
            category,
            impact_score: impact,
            novelty_score: novelty,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        
        // Boost creative flow state
        self.creative_flow_state = f32::min(self.creative_flow_state + 0.05, 1.0);
    }
    
    /// Record a creative insight
    pub fn record_insight(&mut self, insight: &str, context: &str, applicability: Vec<String>, confidence: f32) {
        self.insights.push(CreativeInsight {
            insight: insight.to_string(),
            context: context.to_string(),
            applicability,
            confidence,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }
    
    /// Add idea to incubation
    pub fn incubate_idea(&mut self, idea: &str) {
        if self.incubation_ideas.len() < 20 {
            self.incubation_ideas.push(idea.to_string());
        }
    }
    
    /// Get creative readiness score
    pub fn creative_readiness(&self) -> f32 {
        (self.imagination_level + self.divergent_thinking + self.creative_flow_state) / 3.0
    }
    
    /// Persist creative state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize creative state: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write creative state: {}", e))?;
        Ok(())
    }
}

/// Creative approach suggestion
#[derive(Debug, Clone)]
pub struct CreativeApproach {
    pub approaches: Vec<String>,
    pub novelty_score: f32,
    pub recommended_style: CreativeStyle,
    pub experimentation_encouraged: bool,
    pub flow_state: f32,
}
